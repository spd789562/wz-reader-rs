use std::ops::Deref;
use crate::{WzReader, WzSliceReader, Reader, NodeMethods, parse_wz_directory};
use crate::arc::WzNodeArc;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzFileParseError {
    #[error("Path is null")]
    PathIsNull,
    #[error("Error with game version hash : The specified game version is incorrect and WzLib was unable to determine the version itself")]
    ErrorGameVerHash,
    #[error("Failed, in this case the causes are undetermined.")]
    FailedUnknown,
    #[error("Reader reading error")]
    ReaderError(#[from] scroll::Error),
    #[error("[WzFile] New Wz image header found. checkByte = {0}, File Name = {1}")]
    UnknownImageHeader(u8, String),
}

#[derive(Debug, Clone)]
pub struct WzFileMeta {
    pub path: String,
    pub name: String,
    pub patch_version: i32,
    pub wz_version_header: i32,
    pub wz_with_encrypt_version_header: bool,
    pub hash: usize
}
const WZ_VERSION_HEADER_64BIT_START: u16 = 770;

pub fn parse_wz_file<R: Deref<Target = WzReader> + Clone, Node: NodeMethods<Node = Node, Reader = R> + Clone>(wz_node: &Node, patch_version: Option<i32>) -> Result<(), WzFileParseError> {
    let reader = if let Some(reader) = wz_node.get_reader() {
        reader
    } else {
        panic!("wz_reader in WzFile should not be None")
    };

    let mut wz_file_meta = WzFileMeta {
        path: "".to_string(),
        name: "".to_string(),
        patch_version: patch_version.unwrap_or(-1),
        wz_version_header: 0,
        wz_with_encrypt_version_header: true,
        hash: 0
    };
    
    let slice_reader = reader.create_slice_reader();

    let (wz_with_encrypt_version_header, encrypt_version) = check_64bit_client(&slice_reader);

    wz_file_meta.wz_version_header = if wz_with_encrypt_version_header {
        encrypt_version as i32
    } else {
        WZ_VERSION_HEADER_64BIT_START as i32
    };

    wz_file_meta.wz_with_encrypt_version_header = wz_with_encrypt_version_header;

    if wz_file_meta.patch_version == -1 {
        /* not hold encver in wz_file, directly try 770 - 780 */
        if !wz_with_encrypt_version_header {
            for ver_to_decode in WZ_VERSION_HEADER_64BIT_START..WZ_VERSION_HEADER_64BIT_START + 10 {
                
                if try_decode_with_wz_version_number(wz_node, &reader, &mut wz_file_meta, ver_to_decode as i32).is_ok() {
                    wz_node.update_wz_file_meta(wz_file_meta);
                    return Ok(());
                }
            }
        }

        /* there has code in maplelib to detect version from maplestory.exe here */

        let max_patch_version = 2000;

        for ver_to_decode in 1..max_patch_version {
            // println!("try_decode_with_wz_version_number: {}", ver_to_decode);
            if try_decode_with_wz_version_number(wz_node, &reader, &mut wz_file_meta, ver_to_decode).is_ok() {
                wz_node.update_wz_file_meta(wz_file_meta);
                return Ok(());
            }
        }

        return Err(WzFileParseError::ErrorGameVerHash);
    }

    wz_file_meta.hash = check_and_get_version_hash(wz_file_meta.wz_version_header, wz_file_meta.patch_version) as usize;
    reader.update_hash(wz_file_meta.hash);

    
    if parse_wz_directory(wz_node).is_err() {
        return Err(WzFileParseError::ErrorGameVerHash);
    }

    wz_node.update_wz_file_meta(wz_file_meta);

    Ok(())
}

fn try_decode_with_wz_version_number<R: Deref<Target = WzReader>, Node: NodeMethods<Node = Node, Reader = R> + Clone>(
    wz_node: &Node, 
    reader: &R,
    meta: &mut WzFileMeta,
    use_maplestory_patch_version: i32
) -> Result<(), WzFileParseError> {
    let version_hash = check_and_get_version_hash(meta.wz_version_header, use_maplestory_patch_version) as usize;

    if version_hash == 0 {
        return Err(WzFileParseError::ErrorGameVerHash);
    }
    
    meta.hash = version_hash;
    reader.update_hash(version_hash);

    if parse_wz_directory(wz_node).is_err() {
        return Err(WzFileParseError::ErrorGameVerHash);
    }

    let first_image_node = wz_node.first_image();

    if let Some(image_node) = first_image_node {

        let check_byte = if let Ok(b) = reader.read_u8_at(image_node.get_offset()) {
            b
        } else {
            return Err(WzFileParseError::ErrorGameVerHash);
        };

        match check_byte {
            0x73 | 0x1b | 0x01 => {
            },
            _ => {
                let name = wz_node.get_name();
                /* 0x30, 0x6C, 0xBC */
                println!("UnknownImageHeader: check_byte = {}, File Name = {}", check_byte, name);
                return Err(WzFileParseError::UnknownImageHeader(check_byte, name));
            }
        }
    }

    if !meta.wz_with_encrypt_version_header && use_maplestory_patch_version == 113 {
        // return Err("is_64bit_wz_file && patch_version == 113".to_string());
        return Err(WzFileParseError::ErrorGameVerHash);
    }

    meta.patch_version = use_maplestory_patch_version;
    Ok(())
}

fn is_64bit_wz_file(wz_node: &WzNodeArc) -> bool {
    let node = wz_node.read().unwrap();
    if let Some(meta) = &node.wz_file_meta {
        !meta.wz_with_encrypt_version_header
    } else {
        false
    }
}

fn check_64bit_client(wz_reader: &WzSliceReader) -> (bool, u16) {
    let encrypt_version = wz_reader.read_u16_at(wz_reader.header.fstart).unwrap();
    
    if wz_reader.header.fsize >= 2 {
        if encrypt_version > 0xff {
            return (false, 0);
        }
        if encrypt_version == 0x80 {
            let prop_count = wz_reader.read_i32_at(wz_reader.header.fstart).unwrap();
            if prop_count > 0 && (prop_count & 0xff) == 0 && prop_count <= 0xffff {
                return (false, 0);
            }
        }
        /* the only place return actual encrypt_version */
        return (true, encrypt_version);
    }

    (false, 0)
}

fn check_and_get_version_hash(encver: i32, patch_version: i32) -> i32 {
    let mut version_hash: i32 = 0;

    let bind_version = &patch_version.to_string();

    for i in bind_version.chars() {
        let char_code = i.to_ascii_lowercase() as i32;

        // version_hash * 2^5 + char_code + 1
        version_hash = version_hash * 32 + char_code + 1;
    }
    
    if encver == patch_version {
        return version_hash
    }

    let enc = 0xff ^
        (version_hash >> 24) & 0xff ^
        (version_hash >> 16) & 0xff ^
        (version_hash >> 8) & 0xff ^
        version_hash & 0xff;

    if enc == encver {
        version_hash
    } else {
        0
    }
}