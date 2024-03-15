use thiserror::Error;

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