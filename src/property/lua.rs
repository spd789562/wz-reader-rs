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
    offset: usize,
    length: usize,
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
        let len = data.len();
        let mut decoded = Vec::<u8>::with_capacity(len);

        // check and release the lock, it will be block when the lock is not released(maybe is held by other thread)
        let is_need_mut = !self.reader.keys.read().unwrap().is_enough(len);

        if is_need_mut {
            let mut key = self.reader.keys.write().unwrap();
            key.ensure_key_size(data.len()).unwrap();
        }

        let key = self.reader.keys.read().unwrap();

        for (i, byte) in data.iter().enumerate() {
            let k = key.try_at(i).unwrap_or(&0_u8);
            decoded.push(*byte ^ k);
        }

        String::from_utf8(decoded).map_err(WzLuaParseError::from)
    }
}