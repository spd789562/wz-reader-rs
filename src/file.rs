use crate::{
    directory, reader, util, wz_image, SharedWzMutableKey, WzDirectory, WzHeader, WzNodeArc,
    WzNodeArcVec, WzNodeCast, WzObjectType, WzReader,
};
use memmap2::Mmap;
use std::fs::File;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

const WZ_VERSION_HEADER_64BIT_START: u16 = 770;

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
#[derive(Debug, Clone, Default)]
pub struct WzFile {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: bool,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub wz_file_meta: WzFileMeta,
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
            util::version::guess_iv_from_wz_file(&map).ok_or(Error::UnableToGuessVersion)?
        };

        let reader = if let Some(keys) = existing_key {
            WzReader::new(map)
                .with_iv(wz_iv)
                .with_existing_keys(keys.clone())
        } else {
            WzReader::new(map).with_iv(wz_iv)
        };

        let offset = WzHeader::read_data_start(&reader.map).map_err(|_| Error::InvalidWzFile)?;

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
            is_parsed: false,
            reader: Arc::new(reader),
            wz_file_meta,
        })
    }
    pub fn parse(
        &mut self,
        parent: &WzNodeArc,
        patch_version: Option<i32>,
    ) -> Result<WzNodeArcVec, Error> {
        let reader = self.reader.clone();

        let slice_reader = reader.create_slice_reader();

        let option_encrypt_version = WzHeader::get_encrypted_version(
            slice_reader.buf,
            slice_reader.header.fstart,
            slice_reader.header.fsize,
        );

        let mut wz_file_meta = WzFileMeta {
            path: "".to_string(),
            patch_version: patch_version.unwrap_or(self.wz_file_meta.patch_version),
            wz_version_header: if let Some(encrypt_version) = option_encrypt_version {
                encrypt_version as i32
            } else {
                WZ_VERSION_HEADER_64BIT_START as i32
            },
            wz_with_encrypt_version_header: option_encrypt_version.is_some(),
            hash: 0,
        };

        let mut version_gen = util::version::pkg1::VersionGen::new(
            wz_file_meta.wz_version_header,
            wz_file_meta.patch_version,
            2000,
        );

        if wz_file_meta.patch_version == -1 {
            if wz_file_meta.wz_with_encrypt_version_header {
                version_gen.current = 1;
                version_gen.max_version = 2000;
            } else {
                /* not hold encver in wz_file, directly try 770 - 780 */
                version_gen.current = WZ_VERSION_HEADER_64BIT_START as i32;
                version_gen.max_version = WZ_VERSION_HEADER_64BIT_START as i32 + 10;
            };

            /* there has code in maplelib to detect version from maplestory.exe here */

            for (ver_to_decode, hash) in version_gen {
                wz_file_meta.hash = hash as usize;
                if let Ok(childs) =
                    self.try_decode_with_wz_version_number(parent, &wz_file_meta, ver_to_decode)
                {
                    wz_file_meta.patch_version = ver_to_decode;
                    self.update_wz_file_meta(wz_file_meta);
                    self.is_parsed = true;
                    return Ok(childs);
                }
            }

            return Err(Error::ErrorGameVerHash);
        }

        wz_file_meta.hash = version_gen.check_and_get_version_hash() as usize;

        let childs = self.try_decode_with_wz_version_number(
            parent,
            &wz_file_meta,
            wz_file_meta.patch_version,
        )?;
        self.update_wz_file_meta(wz_file_meta);
        self.is_parsed = true;

        Ok(childs)
    }

    fn try_decode_with_wz_version_number(
        &self,
        parent: &WzNodeArc,
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
            .find(|(_, node)| matches!(node.read().unwrap().object_type, WzObjectType::Image(_)));

        if let Some((name, image_node)) = first_image_node {
            let header_type = image_node
                .read()
                .unwrap()
                .try_as_image()
                .map(|node| node.get_header_type())
                .ok_or(Error::ErrorGameVerHash)?;

            if !wz_image::is_valid_image_header(header_type) {
                return Err(Error::UnknownImageHeader(
                    header_type as u8,
                    name.to_string(),
                ));
            }
        }

        // there a special case this 2 will match
        if !meta.wz_with_encrypt_version_header && use_maplestory_patch_version == 113 {
            return Err(Error::ErrorGameVerHash);
        }

        Ok(childs)
    }

    fn update_wz_file_meta(&mut self, wz_file_meta: WzFileMeta) {
        self.wz_file_meta = WzFileMeta {
            path: std::mem::take(&mut self.wz_file_meta.path),
            ..wz_file_meta
        };
    }
}
