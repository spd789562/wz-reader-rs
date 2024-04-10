use std::sync::Arc;
use crate::{ property::{WzLua, WzValue}, util, Reader, WzNode, WzNodeArc, WzObjectType, WzReader };
use thiserror::Error;

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


#[derive(Debug, Clone)]
pub struct WzImage {
    pub reader: Arc<WzReader>,
    pub name: String,
    pub offset: usize,
    pub block_size: usize,
    pub is_parsed: bool,
}

impl WzImage {
    pub fn new(name: String, offset: usize, block_size: usize, reader: &Arc<WzReader>) -> Self {
        Self {
            reader: Arc::clone(reader),
            name,
            offset,
            block_size,
            is_parsed: false,
        }
    }
    pub fn from_file(path: &str, wz_iv: [u8; 4]) -> Result<Self, WzImageParseError> {
        let name = std::path::Path::new(path).file_stem().unwrap().to_str().unwrap().to_string();
        let file = std::fs::File::open(path)?;
        let map = unsafe { memmap2::Mmap::map(&file).unwrap() };

        let block_size = map.len();
        let reader = WzReader::new(map).with_iv(wz_iv);

        Ok(WzImage {
            reader: Arc::new(reader),
            name,
            offset: 0,
            block_size,
            is_parsed: false
        })
    }

    pub fn resolve_children(&self, parent: &WzNodeArc) -> Result<Vec<(String, WzNodeArc)>, WzImageParseError> {
        let reader = self.reader.create_slice_reader_without_hash();

        reader.seek(self.offset);

        let header_byte = reader.read_u8()?;

        let mut childrens: Vec<(String, WzNodeArc)> = Vec::new();

        match header_byte {
            0x1 => {
                if self.name.ends_with(".lua") {
                    let len = reader.read_wz_int()?;
                    let offset = reader.get_pos();
    
                    let name = String::from("Script");

                    let wz_lua = WzLua::new(
                        &self.reader,
                        offset,
                        len as usize
                    );

                    let lua_node = WzNode::new(
                        name.clone(), 
                        WzObjectType::Value(WzValue::Lua(wz_lua)),
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
                    return Err(WzImageParseError::ParseError(reader.get_pos()));
                }
            },
            _ => {
                return Err(WzImageParseError::UnknownImageHeader(header_byte, reader.get_pos()));
            }
        }

        util::parse_property_list(parent, &self.reader, &reader, reader.get_pos(), self.offset)
            .map_err(WzImageParseError::from)
    }
}

pub fn is_lua_image(name: &str) -> bool {
    name.ends_with(".lua")
}
pub fn is_valid_wz_image(check_byte: u8) -> bool {
    check_byte == WZ_IMAGE_HEADER_BYTE_WITH_OFFSET || check_byte == WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET
}