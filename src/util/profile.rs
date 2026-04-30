use crate::{
    util::{offset::WzOffsetVersion, string_decryptor::DecrypterType},
    version::pkg2::Pkg2VersionGen,
};
use std::sync::{LazyLock, RwLock};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum WzProfileVersion {
    #[default]
    Pkg1,
    Pkg2V1200,
    Pkg2V1199,
    Pkg2V1198,
    Pkg2V1197,
    Pkg2V1196,
}

impl std::fmt::Display for WzProfileVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct WzProfile {
    pub name: WzProfileVersion,
    pub decryptor_type: DecrypterType,
    pub version_gen: Pkg2VersionGen,
    pub offset_version: WzOffsetVersion,
}

impl Default for WzProfile {
    fn default() -> Self {
        Self {
            name: WzProfileVersion::Pkg1,
            decryptor_type: DecrypterType::Unknown,
            version_gen: Pkg2VersionGen::Unknown,
            offset_version: WzOffsetVersion::Pkg1,
        }
    }
}

pub fn get_all_pkg2_profiles() -> Vec<WzProfile> {
    vec![
        WzProfile {
            name: WzProfileVersion::Pkg2V1200,
            decryptor_type: DecrypterType::KMST1199,
            version_gen: Pkg2VersionGen::V5,
            offset_version: WzOffsetVersion::Pkg2V3,
        },
        WzProfile {
            name: WzProfileVersion::Pkg2V1199,
            decryptor_type: DecrypterType::KMST1199,
            version_gen: Pkg2VersionGen::V4,
            offset_version: WzOffsetVersion::Pkg2V3,
        },
        WzProfile {
            name: WzProfileVersion::Pkg2V1198,
            decryptor_type: DecrypterType::KMST1198,
            version_gen: Pkg2VersionGen::V3,
            offset_version: WzOffsetVersion::Pkg2V2,
        },
        WzProfile {
            name: WzProfileVersion::Pkg2V1197,
            decryptor_type: DecrypterType::Unknown,
            version_gen: Pkg2VersionGen::V2,
            offset_version: WzOffsetVersion::Pkg2V2,
        },
        WzProfile {
            name: WzProfileVersion::Pkg2V1196,
            decryptor_type: DecrypterType::Unknown,
            version_gen: Pkg2VersionGen::V1,
            offset_version: WzOffsetVersion::Pkg2V1,
        },
    ]
}

pub static PKG2_PROFILE_CACHE: LazyLock<RwLock<Vec<Pkg2Profile>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

#[derive(Debug, Clone)]
pub struct Pkg2Profile {
    pub profile: WzProfile,
    pub hash: u32,
}

impl Pkg2Profile {
    pub fn new(profile: WzProfile, hash: u32) -> Self {
        Self { profile, hash }
    }
    pub fn verify_hash(&self, hash1: u32, hash2: u32) -> bool {
        self.profile
            .version_gen
            .get_generator(hash1, hash2)
            .get_verifier()(hash1, hash2, self.hash)
    }
}
