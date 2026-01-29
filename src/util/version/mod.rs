pub mod pkg1;
pub mod pkg2;

use crate::util::maple_crypto_constants::{WZ_GMSIV, WZ_MSEAIV};
use crate::util::wz_mutable_key::WzMutableKey;
use crate::{directory::WzDirectoryType, reader, Reader, WzSliceReader};
use std::sync::{Arc, RwLock};

pub fn get_iv_by_maple_version(version: WzMapleVersion) -> [u8; 4] {
    match version {
        WzMapleVersion::GMS => WZ_GMSIV,
        WzMapleVersion::EMS => WZ_MSEAIV,
        _ => [0; 4],
    }
}

/// MapleStory version, use to determine the IV for decryption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WzMapleVersion {
    /// Global MapleStory (old)
    GMS,

    /// 新楓之谷 / 冒险岛Online / 메이플스토리 / MapleSEA / EMS (old)
    EMS,

    /// BMS / GMS / MapleSEA / メイプルストーリー / 메이플스토리
    BMS,

    CLASSIC,

    GENERATE,

    /* from zlz.dll */
    GETFROMZLZ,

    CUSTOM,

    UNKNOWN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PKGVersion {
    V1,
    V2,
    #[default]
    Unknown,
}

impl From<&str> for PKGVersion {
    fn from(value: &str) -> Self {
        match value.as_ref() {
            "PKG1" => PKGVersion::V1,
            "PKG2" => PKGVersion::V2,
            _ => PKGVersion::Unknown,
        }
    }
}

/// Verify IV from wz image
pub fn verify_iv_from_wz_img(buf: &[u8], iv: &[u8; 4]) -> bool {
    let reader = WzSliceReader::new(buf, &Arc::new(RwLock::new(WzMutableKey::from_iv(*iv))));

    reader.pos.set(1);

    reader.read_wz_string().unwrap_or_default() == "Property"
}

/// Try to guess IV from wz image use fixed value. Currently will try GMS, EMS, BMS.
pub fn guess_iv_from_wz_img(buf: &[u8]) -> Option<[u8; 4]> {
    // not support other then WzImageHeaderType::WithoutOffset
    if buf[0] != 0x73 {
        return None;
    }

    let guess_versions = [
        WzMapleVersion::GMS,
        WzMapleVersion::EMS,
        WzMapleVersion::BMS,
    ];

    for version in guess_versions.iter() {
        let iv = get_iv_by_maple_version(*version);
        if verify_iv_from_wz_img(buf, &iv) {
            return Some(iv);
        }
    }

    None
}

// basically wzcr2 implementation see @link https://github.com/Kagamia/WzComparerR2/blob/f66dd68cda767db8a1ff7af5c58fa89324cc2bd0/WzComparerR2.WzLib/Wz_Crypto.cs#L109
// it try to read first wz_directory entry's name and verify it is a valid string
pub fn verify_iv_from_wz_file(buf: &[u8], iv: &[u8; 4]) -> Result<(), reader::Error> {
    let reader =
        WzSliceReader::new_with_header(buf, &Arc::new(RwLock::new(WzMutableKey::from_iv(*iv))));

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

    let _wz_name: String;

    match dir_type {
        // the first entry should always not be offset thou
        WzDirectoryType::MetaAtOffset => {
            let str_offset = reader.read_i32()?;

            let offset = reader.header.data_start + str_offset as usize;
            // just check string can be valid string(instead of parse string lossy), so can prove the iv is valid
            let meta = reader.read_wz_string_meta_at(offset + 1)?;
            _wz_name = reader.try_resolve_wz_string_meta(
                &meta.string_type,
                meta.offset,
                meta.length as usize,
            )?;
        }
        WzDirectoryType::WzDirectory | WzDirectoryType::WzImage => {
            // just check string can be valid string(instead of parse string lossy), so can prove the iv is valid
            let meta = reader.read_wz_string_meta()?;
            _wz_name = reader.try_resolve_wz_string_meta(
                &meta.string_type,
                meta.offset,
                meta.length as usize,
            )?;
        }
        _ => return Err(reader::Error::DecryptError(reader.pos.get())),
    }

    // maybe also check is all valid ascii
    // if !_wz_name.is_ascii() {
    //     return Err(reader::Error::DecryptError(reader.pos.get()));
    // }

    Ok(())
}

pub fn guess_iv_from_wz_file(buf: &[u8]) -> Option<[u8; 4]> {
    let guess_versions = [
        WzMapleVersion::BMS,
        WzMapleVersion::GMS,
        WzMapleVersion::EMS,
    ];

    for version in guess_versions.iter() {
        let iv = get_iv_by_maple_version(*version);
        if verify_iv_from_wz_file(buf, &iv).is_ok() {
            return Some(iv);
        }
    }

    None
}
