use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzStringParseError {
    #[error(transparent)]
    ParseError(#[from] scroll::Error),

    #[error("Not a String property")]
    NotStringProperty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WzStringType {
    Ascii,
    Unicode,
    Empty,
}

#[derive(Debug, Clone)]
pub struct WzStringMeta {
    /// string start offset
    pub offset: usize,
    /// string length
    pub length: u32,
    pub string_type: WzStringType,
}

impl WzStringMeta {
    pub fn new(offset: usize, length: u32, string_type: WzStringType) -> Self {
        Self {
            offset,
            length,
            string_type,
        }
    }
    pub fn empty() -> Self {
        Self {
            offset: 0,
            length: 0,
            string_type: WzStringType::Empty,
        }
    }
    pub fn new_ascii(offset: usize, length: u32) -> Self {
        Self {
            offset,
            length,
            string_type: WzStringType::Ascii,
        }
    }
    pub fn new_unicode(offset: usize, length: u32) -> Self {
        Self {
            offset,
            length,
            string_type: WzStringType::Unicode,
        }
    }
}