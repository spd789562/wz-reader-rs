use crate::util::maple_crypto_constants::{WZ_GMSIV, WZ_MSEAIV};
use crate::util::wz_mutable_key::WzMutableKey;
use crate::WzSliceReader;
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

/// Verify IV from wz image
pub fn verify_iv_from_wz_img(buf: &[u8], iv: &[u8; 4]) -> bool {
    let reader = WzSliceReader::new(buf, &Arc::new(RwLock::new(WzMutableKey::from_iv(iv.clone()))));
    reader.pos.set(1);
    
    let name = reader.read_wz_string().unwrap_or_default();

    if name == "Property" {
        true
    } else {
        false
    }
}

/// Try to guess IV from wz image use fixed value. Currently will try GMS, EMS, BMS.
pub fn guess_iv_from_wz_img(buf: &[u8]) -> Option<[u8; 4]> {
    // not support other then WZ_IMAGE_HEADER_BYTE_WITHOUT_OFFSET
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