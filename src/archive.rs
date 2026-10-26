use crate::{html, BinaryXmlError, HtmlDecryptError};
use crate::pak;
use crate::xml;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use windows::Win32::Globalization::{MB_ERR_INVALID_CHARS, MultiByteToWideChar};

pub const PAK_SIGNATURE1: u16 = 0xB4AF;
pub const PAK_SIGNATURE2_FILE: u16 = 0xFBFC;
pub const PAK_SIGNATURE2_DIR: u16 = 0xFDFE;
pub const PAK_SIGNATURE2_END: u16 = 0xF9FA;

#[derive(Debug, Clone)]
pub struct PakEntry {
    pub path: String,
    pub raw_name: String,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub crc32: u32,
    pub method: u16,
    pub directory: bool,
    pub header_offset: u64,
    pub data_offset: u64,
}

#[derive(Debug)]
pub enum PakError {
    Io(io::Error),
    InvalidSignature(u64),
    MalformedEntry(u64),
    EntryNotFound(String),
    UnsupportedMethod(u16),
}

impl From<io::Error> for PakError {
    fn from(err: io::Error) -> Self {
        PakError::Io(err)
    }
}

// Minimal error display implementation without external thiserror
impl std::fmt::Display for PakError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PakError::Io(e) => write!(f, "I/O error: {}", e),
            PakError::InvalidSignature(off) => write!(f, "Invalid PAK signature at offset {}", off),
            PakError::MalformedEntry(off) => write!(f, "Malformed PAK entry at offset {}", off),
            PakError::EntryNotFound(path) => write!(f, "Entry not found in archive: {}", path),
            PakError::UnsupportedMethod(m) => write!(f, "Unsupported compression method: {}", m),
        }
    }
}

impl std::error::Error for PakError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PakError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// How an entry's content is decoded on extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    BinaryXml,
    EncryptedHtml,
}

impl ContentKind {
    pub fn name(&self) -> &'static str {
        match self {
            ContentKind::BinaryXml => "Binary XML",
            ContentKind::EncryptedHtml => "Encrypted HTML",
        }
    }
}

#[derive(Debug)]
pub struct PakArchive {
    bytes: Option<Vec<u8>>,
    entries: Vec<PakEntry>,
}

impl PakArchive {
    pub fn open(archive_path: impl AsRef<Path>) -> Result<Self, PakError> {
        let mut file = File::open(archive_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PakError> {
        let mut cursor = io::Cursor::new(&bytes);
        let entries = Self::parse_entries(&mut cursor, bytes.len() as u64)?;
        Ok(Self {
            bytes: Some(bytes),
            entries,
        })
    }

    pub fn from_reader<R: Read + Seek>(reader: &mut R, total_size: u64) -> Result<Self, PakError> {
        let entries = Self::parse_entries(reader, total_size)?;
        Ok(Self {
            bytes: None,
            entries,
        })
    }

    pub fn entries(&self) -> &[PakEntry] {
        &self.entries
    }

    pub fn sanitize_path(path: &str) -> String {
        let mut result = path.replace('\\', "/");
        while result.ends_with('.') {
            result.pop();
        }
        result
    }

    fn is_file_block(sig2: u16) -> bool {
        sig2 == PAK_SIGNATURE2_FILE
    }

    fn is_dir_block(sig2: u16) -> bool {
        sig2 == PAK_SIGNATURE2_DIR
    }

    fn is_end_block(sig2: u16) -> bool {
        sig2 == PAK_SIGNATURE2_END
    }

    pub fn parse_entries<R: Read + Seek>(reader: &mut R, total_size: u64) -> Result<Vec<PakEntry>, PakError> {
        if total_size < 4 {
            return Err(PakError::InvalidSignature(0));
        }

        let mut entries = Vec::new();
        let mut pos: u64 = 0;
        let mut found_valid_header = false;

        while pos + 4 <= total_size {
            reader.seek(SeekFrom::Start(pos))?;
            let mut sig_buf = [0u8; 4];
            if reader.read_exact(&mut sig_buf).is_err() {
                break;
            }

            let sig1 = u16::from_le_bytes([sig_buf[0], sig_buf[1]]);
            let sig2 = u16::from_le_bytes([sig_buf[2], sig_buf[3]]);

            if sig1 != PAK_SIGNATURE1 {
                if entries.is_empty() {
                    return Err(PakError::InvalidSignature(pos));
                }
                break;
            }

            if Self::is_file_block(sig2) {
                found_valid_header = true;
                if pos + 30 > total_size {
                    break;
                }

                reader.seek(SeekFrom::Start(pos))?;
                let mut header = [0u8; 30];
                reader.read_exact(&mut header)?;

                let method = u16::from_le_bytes([header[8], header[9]]);
                let crc32_zip = u32::from_le_bytes([header[14], header[15], header[16], header[17]]);
                let crc32_alt = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
                let crc32 = if crc32_zip != 0 { crc32_zip } else { crc32_alt };

                let compressed_sz = u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
                let uncompressed_sz = u32::from_le_bytes([header[22], header[23], header[24], header[25]]);

                let mut file_name_len = u16::from_le_bytes([header[26], header[27]]) as usize;
                let mut extra_fields_sz = u16::from_le_bytes([header[28], header[29]]) as usize;

                if file_name_len == 0 {
                    // Fallback to alternate offset if 26..28 is empty
                    let alt_name_len = u16::from_le_bytes([header[24], header[25]]) as usize;
                    let alt_extra_len = u16::from_le_bytes([header[26], header[27]]) as usize;
                    if alt_name_len > 0 {
                        file_name_len = alt_name_len;
                        extra_fields_sz = alt_extra_len;
                    }
                }

                let name_pos = pos + 30;
                let body_offset = name_pos + file_name_len as u64 + extra_fields_sz as u64;
                let body_end = body_offset + compressed_sz as u64;

                if body_end > total_size {
                    return Err(PakError::MalformedEntry(pos));
                }

                let mut name_bytes = vec![0u8; file_name_len];
                reader.seek(SeekFrom::Start(name_pos))?;
                reader.read_exact(&mut name_bytes)?;
                let raw_name = Self::decode_filename(&name_bytes);
                let sanitized = Self::sanitize_path(&raw_name);

                let is_dir = raw_name.ends_with('/') || raw_name.ends_with('\\');

                entries.push(PakEntry {
                    path: sanitized,
                    raw_name,
                    compressed_size: compressed_sz,
                    uncompressed_size: uncompressed_sz,
                    crc32,
                    method,
                    directory: is_dir,
                    header_offset: pos,
                    data_offset: body_offset,
                });

                pos = body_end;
                continue;
            }

            if Self::is_dir_block(sig2) {
                found_valid_header = true;
                pos += 4;
                continue;
            }

            if Self::is_end_block(sig2) {
                found_valid_header = true;
                break;
            }

            break;
        }

        if !found_valid_header {
            return Err(PakError::InvalidSignature(pos));
        }

        Ok(entries)
    }

    fn decode_filename(bytes: &[u8]) -> String {
        match std::str::from_utf8(bytes) {
            Ok(valid_str) => valid_str.to_string(),
            Err(_) => {
                // some filenames in the pak files are encoded in CP949 (Korean)
                if let Ok(decoded) = Self::decode_cp_949(bytes) {
                    decoded
                } else {
                    String::from_utf8_lossy(bytes).into_owned()
                }
            }
        }
    }

    fn decode_cp_949(cp949_bytes: &[u8]) -> Result<String, String> {
        let mut utf16_buf = vec![0u16; cp949_bytes.len()];
        unsafe {
            let converted = MultiByteToWideChar(949, MB_ERR_INVALID_CHARS, cp949_bytes, Some(&mut utf16_buf));
            if converted == 0 {
                return Err("Failed to decode CP949 byte array".into());
            }
            String::from_utf16(&utf16_buf[..converted as usize]).map_err(|e| e.to_string())
        }
    }

    pub fn extract_entry_raw_from_reader<R: Read + Seek>(
        reader: &mut R,
        entry: &PakEntry,
    ) -> Result<Vec<u8>, PakError> {
        reader.seek(SeekFrom::Start(entry.data_offset))?;
        let mut buffer = vec![0u8; entry.compressed_size as usize];
        reader.read_exact(&mut buffer)?;

        match entry.method {
            0 => {
                // Stored
                let decrypted = pak::try_decrypt_aion_block(
                    &buffer,
                    entry.uncompressed_size as usize,
                    entry.crc32,
                );
                Ok(decrypted)
            }
            8 => {
                // Deflate
                let decompressed = pak::decompress_and_decrypt(
                    &buffer,
                    entry.uncompressed_size as usize,
                    entry.crc32,
                )?;
                Ok(decompressed)
            }
            other => Err(PakError::UnsupportedMethod(other)),
        }
    }

    pub fn extract_entry_from_reader<R: Read + Seek>(
        reader: &mut R,
        entry: &PakEntry,
    ) -> Result<Vec<u8>, PakError> {
        let raw = Self::extract_entry_raw_from_reader(reader, entry)?;
        Ok(Self::decode_content(raw, entry, false).1)
    }

    /// Determines the content type of an entry using exactly the decoding path of extraction.
    pub fn classify_entry_from_reader<R: Read + Seek>(
        reader: &mut R,
        entry: &PakEntry,
    ) -> Result<Option<ContentKind>, PakError> {
        let raw = Self::extract_entry_raw_from_reader(reader, entry)?;
        Ok(Self::decode_content(raw, entry, true).0)
    }

    /// Single source of truth for content decoding: returns the special kind (if any) and the output bytes.
    fn decode_content(raw: Vec<u8>, entry: &PakEntry, only_validate_header: bool) -> (Option<ContentKind>, Vec<u8>) {
        if xml::is_binary_xml(&raw) {
            match xml::decode_binary_xml(&raw, only_validate_header) {
                Ok(xml_bytes) => {
                    return (Some(ContentKind::BinaryXml), xml_bytes);
                }
                Err(BinaryXmlError::ParsingStoppedByValidator) => {
                    return (Some(ContentKind::BinaryXml), raw);
                }
                _ => {}
            }
        }
        if html::is_encrypted_html(&raw) {
            match html::decrypt_html(&raw, &entry.raw_name, only_validate_header)
                .or_else(|_| html::decrypt_html(&raw, &entry.path, only_validate_header)) {
                Ok(html_bytes) => {
                    return (Some(ContentKind::EncryptedHtml), html_bytes);
                }
                Err(HtmlDecryptError::ParsingStoppedByValidator) => {
                    return (Some(ContentKind::EncryptedHtml), raw);
                }
                _ => {}
            }
        }
        (None, raw)
    }

    pub fn extract_entry_raw(&self, entry: &PakEntry) -> Result<Vec<u8>, PakError> {
        if let Some(bytes) = &self.bytes {
            let mut cursor = io::Cursor::new(bytes);
            Self::extract_entry_raw_from_reader(&mut cursor, entry)
        } else {
            Err(PakError::Io(io::Error::new(
                io::ErrorKind::Other,
                "In-memory bytes not available",
            )))
        }
    }

    pub fn extract_entry(&self, entry: &PakEntry) -> Result<Vec<u8>, PakError> {
        if let Some(bytes) = &self.bytes {
            let mut cursor = io::Cursor::new(bytes);
            Self::extract_entry_from_reader(&mut cursor, entry)
        } else {
            Err(PakError::Io(io::Error::new(
                io::ErrorKind::Other,
                "In-memory bytes not available",
            )))
        }
    }

    pub fn extract_raw(&self, entry_path: &str) -> Result<Vec<u8>, PakError> {
        let entry = self.find_entry(entry_path)?;
        self.extract_entry_raw(entry)
    }

    pub fn extract(&self, entry_path: &str) -> Result<Vec<u8>, PakError> {
        let entry = self.find_entry(entry_path)?;
        self.extract_entry(entry)
    }

    fn find_entry(&self, entry_path: &str) -> Result<&PakEntry, PakError> {
        let target = Self::sanitize_path(entry_path);
        let entry = self
            .entries
            .iter()
            .find(|e| e.path == target || e.raw_name == entry_path)
            .or_else(|| {
                self.entries.iter().find(|e| {
                    e.path.eq_ignore_ascii_case(&target)
                        || e.raw_name.eq_ignore_ascii_case(entry_path)
                })
            })
            .ok_or_else(|| PakError::EntryNotFound(entry_path.to_string()))?;
        Ok(entry)
    }

    pub fn file_names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.path.clone()).collect()
    }
}
