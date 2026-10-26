pub const ENCRYPTED_HTML_MARKER: u8 = 0x81;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlDecryptError {
    NotEncryptedHtml,
    DecryptionFailed,
    ParsingStoppedByValidator,
}

impl std::fmt::Display for HtmlDecryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HtmlDecryptError::NotEncryptedHtml => {
                write!(f, "Not an encrypted HTML stream (marker 0x81 missing)")
            }
            HtmlDecryptError::DecryptionFailed => {
                write!(f, "HTML decryption failed (inner marker 0x81 verification failed)")
            }
            HtmlDecryptError::ParsingStoppedByValidator => {
                write!(f, "HTML parsing stopped by validator")
            }
        }
    }
}

impl std::error::Error for HtmlDecryptError {}

pub fn is_encrypted_html(data: &[u8]) -> bool {
    data.first().copied() == Some(ENCRYPTED_HTML_MARKER)
}

pub fn extract_stem(file_name: &str) -> &str {
    let base = file_name.rsplit('/').next().unwrap_or(file_name);
    let short = base.rsplit('\\').next().unwrap_or(base);
    match short.rfind('.') {
        Some(dot) => &short[..dot],
        None => short,
    }
}

pub fn decrypt_html(input: &[u8], file_name: &str, only_validate_header: bool) -> Result<Vec<u8>, HtmlDecryptError> {
    if input.is_empty() || input[0] != ENCRYPTED_HTML_MARKER {
        return Err(HtmlDecryptError::NotEncryptedHtml);
    }

    let stem = extract_stem(file_name);
    let mut edx: u32 = 0;
    let mut ebx: u32 = 0;

    for (i, &b) in stem.as_bytes().iter().enumerate() {
        let t = (b as u32 & 0x0F) + i as u32;
        edx = edx.wrapping_add(t);
        ebx = ebx ^ t;
    }

    let payload = &input[1..];
    if payload.is_empty() {
        return Err(HtmlDecryptError::DecryptionFailed);
    }

    // Verify first byte before allocating full buffer
    let mut next_mask = move || {
        edx = edx.wrapping_add(0x1D) ^ ebx;
        ebx = ebx.wrapping_add(3);
        (edx & 0xFF) as u8
    };
    if (payload[0] ^ next_mask()) != ENCRYPTED_HTML_MARKER {
        return Err(HtmlDecryptError::DecryptionFailed);
    }
    if only_validate_header {
        return Err(HtmlDecryptError::ParsingStoppedByValidator);
    }

    let mut decrypted = Vec::with_capacity(payload.len() - 1);
    for &b in &payload[1..] {
        decrypted.push(b ^ next_mask());
    }

    Ok(decrypted)
}
