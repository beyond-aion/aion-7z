use std::io::{self, Write};

pub const BINARY_XML_MARKER: u8 = 0x80;

#[derive(Debug)]
pub enum BinaryXmlError {
    Io(io::Error),
    NotBinaryXml,
    ParsingStoppedByValidator,
    UnexpectedEof,
    InvalidData(&'static str),
}

impl From<io::Error> for BinaryXmlError {
    fn from(err: io::Error) -> Self {
        BinaryXmlError::Io(err)
    }
}

impl std::fmt::Display for BinaryXmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryXmlError::Io(e) => write!(f, "I/O error: {}", e),
            BinaryXmlError::NotBinaryXml => write!(f, "Not a binary XML stream (marker 0x80 missing)"),
            BinaryXmlError::ParsingStoppedByValidator => write!(f, "Parsing stopped by validator"),
            BinaryXmlError::UnexpectedEof => write!(f, "Unexpected end of data in binary XML"),
            BinaryXmlError::InvalidData(msg) => write!(f, "Invalid binary XML data: {}", msg),
        }
    }
}

impl std::error::Error for BinaryXmlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BinaryXmlError::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub text: Option<String>,
    pub children: Vec<XmlElement>,
}

impl XmlElement {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.push((key.into(), value.into()));
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn with_child(mut self, child: XmlElement) -> Self {
        self.children.push(child);
        self
    }

    pub fn to_xml_string(&self) -> String {
        let mut buffer = Vec::new();
        let _ = self.write_xml(&mut buffer);
        String::from_utf8_lossy(&buffer).into_owned()
    }

    pub fn write_xml<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
        write_element(self, 0, writer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlDocument {
    pub root: XmlElement,
}

impl XmlDocument {
    pub fn new(root: XmlElement) -> Self {
        Self { root }
    }

    pub fn to_xml_string(&self) -> String {
        self.root.to_xml_string()
    }

    pub fn write_xml<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        self.root.write_xml(writer)
    }
}

pub fn is_binary_xml(data: &[u8]) -> bool {
    data.first().copied() == Some(BINARY_XML_MARKER)
}

pub fn read_packed_int(cursor: &mut &[u8]) -> Result<usize, BinaryXmlError> {
    let mut num2: usize = 0;
    let mut num3: u32 = 0;
    loop {
        if cursor.is_empty() {
            return Err(BinaryXmlError::UnexpectedEof);
        }
        let num1 = cursor[0] as usize;
        *cursor = &cursor[1..];
        if num1 < 128 {
            return Ok(num2 | (num1 << num3));
        }
        num2 |= (num1 & 0x7F) << num3;
        num3 += 7;
        if num3 > 35 {
            return Err(BinaryXmlError::InvalidData("Integer overflow in packed integer"));
        }
    }
}

pub fn write_packed_int<W: Write>(writer: &mut W, mut val: usize) -> io::Result<()> {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
            writer.write_all(&[byte])?;
        } else {
            writer.write_all(&[byte])?;
            break;
        }
    }
    Ok(())
}

pub fn read_utf16z(data: &[u8], start_index: usize) -> String {
    if start_index >= data.len() {
        return String::new();
    }
    let mut end_index = start_index;
    while end_index + 1 < data.len() && (data[end_index] != 0 || data[end_index + 1] != 0) {
        end_index += 2;
    }
    if end_index == start_index {
        return String::new();
    }
    let iter = data[start_index..end_index]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]));
    std::char::decode_utf16(iter)
        .map(|r| r.unwrap_or(std::char::REPLACEMENT_CHARACTER))
        .collect()
}

fn next_string(cursor: &mut &[u8], string_table: &[u8]) -> Result<String, BinaryXmlError> {
    let index = read_packed_int(cursor)?;
    if index == 0 {
        Ok(String::new())
    } else {
        Ok(read_utf16z(string_table, index * 2))
    }
}

fn read_element(cursor: &mut &[u8], string_table: &[u8]) -> Result<XmlElement, BinaryXmlError> {
    let name = next_string(cursor, string_table)?;
    if cursor.is_empty() {
        return Err(BinaryXmlError::UnexpectedEof);
    }
    let types = cursor[0];
    *cursor = &cursor[1..];

    let text = if (types & 1) != 0 {
        let t = next_string(cursor, string_table)?;
        Some(t)
    } else {
        None
    };

    let mut attributes = Vec::new();
    if (types & 2) != 0 {
        let attribute_count = read_packed_int(cursor)?;
        attributes.reserve(attribute_count);
        for _ in 0..attribute_count {
            let key = next_string(cursor, string_table)?;
            let value = next_string(cursor, string_table)?;
            attributes.push((key, value));
        }
    }

    let mut children = Vec::new();
    if (types & 4) != 0 {
        let child_elements = read_packed_int(cursor)?;
        children.reserve(child_elements);
        for _ in 0..child_elements {
            let child = read_element(cursor, string_table)?;
            children.push(child);
        }
    }

    Ok(XmlElement {
        name,
        attributes,
        text,
        children,
    })
}

pub fn parse_binary_xml(data: &[u8], only_validate_header: bool) -> Result<XmlDocument, BinaryXmlError> {
    if data.is_empty() || data[0] != BINARY_XML_MARKER {
        return Err(BinaryXmlError::NotBinaryXml);
    }
    let mut cursor = &data[1..];
    let string_table_len = read_packed_int(&mut cursor)?;
    if cursor.len() < string_table_len {
        return Err(BinaryXmlError::UnexpectedEof);
    }
    if only_validate_header {
        return Err(BinaryXmlError::ParsingStoppedByValidator);
    }
    let string_table = &cursor[..string_table_len];
    cursor = &cursor[string_table_len..];

    let root = read_element(&mut cursor, string_table)?;
    Ok(XmlDocument::new(root))
}

pub fn decode_binary_xml(data: &[u8], only_validate_header: bool) -> Result<Vec<u8>, BinaryXmlError> {
    let doc = parse_binary_xml(data, only_validate_header)?;
    let mut out = Vec::new();
    doc.write_xml(&mut out)?;
    Ok(out)
}

pub fn decode_binary_xml_to_string(data: &[u8]) -> Result<String, BinaryXmlError> {
    let doc = parse_binary_xml(data, false)?;
    Ok(doc.to_xml_string())
}

fn write_escaped<W: Write>(writer: &mut W, s: &str, escape_quotes: bool) -> io::Result<()> {
    let mut last = 0;
    for (i, b) in s.bytes().enumerate() {
        let esc = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' if escape_quotes => "&quot;",
            b'\'' if escape_quotes => "&apos;",
            _ => continue,
        };
        writer.write_all(s[last..i].as_bytes())?;
        writer.write_all(esc.as_bytes())?;
        last = i + 1;
    }
    if last < s.len() {
        writer.write_all(s[last..].as_bytes())?;
    }
    Ok(())
}

fn write_indent<W: Write>(writer: &mut W, indent: usize) -> io::Result<()> {
    const SPACES: &[u8] = b"                                "; // 32 spaces
    let mut pending = indent * 2;
    while pending > 0 {
        let chunk = pending.min(SPACES.len());
        writer.write_all(&SPACES[..chunk])?;
        pending -= chunk;
    }
    Ok(())
}

fn write_element<W: Write>(elem: &XmlElement, indent: usize, writer: &mut W) -> io::Result<()> {
    write_indent(writer, indent)?;
    write!(writer, "<{}", elem.name)?;

    for (k, v) in &elem.attributes {
        write!(writer, " {}=\"", k)?;
        write_escaped(writer, v, true)?;
        writer.write_all(b"\"")?;
    }

    let has_text = elem.text.as_ref().map_or(false, |t| !t.is_empty());
    let has_children = !elem.children.is_empty();

    if !has_text && !has_children {
        writeln!(writer, "/>")?;
    } else if has_text && !has_children {
        let text = elem.text.as_ref().unwrap();
        writer.write_all(b">")?;
        write_escaped(writer, text, false)?;
        writeln!(writer, "</{}>", elem.name)?;
    } else {
        writer.write_all(b">\n")?;
        if has_text {
            let text = elem.text.as_ref().unwrap();
            write_indent(writer, indent + 1)?;
            write_escaped(writer, text, false)?;
            writer.write_all(b"\n")?;
        }
        for child in &elem.children {
            write_element(child, indent + 1, writer)?;
        }
        write_indent(writer, indent)?;
        writeln!(writer, "</{}>", elem.name)?;
    }
    Ok(())
}
