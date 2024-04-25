use std::sync::Arc;
use crate::{WzReader, Reader, WzNodeArc, WzNodeCast};
use thiserror::Error;


#[derive(Debug, Error)]
pub enum WzStringParseError {
    #[error("Error parsing WzString: {0}")]
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

/// `WzString` only hold the string information.
#[derive(Debug, Clone)]
pub struct WzString {
    reader: Arc<WzReader>,
    /// string start offset
    offset: usize,
    /// string length
    length: u32,
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

impl WzString {
    pub fn from_meta(meta: WzStringMeta, reader: &Arc<WzReader>) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset: meta.offset,
            length: meta.length,
            string_type: meta.string_type,
        }
    }
    /// Decode string from wz file.
    pub fn get_string(&self) -> Result<String, WzStringParseError> {
        self.reader.resolve_wz_string_meta(&self.string_type, self.offset, self.length as usize).map_err(WzStringParseError::from)
    }
}

/// A helper function to resolve string from `WzNodeArc`.
pub fn resolve_string_from_node(node: &WzNodeArc) -> Result<String, WzStringParseError> {
    node.read().unwrap().try_as_string()
        .ok_or(WzStringParseError::NotStringProperty)
        .and_then(|string| string.get_string())
}