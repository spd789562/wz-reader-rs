use crate::util::maple_crypto_constants::{WZ_GMSIV, WZ_MSEAIV};

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