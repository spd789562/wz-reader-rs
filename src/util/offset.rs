use crate::{reader::Error, WzHeader};

type Result<T> = std::result::Result<T, Error>;

static WZ_OFFSET: u32 = 0x581C3F6D;

pub enum WzOffsetVersion {
    Pkg1,
    Pkg2V1,
    Pkg2V2,
    Pkg2V3,
}

impl WzOffsetVersion {
    pub fn get_calculator(&self) -> OffsetCalculator {
        match self {
            WzOffsetVersion::Pkg1 => read_wz_offset,
            WzOffsetVersion::Pkg2V1 => read_wz_offset_pkg2,
            WzOffsetVersion::Pkg2V2 => read_wz_offset_pkg2_v2,
            WzOffsetVersion::Pkg2V3 => read_wz_offset_pkg2_v3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WzOffsetMeta {
    pub hash: u32,
    pub encrypted_offset: u32,
    pub offset: usize,
    pub pkg2_hash1: u32,
}

pub type OffsetCalculator = fn(&WzHeader, &WzOffsetMeta) -> Result<usize>;

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg1
#[inline]
pub fn read_wz_offset(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let header_size = header.fstart;
    let offset = meta.offset;

    let offset = offset.wrapping_sub(header_size) ^ 0xFFFFFFFF;
    let offset = offset.wrapping_mul(meta.hash as usize) & 0xFFFFFFFF;
    let offset = offset.wrapping_sub(WZ_OFFSET as usize);
    // it's pretty important need to cast to i32 first usize.rotate_left will give wrong result
    let offset = (offset as i32).rotate_left((offset as u32) & 0x1F) as usize & 0xFFFFFFFF;

    let offset = (offset ^ (meta.encrypted_offset as usize)) & 0xFFFFFFFF;
    let offset = offset.wrapping_add(header_size * 2) & 0xFFFFFFFF;

    Ok(offset)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1196-1197
#[inline]
pub fn read_wz_offset_pkg2(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;

    let distance = ((meta.hash ^ meta.pkg2_hash1) & 0x1F) as u8;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset as u32;
    let offset = offset.wrapping_mul(meta.hash);
    let offset = offset.wrapping_sub(WZ_OFFSET);
    let offset = offset ^ meta.pkg2_hash1.wrapping_mul(0x01010101);
    let offset = offset.rotate_left(distance as u32);

    let offset = offset ^ meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1198-1199
#[inline]
pub fn read_wz_offset_pkg2_v2(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;

    let distance = ((meta.hash ^ meta.pkg2_hash1) & 0x1F) as u8 as u32;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset as u32;
    let offset = offset.wrapping_mul(meta.hash ^ meta.pkg2_hash1);
    let offset = offset.wrapping_sub(WZ_OFFSET as u32);
    let offset = offset ^ meta.pkg2_hash1.wrapping_mul(0x01010101);
    let offset = offset.rotate_left(distance as u32);

    let offset = offset ^ !meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1200
#[inline]
pub fn read_wz_offset_pkg2_v3(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;
    let pre_hash = meta.pkg2_hash1 ^ meta.hash;
    let mixed_hash =
        crate::util::string_decryptor::pkg2_decryptor::mix_kmst1199(pre_hash ^ 0x6D4C3B2A)
            ^ 0x91E10DA5;
    let distance = ((pre_hash ^ mixed_hash) & 0x1F) as u32;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset;
    let offset = offset.wrapping_mul(pre_hash.wrapping_add(mixed_hash ^ 0xA7E3C093));
    let offset = offset ^ meta.pkg2_hash1.wrapping_mul(0x01010101);
    let offset = offset ^ mixed_hash.wrapping_mul(0x9E3779B9);
    let offset = offset.rotate_left(distance);
    let offset = offset ^ !meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}
