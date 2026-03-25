use std::sync::{Arc, RwLock};

pub mod ecb_decryptor;
pub mod pkg2_decryptor;

use crate::property::{WzStringMeta, WzStringType};
use crate::util::version::PKGVersion;
use crate::version::{get_iv_by_maple_version, get_key_by_maple_version, WzMapleVersion};
use crate::SharedWzMutableKey;
use crate::{directory::WzDirectoryType, reader, Reader, WzSliceReader};

pub trait Decryptor: std::fmt::Debug + Send + Sync {
    fn get_iv_hash(&self) -> u64;
    fn is_enough(&self, size: usize) -> bool;
    fn at(&mut self, index: usize) -> &u8;
    fn try_at(&self, index: usize) -> Option<&u8>;
    fn decrypt_slice(&self, data: &mut [u8]);
    fn ensure_key_size(&mut self, size: usize) -> Result<(), String>;
    fn get_enc_type(&self) -> DecrypterType;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DecrypterType {
    BMS,
    KMS,
    GMS,
    KMST1198,
    #[default]
    Unknown,
}

pub(crate) fn try_get_first_wz_name_meta(buf: &[u8]) -> Result<WzStringMeta, reader::Error> {
    let keys: SharedWzMutableKey =
        Arc::new(RwLock::new(ecb_decryptor::EcbDecryptor::from_iv([0; 4])));
    let reader = WzSliceReader::new_with_header(buf, &keys);

    reader.seek(reader.header.data_start);

    let mut entry_count = reader.read_wz_int()?;

    if reader.header.ident == PKGVersion::V1 {
        // invalid wz file
        if !(0..=1000000).contains(&entry_count) {
            return Err(reader::Error::DecryptError(reader.pos.get()));
        }
    } else if reader.header.ident == PKGVersion::V2 {
        let dir_type = WzDirectoryType::from(reader.read_u8_at(reader.pos.get())?);
        if !matches!(
            dir_type,
            WzDirectoryType::WzDirectory | WzDirectoryType::WzImage
        ) {
            // even it's a encrypted_offset_count, it's still probably a invalid wz file since we not getting any wz_dir or wz_image
            return Err(reader::Error::DecryptError(reader.pos.get()));
        }
        entry_count = 1;
    }

    if entry_count == 0 {
        return Err(reader::Error::DecryptError(reader.pos.get()));
    }

    let dir_type = WzDirectoryType::from(reader.read_u8()?);

    let wz_name_meta = match dir_type {
        // the first entry should always not be offset thou
        WzDirectoryType::MetaAtOffset => {
            let str_offset = reader.read_i32()?;

            let offset = reader.header.data_start + str_offset as usize;
            reader.read_wz_string_meta_at(offset + 1)?
        }
        WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => reader.read_wz_string_meta()?,
        _ => return Err(reader::Error::DecryptError(reader.pos.get())),
    };

    Ok(wz_name_meta)
}

pub(crate) fn try_get_first_wz_name_pkg2_meta_from_wz_file(
    buf: &[u8],
) -> Result<WzStringMeta, reader::Error> {
    let keys: SharedWzMutableKey =
        Arc::new(RwLock::new(ecb_decryptor::EcbDecryptor::from_iv([0; 4])));
    let reader = WzSliceReader::new_with_header(buf, &keys);

    if reader.header.ident != PKGVersion::V2 {
        return Err(reader::Error::DecryptError(reader.pos.get()));
    }

    reader.seek(reader.header.data_start);

    // entry count
    reader.read_wz_int()?;

    // first dir type
    reader.read_u8()?;

    reader.read_wz_string_pkg2_dir_meta()
}

pub fn verify_decryptor_from_wz_file_with_meta(
    buf: &[u8],
    decryptor: &SharedWzMutableKey,
    meta: &WzStringMeta,
) -> Result<(), reader::Error> {
    let reader = WzSliceReader::new_with_header(buf, decryptor);

    // just check string can be valid string(instead of parse string lossy), so can prove the iv is valid
    let _wz_name =
        reader.try_resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?;

    // maybe also check is all valid ascii
    // if !_wz_name.is_ascii() {
    //     return Err(reader::Error::DecryptError(reader.pos.get()));
    // }

    Ok(())
}

pub fn verify_decryptor_from_wz_file(
    buf: &[u8],
    decryptor: &SharedWzMutableKey,
) -> Result<(), reader::Error> {
    let meta = try_get_first_wz_name_meta(buf)?;
    verify_decryptor_from_wz_file_with_meta(buf, decryptor, &meta)
}

pub fn guess_decryptor_from_wz_file(buf: &[u8]) -> Option<SharedWzMutableKey> {
    let guess_versions = [
        WzMapleVersion::BMS,
        WzMapleVersion::GMS,
        WzMapleVersion::EMS,
    ];

    let meta = try_get_first_wz_name_meta(buf)
        .ok()
        .or(Some(WzStringMeta::empty()))?;

    if meta.string_type != WzStringType::Empty {
        for version in guess_versions.iter() {
            let iv = get_iv_by_maple_version(*version);
            let keys: SharedWzMutableKey =
                Arc::new(RwLock::new(ecb_decryptor::EcbDecryptor::from_iv(iv)));
            if verify_decryptor_from_wz_file_with_meta(buf, &keys, &meta).is_ok() {
                return Some(keys);
            }
        }
    }

    let guess_versions = [WzMapleVersion::KMST1198];

    let pkg2_dir_meta = try_get_first_wz_name_pkg2_meta_from_wz_file(buf)
        .ok()
        .or(Some(WzStringMeta::empty()))?;

    if pkg2_dir_meta.string_type != WzStringType::Empty {
        for version in guess_versions.iter() {
            let key = get_key_by_maple_version(*version);
            let keys: SharedWzMutableKey = Arc::new(RwLock::new(
                pkg2_decryptor::Pkg2Decryptor::new_with_key(key),
            ));
            if verify_decryptor_from_wz_file_with_meta(buf, &keys, &pkg2_dir_meta).is_ok() {
                return Some(keys);
            }
        }
    }

    None
}
