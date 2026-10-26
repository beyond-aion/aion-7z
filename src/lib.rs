#![allow(unsafe_op_in_unsafe_fn)]

pub mod archive;
pub mod html;
pub mod pak;
pub mod xml;

#[cfg(windows)]
pub mod sevenzip;

pub use archive::{PakArchive, PakEntry, PakError};
pub use html::{HtmlDecryptError};
pub use xml::{BinaryXmlError};
