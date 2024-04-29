use std::sync::Arc;
use crate::{ util, WzNode, WzNodeArc, WzNodeArcVec, WzNodeName, WzReader };
use crate::property::WzLua;
use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[derive(Debug, Error)]
pub enum WzImageParseError {
    #[error(transparent)]
    FileError(#[from] std::io::Error),
    #[error("Lua parse error")]
    LuaParseError,
    #[error("parse as wz image failed, pos at {0}")]
    ParseError(usize),
    #[error("New Wz image header found. b = {0}, offset = {1}")]
    UnknownImageHeader(u8, usize),
    #[error(transparent)]
    ParsePropertyListError(#[from] util::WzPropertyParseError),
    #[error("Binary reading error")]
    ReaderError(#[from] scroll::Error),
    #[error("Not a Image object")]
    NotImageObject,
}

pub const WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET: u8 = 0x73;
pub const WZ_IMAGE_HEADER_BYTE_WITH_OFFSET: u8 = 0x1B;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Debug, Clone, Default)]
pub struct WzImage {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub name: WzNodeName,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: bool,
}

impl WzImage {
    pub fn new(name: &WzNodeName, offset: usize, block_size: usize, reader: &Arc<WzReader>) -> Self {
        Self {
            reader: Arc::clone(reader),
            name: name.clone(),
            offset,
            block_size,
            is_parsed: false,
        }
    }
    pub fn from_file(path: &str, wz_iv: [u8; 4]) -> Result<Self, WzImageParseError> {
        let name = std::path::Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
        let file = std::fs::File::open(path)?;
        let map = unsafe { memmap2::Mmap::map(&file)? };

        let block_size = map.len();
        let reader = WzReader::new(map).with_iv(wz_iv);

        Ok(WzImage {
            reader: Arc::new(reader),
            name: name.into(),
            offset: 0,
            block_size,
            is_parsed: false
        })
    }

    /// Direct get child node inside `WzImage` without parsing the whole `WzImage`. Sometimes
    /// we just need a single node in `WzImage`, but don't want to parse it and
    /// unparse later, it waste time and memory.
    pub fn at_path(&self, path: &str) -> Result<WzNodeArc, WzImageParseError> {
        let reader = self.reader.create_slice_reader_without_hash();

        reader.seek(self.offset);
        let header_byte = reader.read_u8()?;

        if header_byte != WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET {
            return Err(WzImageParseError::UnknownImageHeader(header_byte, reader.pos.get()));
        } else {
            let name = reader.read_wz_string()?;
            let value = reader.read_u16()?;
            if name != "Property" && value != 0 {
                return Err(WzImageParseError::ParseError(reader.pos.get()));
            }
        }

        let result = util::get_node(path, &self.reader, &reader, self.offset);

        match result {
            Ok(node) => Ok(node.1),
            Err(e) => Err(WzImageParseError::from(e)),
        }
    }

    pub fn resolve_children(&self, parent: &WzNodeArc) -> Result<WzNodeArcVec, WzImageParseError> {
        let reader = self.reader.create_slice_reader_without_hash();

        reader.seek(self.offset);

        let header_byte = reader.read_u8()?;

        let mut childrens: WzNodeArcVec = Vec::new();

        match header_byte {
            0x1 => {
                if self.name.ends_with(".lua") {
                    let len = reader.read_wz_int()?;
                    let offset = reader.pos.get();
    
                    let name: WzNodeName = String::from("Script").into();

                    let wz_lua = WzLua::new(
                        &self.reader,
                        offset,
                        len as usize
                    );

                    let lua_node = WzNode::new(
                        &name, 
                        wz_lua,
                        Some(parent)
                    );

                    childrens.push((name, lua_node.into_lock()));

                    return Ok(childrens);
                }
                return Err(WzImageParseError::LuaParseError)
            },
            WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET => {
                let name = reader.read_wz_string()?;
                let value = reader.read_u16()?;
                if name != "Property" && value != 0 {
                    return Err(WzImageParseError::ParseError(reader.pos.get()));
                }
            },
            _ => {
                return Err(WzImageParseError::UnknownImageHeader(header_byte, reader.pos.get()));
            }
        }

        util::parse_property_list(parent, &self.reader, &reader, self.offset)
            .map_err(WzImageParseError::from)
    }
}

pub fn is_lua_image(name: &str) -> bool {
    name.ends_with(".lua")
}
pub fn is_valid_wz_image(check_byte: u8) -> bool {
    check_byte == WZ_IMAGE_HEADER_BYTE_WITH_OFFSET || check_byte == WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET
}