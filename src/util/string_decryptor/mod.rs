use std::sync::{Arc, LazyLock, OnceLock, RwLock};

pub mod ecb_decryptor;
pub mod pkg2_decryptor;

use crate::property::{WzStringMeta, WzStringType};
use crate::util::maple_crypto_constants::{WZ_GMSIV, WZ_MSEAIV};
use crate::util::version::PKGVersion;
use crate::version::{get_iv_by_maple_version, get_key_by_maple_version, WzMapleVersion};
use crate::{directory::WzDirectoryType, reader, Reader, WzSliceReader};

pub type SharedWzStringDecryptor = Arc<RwLock<dyn Decryptor>>;
pub use ecb_decryptor::EcbDecryptor;
pub use pkg2_decryptor::Pkg2Decryptor;

pub trait Decryptor: std::fmt::Debug + Send + Sync {
    fn get_iv_hash(&self) -> u64;
    fn set_iv(&mut self, iv: u32, enc_type: DecrypterType);
    fn is_enough(&self, size: usize) -> bool;
    fn is_pkg2(&self) -> bool;
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
    KMST1199,
    Custom,
    #[default]
    Unknown,
}

impl DecrypterType {
    pub fn need_init_key(&self) -> bool {
        self == &DecrypterType::KMST1199
    }
}

pub(crate) fn try_get_first_wz_name_meta(buf: &[u8]) -> Result<WzStringMeta, reader::Error> {
    let keys: SharedWzStringDecryptor = Arc::new(RwLock::new(EcbDecryptor::from_iv([0; 4])));
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
    let keys: SharedWzStringDecryptor = Arc::new(RwLock::new(EcbDecryptor::from_iv([0; 4])));
    let reader = WzSliceReader::new_with_header(buf, &keys);

    if reader.header.ident != PKGVersion::V2 {
        return Err(reader::Error::DecryptError(reader.pos.get()));
    }

    reader.seek(reader.header.data_start);

    // entry count
    let encrypted_entry_count = reader.read_wz_int()?;

    // first dir type
    let dir = reader.read_u8()?;

    // ignore the zero entry count
    if dir == 128 {
        reader.pos.set(reader.pos.get() - 1);
        if reader.read_wz_int()? == encrypted_entry_count {
            return Ok(WzStringMeta::empty());
        }
        return Err(reader::Error::DecryptError(reader.pos.get()));
    }

    reader.read_wz_string_pkg2_dir_meta()
}

pub fn verify_decryptor_from_wz_file_with_meta(
    buf: &[u8],
    decryptor: &SharedWzStringDecryptor,
    meta: &WzStringMeta,
) -> Result<(), reader::Error> {
    let reader = WzSliceReader::new_with_header(buf, decryptor);

    // just check string can be valid string(instead of parse string lossy), so can prove the iv is valid
    let _wz_name =
        reader.try_resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?;

    // maybe also check is all valid ascii
    if !_wz_name.is_ascii() {
        return Err(reader::Error::DecryptError(reader.pos.get()));
    }

    Ok(())
}

pub fn verify_decryptor_from_wz_file(
    buf: &[u8],
    decryptor: &SharedWzStringDecryptor,
) -> Result<(), reader::Error> {
    let meta = try_get_first_wz_name_meta(buf)?;
    verify_decryptor_from_wz_file_with_meta(buf, decryptor, &meta)
}

pub fn guess_decryptor_from_wz_file(buf: &[u8]) -> Option<SharedWzStringDecryptor> {
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
            let keys: SharedWzStringDecryptor =
                GLOBAL_STRING_DECRYPTOR.get_decryptor_by_version(*version);
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
            let keys: SharedWzStringDecryptor =
                GLOBAL_STRING_DECRYPTOR.get_decryptor_by_version(*version);
            if verify_decryptor_from_wz_file_with_meta(buf, &keys, &pkg2_dir_meta).is_ok() {
                return Some(keys);
            }
        }
    }

    None
}

pub struct StringDecryptor {
    pub gms: Arc<RwLock<EcbDecryptor>>,
    pub kms: Arc<RwLock<EcbDecryptor>>,
    pub general: Arc<RwLock<EcbDecryptor>>,
    pub custom: OnceLock<Arc<RwLock<EcbDecryptor>>>,
    pub kmst1198: Arc<RwLock<Pkg2Decryptor>>,
}

impl StringDecryptor {
    pub fn get_decryptor(&self, decryptor_type: DecrypterType) -> SharedWzStringDecryptor {
        match decryptor_type {
            DecrypterType::GMS => Arc::clone(&self.gms) as SharedWzStringDecryptor,
            DecrypterType::KMS => Arc::clone(&self.kms) as SharedWzStringDecryptor,
            DecrypterType::Custom => self.custom.get().unwrap().clone() as SharedWzStringDecryptor,
            DecrypterType::KMST1198 => Arc::clone(&self.kmst1198) as SharedWzStringDecryptor,
            /* KMST1199 key is calculate dynamically so we always return a new one */
            DecrypterType::KMST1199 => {
                Arc::new(RwLock::new(Pkg2Decryptor::default())) as SharedWzStringDecryptor
            }
            _ => Arc::clone(&self.general) as SharedWzStringDecryptor,
        }
    }
    pub fn get_decryptor_by_version(&self, version: WzMapleVersion) -> SharedWzStringDecryptor {
        match version {
            WzMapleVersion::GMS => Arc::clone(&self.gms) as SharedWzStringDecryptor,
            WzMapleVersion::EMS => Arc::clone(&self.kms) as SharedWzStringDecryptor,
            WzMapleVersion::KMST1198 => Arc::clone(&self.kmst1198) as SharedWzStringDecryptor,
            WzMapleVersion::KMST1199 => {
                Arc::new(RwLock::new(Pkg2Decryptor::default())) as SharedWzStringDecryptor
            }
            _ => Arc::clone(&self.general) as SharedWzStringDecryptor,
        }
    }
    pub fn get_decryptor_by_iv(&self, iv: [u8; 4]) -> SharedWzStringDecryptor {
        match iv {
            WZ_GMSIV => Arc::clone(&self.gms) as SharedWzStringDecryptor,
            WZ_MSEAIV => Arc::clone(&self.kms) as SharedWzStringDecryptor,
            [0, 0, 0, 0] => Arc::clone(&self.general) as SharedWzStringDecryptor,
            _ => Arc::new(RwLock::new(EcbDecryptor::from_iv(iv))) as SharedWzStringDecryptor,
        }
    }
}

pub static GLOBAL_STRING_DECRYPTOR: LazyLock<StringDecryptor> = LazyLock::new(|| StringDecryptor {
    gms: Arc::new(RwLock::new(EcbDecryptor::from_iv(get_iv_by_maple_version(
        WzMapleVersion::GMS,
    )))),
    kms: Arc::new(RwLock::new(EcbDecryptor::from_iv(get_iv_by_maple_version(
        WzMapleVersion::EMS,
    )))),
    custom: OnceLock::new(),
    general: Arc::new(RwLock::new(EcbDecryptor::from_iv([0; 4]))),
    kmst1198: Arc::new(RwLock::new(Pkg2Decryptor::new_with_key(
        get_key_by_maple_version(WzMapleVersion::KMST1198),
        DecrypterType::KMST1198,
    ))),
});
