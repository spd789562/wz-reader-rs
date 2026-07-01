use crate::{directory, reader::Error, WzHeader};

type Result<T> = std::result::Result<T, Error>;
type EntryCountResult<T> = std::result::Result<T, directory::Error>;

static WZ_OFFSET: u32 = 0x581C3F6D;

#[derive(Debug, Clone, Copy, Default)]
pub enum WzOffsetVersion {
    #[default]
    Pkg1,
    Pkg2V1,
    Pkg2V2,
    Pkg2V3,
    Pkg2_64V1,
}

impl WzOffsetVersion {
    pub fn get_calculator(&self) -> OffsetCalculator {
        match self {
            WzOffsetVersion::Pkg1 => read_wz_offset,
            WzOffsetVersion::Pkg2V1 => read_wz_offset_pkg2,
            WzOffsetVersion::Pkg2V2 => read_wz_offset_pkg2_v2,
            WzOffsetVersion::Pkg2V3 => read_wz_offset_pkg2_v3,
            WzOffsetVersion::Pkg2_64V1 => read_wz_offset_pkg2_64_v1,
        }
    }
    pub fn get_entry_count_calculator(&self) -> EntryCountCalculator {
        match self {
            WzOffsetVersion::Pkg2_64V1 => decrypt_pkg2_entry_count_64_v1,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WzOffsetMeta {
    pub hash: u64,
    pub encrypted_offset: u32,
    pub offset: usize,
}

impl WzOffsetMeta {
    #[inline]
    pub fn hash_u32(&self) -> u32 {
        self.hash as u32
    }
}

pub type OffsetCalculator = fn(&WzHeader, &WzOffsetMeta) -> Result<usize>;
pub type EntryCountCalculator = fn(&WzHeader, u64, i64) -> EntryCountResult<usize>;

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg1
#[inline]
pub fn read_wz_offset(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let header_size = header.fstart;
    let offset = meta.offset;
    let hash = meta.hash_u32() as usize;

    let offset = offset.wrapping_sub(header_size) ^ 0xFFFFFFFF;
    let offset = offset.wrapping_mul(hash) & 0xFFFFFFFF;
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
    let hash = meta.hash_u32();
    let hash1 = header.hash1_u32();

    let distance = ((hash ^ hash1) & 0x1F) as u8;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset;
    let offset = offset.wrapping_mul(hash);
    let offset = offset.wrapping_sub(WZ_OFFSET);
    let offset = offset ^ hash1.wrapping_mul(0x01010101);
    let offset = offset.rotate_left(distance as u32);

    let offset = offset ^ meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1198
#[inline]
pub fn read_wz_offset_pkg2_v2(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;
    let hash = meta.hash_u32();
    let hash1 = header.hash1_u32();

    let distance = ((hash ^ hash1) & 0x1F) as u8 as u32;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset;
    let offset = offset.wrapping_mul(hash ^ hash1);
    let offset = offset.wrapping_sub(WZ_OFFSET);
    let offset = offset ^ hash1.wrapping_mul(0x01010101);
    let offset = offset.rotate_left(distance);

    let offset = offset ^ !meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1199-1200
#[inline]
pub fn read_wz_offset_pkg2_v3(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;
    let hash = meta.hash_u32();
    let hash1 = header.hash1_u32();
    let pre_hash = hash1 ^ hash;
    let mixed_hash =
        crate::util::string_decryptor::pkg2_decryptor::mix_kmst1199(pre_hash ^ 0x6D4C3B2A)
            ^ 0x91E10DA5;
    let distance = (pre_hash ^ mixed_hash) & 0x1F;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset;
    let offset = offset.wrapping_mul(pre_hash.wrapping_add(mixed_hash ^ 0xA7E3C093));
    let offset = offset.wrapping_sub(WZ_OFFSET);
    let offset = offset ^ hash1.wrapping_mul(0x01010101);
    let offset = offset ^ mixed_hash.wrapping_mul(0x9E3779B9);
    let offset = offset.rotate_left(distance);
    let offset = offset ^ !meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

/// calculate the offset of the specific data like wz image/directory in wz file,
/// only work in pkg2 with version 1202
#[inline]
pub fn read_wz_offset_pkg2_64_v1(header: &WzHeader, meta: &WzOffsetMeta) -> Result<usize> {
    let offset = meta.offset as u32;
    let header_size = header.fstart as u32;
    let pre_hash = header.hash1_u32() ^ meta.hash_u32();
    let mixed_hash = pre_hash ^ 0x33BBBB33;

    let offset = offset.wrapping_sub(header_size);
    let offset = !offset;
    let offset = offset.wrapping_mul(pre_hash.wrapping_add(mixed_hash ^ 0xA7E3C093));
    let offset = offset.wrapping_sub(WZ_OFFSET);
    let offset = offset ^ header.hash1_u32().wrapping_mul(0x01010101);
    let offset = offset ^ mixed_hash.wrapping_mul(0x9E3779B9);
    let offset = offset.rotate_left(19);
    let offset = offset ^ !meta.encrypted_offset;
    let offset = offset.wrapping_add(header_size);

    Ok(offset as usize)
}

#[inline]
pub fn decrypt_pkg2_entry_count_64_v1(
    header: &WzHeader,
    hash: u64,
    encrypted_entry_count: i64,
) -> EntryCountResult<usize> {
    let dir_count =
        (encrypted_entry_count ^ header.hash1 as i64 ^ hash as i64 ^ 0x550EC4DD02C468EC) >> 16;
    if dir_count > i32::MAX as i64 {
        return Err(directory::Error::InvalidEntryCount);
    }
    Ok(dir_count as usize)
}
