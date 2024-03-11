use std::ops::Deref;
use crate::{ Reader, WzObjectType, WzReader };
use crate::node::NodeMethods;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzDirectoryParseError {
    #[error("Lua parse error")]
    LuaParseError,
    #[error("parse as wz image failed, pos at {0}")]
    ParseError(usize),
    #[error("New Wz image header found. b = {0}, offset = {1}")]
    UnknownWzDirectoryType(u8, usize),
    #[error("Invalid wz version used for decryption, try parsing other version numbers.")]
    InvalidWzVersion,
    #[error("Entry count overflow, Invalid wz version used for decryption, try parsing other version numbers.")]
    InvalidEntryCount,
    #[error("Binary reading error")]
    ReaderError(#[from] scroll::Error),
}

#[derive(Debug)]
pub enum WzDirectoryType {
    UnknownType,
    RetrieveStringFromOffset,
    WzDirectory,
    WzImage,
    NewUnknownType,
}

pub fn get_wz_directory_type_from_byte(byte: u8) -> WzDirectoryType {
    match byte {
        1 => WzDirectoryType::UnknownType,
        2 => WzDirectoryType::RetrieveStringFromOffset,
        3 => WzDirectoryType::WzDirectory,
        4 => WzDirectoryType::WzImage,
        _ => WzDirectoryType::UnknownType,
    }
}

pub fn parse_wz_directory<R: Deref<Target = WzReader>, Node: NodeMethods<Node = Node, Reader = R> + Clone>(wz_node: &Node) -> Result<(), WzDirectoryParseError> {
    let origin_reader = if let Some(reader) = wz_node.get_reader() {
        reader
    } else {
        panic!("Reader not found in wz_directory node")
    };

    let reader = origin_reader.create_slice_reader();
    
    let node_offset = wz_node.get_offset();
    
    reader.seek(node_offset);


    let entry_count = reader.read_wz_int()?;

    // println!("entry_count: {}", entry_count);

    if !(0..=1000000).contains(&entry_count) {
        return Err(WzDirectoryParseError::InvalidEntryCount);
    }

    for _ in 0..entry_count {
        let dir_byte = reader.read_u8()?;
        let mut dir_type = get_wz_directory_type_from_byte(dir_byte);

        let fname: String;

        match dir_type {
            WzDirectoryType::UnknownType => {
                /* unknown, just skip this chunk */
                reader.skip(4 + 4 + 2);
                continue;
            }
            WzDirectoryType::RetrieveStringFromOffset => {
                let str_offset = reader.read_i32().unwrap();
                
                let pos = reader.get_pos();

                let offset = reader.header.fstart + str_offset as usize;

                reader.seek(offset);

                dir_type = get_wz_directory_type_from_byte(reader.read_u8().unwrap());
                fname = reader.read_wz_string()?;

                reader.seek(pos);
            }
            WzDirectoryType::WzDirectory |
            WzDirectoryType::WzImage => {
                fname = reader.read_wz_string()?;
            }
            WzDirectoryType::NewUnknownType => {
                println!("NewUnknownType: {}", dir_byte);
                return Err(WzDirectoryParseError::UnknownWzDirectoryType(dir_byte, reader.get_pos()))
            }
        }
        
        let fsize = reader.read_wz_int()?;
        let _checksum = reader.read_wz_int()?;
        let offset = reader.read_wz_offset(None)?;
        let buf_start = offset;
        
        
        let buf_end = buf_start + fsize as usize;

        if !reader.is_valid_pos(buf_end) {
            return Err(WzDirectoryParseError::InvalidWzVersion);
        }

        match dir_type {
            WzDirectoryType::WzDirectory => {
                let node = Node::new_with_parent(wz_node, WzObjectType::Directory, None, fname.clone(), offset, fsize as usize);
                wz_node.add_node_child(node);
            }
            WzDirectoryType::WzImage => {
                let node = Node::new_with_parent(wz_node, WzObjectType::Image, None, fname.clone(), offset, fsize as usize);
                wz_node.add_node_child(node);
            }
            _ => {
                // should never be here
            }
        }

    }

    Ok(())
}