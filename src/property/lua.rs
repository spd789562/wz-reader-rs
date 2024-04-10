use std::sync::Arc;
use crate::WzReader;
use thiserror::Error;



#[derive(Debug, Error)]
pub enum WzLuaParseError {
    #[error("Lua decode fail")]
    StringDecodeFail(#[from] std::string::FromUtf8Error),

    #[error("Not a Lua property")]
    NotLuaProperty,
}

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

    pub fn extract_lua(&self) -> Result<String, WzLuaParseError> {
        let data = self.reader.get_slice(self.offset..self.length + self.offset);
        let mut decoded = Vec::<u8>::with_capacity(data.len());
        let mut key = self.reader.keys.lock().unwrap();

        key.ensure_key_size(data.len()).unwrap();

        for (i, byte) in data.iter().enumerate() {
            let k = key.at(i);
            decoded.push(*byte ^ k);
        }

        String::from_utf8(decoded).map_err(WzLuaParseError::from)
    }
}