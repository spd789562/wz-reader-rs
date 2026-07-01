use crate::util::string_decryptor::pkg2_decryptor::{get_kmst1199_key, get_kmst1202_key};
use crate::{
    directory, reader, util, util::profile, util::string_decryptor, util::version::PKGVersion,
    wz_image, SharedWzStringDecryptor, WzDirectory, WzNodeArc, WzNodeArcVec, WzNodeCast,
    WzObjectType, WzReader,
};
use memmap2::Mmap;
use std::fs::File;
use std::sync::{Arc, RwLock};

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
    #[error("Unknown pkg version, can't resolve children")]
    UnknownPkgVersion,
    #[error("Failed to verify string decryptor")]
    FailedToVerifyStringDecryptor,
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
    /// whether the string decryptor is verified
    pub string_decryptor_verified: bool,
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
        existing_key: Option<&SharedWzStringDecryptor>,
    ) -> Result<WzFile, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file: File = File::open(&path)?;
        let map = unsafe { Mmap::map(&file)? };

        let block_size = map.len();

        // use existing key or create from wz_iv
        let existing_keys = existing_key.cloned().or_else(|| {
            wz_iv.map(|iv| {
                let keys: SharedWzStringDecryptor = Arc::new(RwLock::new(
                    string_decryptor::ecb_decryptor::EcbDecryptor::from_iv(iv),
                ));
                keys
            })
        });

        // ensure the keys is valid or guess one
        let verified_keys = existing_keys
            .and_then(|keys| {
                if string_decryptor::verify_decryptor_from_wz_file(&map, &keys).is_ok() {
                    Some(keys)
                } else {
                    None
                }
            })
            .or_else(|| string_decryptor::guess_decryptor_from_wz_file(&map));
        let string_decryptor_verified = verified_keys.is_some();

        let reader = if let Some(keys) = verified_keys {
            WzReader::new(map).with_existing_keys(keys.clone())
        } else {
            WzReader::new(map)
        };

        let offset = reader.header.data_start;

        let wz_file_meta = WzFileMeta {
            path: path.as_ref().to_str().unwrap().to_string(),
            patch_version: patch_version.unwrap_or(-1),
            wz_version_header: 0,
            wz_with_encrypt_version_header: true,
            hash: 0,
            string_decryptor_verified,
        };

        Ok(WzFile {
            offset,
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
        match self.reader.create_header().ident {
            PKGVersion::V1 => self.parse_pkg1(parent, patch_version),
            PKGVersion::V2 => self.parse_pkg2(parent),
            _ => Err(Error::UnknownPkgVersion),
        }
    }

    fn parse_pkg1(
        &mut self,
        parent: &WzNodeArc,
        patch_version: Option<i32>,
    ) -> Result<WzNodeArcVec, Error> {
        let option_encrypt_version = self.reader.header.encrypt_version;

        let mut wz_file_meta = WzFileMeta {
            path: "".to_string(),
            patch_version: patch_version.unwrap_or(self.wz_file_meta.patch_version),
            wz_version_header: if let Some(encrypt_version) = option_encrypt_version {
                encrypt_version as i32
            } else {
                WZ_VERSION_HEADER_64BIT_START as i32
            },
            string_decryptor_verified: self.wz_file_meta.string_decryptor_verified,
            wz_with_encrypt_version_header: option_encrypt_version.is_some(),
            hash: 0,
        };

        let mut wz_dir = WzDirectory::new(self.offset, self.block_size, &self.reader, false);

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
                wz_file_meta.wz_version_header = ver_to_decode;
                wz_dir.hash = hash as u64;
                if let Ok(children) =
                    self.try_decode_with_wz_version_number(parent, &wz_file_meta, &mut wz_dir)
                {
                    wz_file_meta.patch_version = ver_to_decode;
                    self.update_wz_file_meta(wz_file_meta);
                    self.is_parsed = true;
                    return Ok(children);
                }
            }

            return Err(Error::ErrorGameVerHash);
        }

        wz_file_meta.hash = version_gen.check_and_get_version_hash() as usize;
        wz_dir.hash = wz_file_meta.hash as u64;

        let children =
            self.try_decode_with_wz_version_number(parent, &wz_file_meta, &mut wz_dir)?;
        self.update_wz_file_meta(wz_file_meta);
        self.is_parsed = true;

        Ok(children)
    }

    fn parse_pkg2(&mut self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        let hash1 = self.reader.header.hash1;
        let hash2 = self.reader.header.hash2;

        let mut wz_file_meta = WzFileMeta {
            path: "".to_string(),
            patch_version: -1,
            wz_version_header: 0,
            wz_with_encrypt_version_header: false,
            string_decryptor_verified: self.wz_file_meta.string_decryptor_verified,
            hash: 0,
        };

        let mut wz_dir = WzDirectory::new(self.offset, self.block_size, &self.reader, false);

        if !self.reader.header.is_pkg2_64() {
            wz_dir.prepare_entries()?;
        }

        // clone the cache so it don't take the lock too long
        let cached_profiles = profile::PKG2_PROFILE_CACHE.read().unwrap().clone();

        for cached_profile in cached_profiles.iter() {
            if cached_profile.profile.should_be_pkg2_64() != self.reader.header.is_pkg2_64() {
                continue;
            }
            if !cached_profile.verify_hash(hash1, hash2) {
                continue;
            }
            let hash = cached_profile.hash;

            if cached_profile.profile.should_be_pkg2_64() {
                wz_dir.reset_entry_parse();
            }
            wz_file_meta.hash = hash as usize;
            wz_dir.hash = hash;
            wz_dir.profile = cached_profile.profile.clone();

            if let Ok(children) = self.try_pkg2_profile(
                parent,
                &mut wz_file_meta,
                &mut wz_dir,
                hash1,
                hash,
                &cached_profile.profile,
            ) {
                self.update_wz_file_meta(wz_file_meta);
                self.is_parsed = true;
                return Ok(children);
            }
        }

        for profile in profile::get_all_pkg2_profiles() {
            if profile.should_be_pkg2_64() != self.reader.header.is_pkg2_64() {
                continue;
            }

            wz_dir.profile = profile.clone();

            for hash in profile.get_hash_iter(hash1, hash2) {
                if profile.should_be_pkg2_64() {
                    wz_dir.reset_entry_parse();
                }
                wz_file_meta.hash = hash as usize;
                wz_dir.hash = hash;

                if let Ok(children) = self.try_pkg2_profile(
                    parent,
                    &mut wz_file_meta,
                    &mut wz_dir,
                    hash1,
                    hash,
                    &profile,
                ) {
                    self.update_wz_file_meta(wz_file_meta);
                    self.is_parsed = true;

                    profile::PKG2_PROFILE_CACHE
                        .write()
                        .unwrap()
                        .push(profile::Pkg2Profile::new(profile.clone(), hash));

                    return Ok(children);
                }
            }
        }

        Err(Error::ErrorGameVerHash)
    }

    fn try_pkg2_profile(
        &self,
        parent: &WzNodeArc,
        wz_file_meta: &mut WzFileMeta,
        wz_dir: &mut WzDirectory,
        hash1: u64,
        hash: u64,
        profile: &profile::WzProfile,
    ) -> Result<WzNodeArcVec, Error> {
        if !wz_file_meta.string_decryptor_verified {
            self.update_keys(profile, hash1, hash);
            wz_dir.prepare_entries()?;
            if !wz_dir.verify_string_decryptor() {
                return Err(Error::FailedToVerifyStringDecryptor);
            }
        }
        self.try_decode_with_wz_version_number(parent, wz_file_meta, wz_dir)
    }

    fn try_decode_with_wz_version_number(
        &self,
        parent: &WzNodeArc,
        meta: &WzFileMeta,
        wz_dir: &mut WzDirectory,
    ) -> Result<WzNodeArcVec, Error> {
        if wz_dir.hash == 0 || wz_dir.calculate_offset_and_verify().is_err() {
            return Err(Error::ErrorGameVerHash);
        }

        let children = wz_dir.resolve_children(parent)?;

        let first_image_node = children
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
        if !meta.wz_with_encrypt_version_header && meta.wz_version_header == 113 {
            return Err(Error::ErrorGameVerHash);
        }

        Ok(children)
    }

    fn update_wz_file_meta(&mut self, wz_file_meta: WzFileMeta) {
        self.wz_file_meta = WzFileMeta {
            path: std::mem::take(&mut self.wz_file_meta.path),
            ..wz_file_meta
        };
    }

    fn update_keys(&self, profile: &profile::WzProfile, hash1: u64, target_hash: u64) {
        let iv = if profile.should_be_pkg2_64() {
            get_kmst1202_key(hash1, target_hash)
        } else {
            get_kmst1199_key(hash1 as u32, target_hash as u32) as u64
        };
        self.reader
            .pkg2_keys
            .write()
            .unwrap()
            .set_iv(iv, profile.decryptor_type);
    }
}
