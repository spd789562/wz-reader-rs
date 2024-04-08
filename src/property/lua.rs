use std::sync::Arc;
use crate::WzReader;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct WzLua {
    reader: Arc<WzReader>,
    pub offset: usize,
    pub length: usize,
}

impl WzLua {
    pub fn new(reader: &Arc<WzReader>, offset: usize, length: usize) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset,
            length,
        }
    }
}

#[derive(Debug, Error)]
pub enum WzLuaParseError {

    #[error("Not a Lua property")]
    NotLuaProperty,
}


/// still not working
pub fn extract_lua(data: &[u8]) -> Result<String, WzLuaParseError> {
    let mut decoded = Vec::<u8>::with_capacity(data.len());
    
    for (i, byte) in data.iter().enumerate() {
        decoded.push(((*byte as usize) ^ (i + 0xAA)) as u8);
    }

    let decoded = String::from_utf8(decoded).map_err(|_| WzLuaParseError::NotLuaProperty)?;

    Ok(decoded)
}