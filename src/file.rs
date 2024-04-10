use std::fs::File;
use std::sync::Arc;
use memmap2::Mmap;
use crate::{Reader, WzDirectory, WzNodeArc, WzNodeArcVec, WzObjectType, WzReader, WzSliceReader};

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
    pub patch_version: i32,
    pub wz_version_header: i32,
    pub wz_with_encrypt_version_header: bool,
    pub hash: usize
}

#[derive(Debug, Clone)]
pub struct WzFile {
    pub reader: Arc<WzReader>,
    pub offset: usize,
    pub block_size: usize,
    pub is_parsed: bool,
    pub wz_file_meta: WzFileMeta,
}

impl WzFile {
    pub fn from_file(path: &str, wz_iv: [u8; 4], patch_version: Option<i32>) -> Result<WzFile, WzFileParseError> {
        let file: File = File::open(path).expect("file not found");
        let map = unsafe { Mmap::map(&file).unwrap() };

        let block_size = map.len();
        let reader = WzReader::new(map).with_iv(wz_iv);

        let offset = reader.get_wz_fstart().unwrap() + 2;

        let wz_file_meta = WzFileMeta {
            path: path.to_string(),
            patch_version: patch_version.unwrap_or(-1),
            wz_version_header: 0,
            wz_with_encrypt_version_header: true,
            hash: 0
        };

        Ok(WzFile {
            offset: offset as usize,
            block_size,
            is_parsed: false,
            reader: Arc::new(reader),
            wz_file_meta
        })
    }
    pub fn parse(&mut self, parent: &WzNodeArc, patch_version: Option<i32>) -> Result<WzNodeArcVec, WzFileParseError> {
        let reader = self.reader.clone();

        let mut wz_file_meta = WzFileMeta {
            path: "".to_string(),
            patch_version: patch_version.unwrap_or(self.wz_file_meta.patch_version),
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
                    wz_file_meta.hash = check_and_get_version_hash(wz_file_meta.wz_version_header, ver_to_decode as i32) as usize;
                    if let Ok(childs) = self.try_decode_with_wz_version_number(parent, &slice_reader, &wz_file_meta, ver_to_decode as i32) {
                        wz_file_meta.patch_version = ver_to_decode as i32;
                        self.update_wz_file_meta(wz_file_meta);
                        self.is_parsed = true;
                        return Ok(childs);
                    }
                }
            }
    
            /* there has code in maplelib to detect version from maplestory.exe here */
    
            let max_patch_version = 2000;
    
            for ver_to_decode in 1..max_patch_version {
                wz_file_meta.hash = check_and_get_version_hash(wz_file_meta.wz_version_header, ver_to_decode) as usize;
                // println!("try_decode_with_wz_version_number: {}", ver_to_decode);
                if let Ok(childs) = self.try_decode_with_wz_version_number(parent, &slice_reader, &wz_file_meta, ver_to_decode) {
                    wz_file_meta.patch_version = ver_to_decode;
                    self.update_wz_file_meta(wz_file_meta);
                    self.is_parsed = true;
                    return Ok(childs);
                }
            }
    
            return Err(WzFileParseError::ErrorGameVerHash);
        }

        Ok(vec![])
    }

    pub fn try_decode_with_wz_version_number(
        &self,
        parent: &WzNodeArc,
        reader: &WzSliceReader,
        meta: &WzFileMeta,
        use_maplestory_patch_version: i32
    ) -> Result<WzNodeArcVec, WzFileParseError> {
        if meta.hash == 0 {
            return Err(WzFileParseError::ErrorGameVerHash);
        }

        let node = WzDirectory::new(
                self.offset,
                self.block_size,
                &self.reader,
                false
            )
            .with_hash(meta.hash);


        let childs = node.resolve_children(parent).map_err(|_| WzFileParseError::ErrorGameVerHash)?;

        let first_image_node = childs.iter().find(|(_, node)| matches!(node.read().unwrap().object_type, WzObjectType::Image(_)));

        if let Some((name, image_node)) = first_image_node {
            let offset = if let WzObjectType::Image(node) = &image_node.read().unwrap().object_type {
                node.offset
            } else {
                return Err(WzFileParseError::ErrorGameVerHash);
            };

            let check_byte = if let Ok(b) = reader.read_u8_at(offset) {
                b
            } else {
                return Err(WzFileParseError::ErrorGameVerHash);
            };

            match check_byte {
                0x73 | 0x1b | 0x01 => {
                },
                _ => {
                    /* 0x30, 0x6C, 0xBC */
                    println!("UnknownImageHeader: check_byte = {}, File Name = {}", check_byte, name);
                    return Err(WzFileParseError::UnknownImageHeader(check_byte, name.to_string()));
                }
            }
        }

        if !meta.wz_with_encrypt_version_header && use_maplestory_patch_version == 113 {
            // return Err("is_64bit_wz_file && patch_version == 113".to_string());
            return Err(WzFileParseError::ErrorGameVerHash);
        }

        Ok(childs)
    }

    fn update_wz_file_meta(&mut self, wz_file_meta: WzFileMeta) {
        self.wz_file_meta = WzFileMeta {
            path: self.wz_file_meta.path.clone(),
            ..wz_file_meta
        };
    }
}

const WZ_VERSION_HEADER_64BIT_START: u16 = 770;

fn check_64bit_client(wz_reader: &WzSliceReader) -> (bool, u16) {
    let encrypt_version = wz_reader.read_u16_at(wz_reader.header.fstart).unwrap();
    
    if wz_reader.header.fsize >= 2 {
        if encrypt_version > 0xff {
            return (false, 0);
        }
        if encrypt_version == 0x80 {
            let prop_count = wz_reader.read_i32_at(wz_reader.header.fstart + 2).unwrap();
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