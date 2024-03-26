use std::ops::Deref;
use crate::{ property::WzPropertyType, util, NodeMethods, Reader, WzObjectType, WzReader };
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzImageParseError {
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

pub fn is_lua_image(name: &str) -> bool {
    name.ends_with(".lua")
}
pub fn is_valid_wz_image(check_byte: u8) -> bool {
    check_byte == WZ_IMAGE_HEADER_BYTE_WITH_OFFSET || check_byte == WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET
}

pub fn parse_wz_image<R: Deref<Target = WzReader> + Clone, Node: NodeMethods<Node = Node, Reader = R> + Clone>(wz_node: &Node) -> Result<(), WzImageParseError> {
    let reader = if let Some(reader) = wz_node.get_reader() {
        reader
    } else {
        panic!("Reader not found in wz_img node")
    };

    let reader = reader.create_slice_reader_without_hash();

    let node_offset = wz_node.get_offset();

    reader.seek(node_offset);

    let header_byte = reader.read_u8()?;

    match header_byte {
        0x1 => {
            if wz_node.get_name().ends_with(".lua") {
                let len = reader.read_wz_int()?;
                let offset = reader.get_pos();

                let node = Node::new_with_parent(
                    wz_node,
                    WzObjectType::Property,
                    Some(WzPropertyType::Lua),
                    String::from("Script"),
                    offset,
                    len as usize
                );
                
                wz_node.add_node_child(node);

                return Ok(());
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

    match util::parse_property_list::<Node>(wz_node, &reader, reader.get_pos(), node_offset) {
        Ok(_) => Ok(()),
        Err(e) => Err(WzImageParseError::from(e))
    }
}