use crate::{
    directory, reader, version, Reader, SharedWzMutableKey, WzDirectory, WzNodeArc, WzNodeArcVec,
    WzNodeCast, WzObjectType, WzReader, WzSliceReader,
};
use memmap2::Mmap;
use std::fs::File;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    FileError(#[from] std::io::Error),
    #[error("invald wz file")]
    InvalidWzFile,
    #[error("Error with game version hash : The specified game version is incorrect and WzLib was unable to determine the version itself")]
    ErrorGameVerHash,
    #[error("Failed, in this case the causes are undetermined.")]
    FailedUnknown,
    #[error("Binary reading error")]
    ReaderError(#[from] reader::Error),
    #[error(transparent)]
    DirectoryError(#[from] directory::Error),
    #[error("[WzFile] New Wz image header found. checkByte = {0}, File Name = {1}")]
    UnknownImageHeader(u8, String),
    #[error("Unable to guess version")]
    UnableToGuessVersion,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct WzFileMeta {
    /// path of wz file
    pub path: String,
    /// the wz file's patch version, if not set, try to guess from wz file
    pub patch_version: i32,
    /// a.k.a encver
    pub wz_version_header: i32,
    /// a wz file is cantain wz_version_header(encver) in header
    pub wz_with_encrypt_version_header: bool,
    /// the hash use to calculate img offset
    pub hash: usize,
}

/// Root of the `WzNode`, represents the Wz file itself and contains `WzFileMeta`
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default)]
pub struct WzFile {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: Mutex<bool>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub wz_file_meta: Arc<RwLock<WzFileMeta>>,
}

impl WzFile {
    pub fn from_file<P>(
        path: P,
        wz_iv: Option<[u8; 4]>,
        patch_version: Option<i32>,
        existing_key: Option<&SharedWzMutableKey>,
    ) -> Result<WzFile, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file: File = File::open(&path)?;
        let map = unsafe { Mmap::map(&file)? };

        let block_size = map.len();

        let wz_iv = if let Some(iv) = wz_iv {
            // consider do version::verify_iv_from_wz_file here like WzImage does, but feel like it's not necessary
            iv
        } else {
            version::guess_iv_from_wz_file(&map).ok_or(Error::UnableToGuessVersion)?
        };

        let reader = if let Some(keys) = existing_key {
            WzReader::new(map)
                .with_iv(wz_iv)
                .with_existing_keys(keys.clone())
        } else {
            WzReader::new(map).with_iv(wz_iv)
        };

        let offset = reader.get_wz_fstart().map_err(|_| Error::InvalidWzFile)? + 2;

        let wz_file_meta = WzFileMeta {
            path: path.as_ref().to_str().unwrap().to_string(),
            patch_version: patch_version.unwrap_or(-1),
            wz_version_header: 0,
            wz_with_encrypt_version_header: true,
            hash: 0,
        };

        Ok(WzFile {
            offset: offset as usize,
            block_size,
            is_parsed: Mutex::new(false),
            reader: Arc::new(reader),
            wz_file_meta: Arc::new(RwLock::new(wz_file_meta)),
        })
    }
    pub fn parse(
        &self,
        parent: &WzNodeArc,
        patch_version: Option<i32>,
    ) -> Result<WzNodeArcVec, Error> {
        let reader = self.reader.clone();

        let mut wz_file_meta = WzFileMeta {
            path: "".to_string(),
            patch_version: patch_version.unwrap_or(self.wz_file_meta.read().unwrap().patch_version),
            wz_version_header: 0,
            wz_with_encrypt_version_header: true,
            hash: 0,
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
            let guess_range: Range<i32> = if wz_with_encrypt_version_header {
                1..2000
            } else {
                /* not hold encver in wz_file, directly try 770 - 780 */
                WZ_VERSION_HEADER_64BIT_START as i32..WZ_VERSION_HEADER_64BIT_START as i32 + 10
            };

            /* there has code in maplelib to detect version from maplestory.exe here */

            for ver_to_decode in guess_range {
                wz_file_meta.hash =
                    check_and_get_version_hash(wz_file_meta.wz_version_header, ver_to_decode)
                        as usize;
                if let Ok(childs) = self.try_decode_with_wz_version_number(
                    parent,
                    &slice_reader,
                    &wz_file_meta,
                    ver_to_decode,
                ) {
                    wz_file_meta.patch_version = ver_to_decode;
                    self.update_wz_file_meta(wz_file_meta);
                    return Ok(childs);
                }
            }

            return Err(Error::ErrorGameVerHash);
        }

        wz_file_meta.hash =
            check_and_get_version_hash(wz_file_meta.wz_version_header, wz_file_meta.patch_version)
                as usize;

        let childs = self.try_decode_with_wz_version_number(
            parent,
            &slice_reader,
            &wz_file_meta,
            wz_file_meta.patch_version,
        )?;
        self.update_wz_file_meta(wz_file_meta);

        Ok(childs)
    }

    fn try_decode_with_wz_version_number(
        &self,
        parent: &WzNodeArc,
        reader: &WzSliceReader,
        meta: &WzFileMeta,
        use_maplestory_patch_version: i32,
    ) -> Result<WzNodeArcVec, Error> {
        if meta.hash == 0 {
            return Err(Error::ErrorGameVerHash);
        }

        let node = WzDirectory::new(self.offset, self.block_size, &self.reader, false)
            .with_hash(meta.hash);

        node.verify_hash()?;

        let childs = node.resolve_children(parent)?;

        let first_image_node = childs
            .iter()
            .find(|(_, node)| matches!(node.object_type, WzObjectType::Image(_)));

        if let Some((name, image_node)) = first_image_node {
            let offset = image_node
                .try_as_image()
                .map(|node| node.offset)
                .ok_or(Error::ErrorGameVerHash)?;

            let check_byte = reader
                .read_u8_at(offset)
                .map_err(|_| Error::ErrorGameVerHash)?;

            match check_byte {
                0x73 | 0x1b | 0x01 => {}
                _ => {
                    /* 0x30, 0x6C, 0xBC */
                    println!(
                        "UnknownImageHeader: check_byte = {}, File Name = {}",
                        check_byte, name
                    );
                    return Err(Error::UnknownImageHeader(check_byte, name.to_string()));
                }
            }
        }

        // there a special case this 2 will match
        if !meta.wz_with_encrypt_version_header && use_maplestory_patch_version == 113 {
            return Err(Error::ErrorGameVerHash);
        }

        Ok(childs)
    }

    fn update_wz_file_meta(&self, wz_file_meta: WzFileMeta) {
        let mut old_meta = self.wz_file_meta.write().unwrap();
        *old_meta = WzFileMeta {
            path: std::mem::take(&mut old_meta.path),
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
        return version_hash;
    }

    let enc = 0xff
        ^ (version_hash >> 24) & 0xff
        ^ (version_hash >> 16) & 0xff
        ^ (version_hash >> 8) & 0xff
        ^ version_hash & 0xff;

    if enc == encver {
        version_hash
    } else {
        0
    }
}
