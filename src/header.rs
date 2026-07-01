use super::reader::Error;
use crate::util::version::PKGVersion;
use scroll::{ctx, Endian, Error as ScrollError, Pread, LE};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WzHeaderFlag(u8);

impl WzHeaderFlag {
    pub const ENCVER_MISSING: Self = Self(1);
    pub const PKG2_RANDOM_HEADER: Self = Self(2);
    pub const PKG2_RANDOM_HEADER64: Self = Self(4);

    #[inline]
    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    #[inline]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Wz file's header.
#[derive(Debug, Clone, Copy, Default)]
pub struct WzHeader {
    pub ident: PKGVersion,
    pub fsize: u64,
    /// when wz file's content actually start
    pub fstart: usize,
    /// when wz directory actually start
    pub data_start: usize,

    // pkg1 encrypt version
    pub encrypt_version: Option<u16>,
    // pkg2 hash
    pub hash1: u64,
    pub hash2: u64,
    pub flags: WzHeaderFlag,
}

impl ctx::TryFromCtx<'_, Endian> for WzHeader {
    type Error = Error;
    fn try_from_ctx(src: &'_ [u8], _: Endian) -> std::result::Result<(Self, usize), Self::Error> {
        Self::read_from_buf(src)
    }
}

impl WzHeader {
    /// Low 32 bits — used by PKG2 V1–V5 offset calcs, version gens, KMST1199 key.
    #[inline]
    pub fn hash1_u32(&self) -> u32 {
        self.hash1 as u32
    }

    #[inline]
    pub fn hash2_u32(&self) -> u32 {
        self.hash2 as u32
    }

    #[inline]
    pub fn is_pkg2_64(&self) -> bool {
        self.flags.contains(WzHeaderFlag::PKG2_RANDOM_HEADER64)
    }

    #[inline]
    pub fn get_header_slice(buf: &[u8]) -> &[u8] {
        let fstart = Self::get_wz_fstart(buf).unwrap() as usize;
        &buf[0..fstart]
    }
    #[inline]
    pub fn get_ident(buf: &[u8]) -> Result<PKGVersion> {
        buf.get(0..4)
            .ok_or(ScrollError::BadInput {
                size: buf.len(),
                msg: "invalid buffer to reading ident",
            })?
            .pread::<&str>(0)
            .map(PKGVersion::from)
            .or(Ok(PKGVersion::Unknown))
    }
    #[inline]
    pub fn get_wz_fsize(buf: &[u8]) -> Result<u64> {
        buf.pread_with::<u64>(4, LE).map_err(Error::from)
    }
    #[inline]
    pub fn get_wz_fstart(buf: &[u8]) -> Result<u32> {
        buf.pread_with::<u32>(12, LE).map_err(Error::from)
    }
    #[inline]
    pub fn get_encrypted_version(&self, buf: &[u8]) -> Option<u16> {
        let encrypted_version = buf.pread_with::<u16>(self.fstart, LE).ok()?;

        if self.fsize < 2 || encrypted_version > 0xff {
            return None;
        }

        if encrypted_version == 0x80 {
            let entry_count = buf.pread_with::<i32>(self.fstart + 2, LE).ok()?;
            //check entry count is valid
            if entry_count > 0 && (entry_count & 0xff) == 0 && entry_count <= 0xffff {
                return None;
            }
        }

        Some(encrypted_version)
    }
    pub fn read_from_buf(buf: &[u8]) -> Result<(WzHeader, usize)> {
        let mut header = WzHeader::default();

        header.ident = Self::get_ident(buf)?;

        if header.ident == PKGVersion::Unknown {
            if let Ok(result) = WzHeader::read_pkg2_random_header_from_buf(
                buf,
                U64_HEADER_LEN,
                read_kmst1202_random_header,
                WzHeaderFlag::PKG2_RANDOM_HEADER64,
            ) {
                return Ok(result);
            }
            return WzHeader::read_pkg2_random_header_from_buf(
                buf,
                U32_HEADER_LEN,
                read_kmst1201_random_header,
                WzHeaderFlag::PKG2_RANDOM_HEADER,
            );
        }

        header.fsize = Self::get_wz_fsize(buf)?;
        header.fstart = Self::get_wz_fstart(buf)? as usize;

        header.data_start = header.fstart;

        if header.ident == PKGVersion::V1 {
            header.encrypt_version = header.get_encrypted_version(buf);
            if header.encrypt_version.is_some() {
                header.data_start += 2;
            }
        }

        if header.ident == PKGVersion::V2 {
            header.hash1 = buf
                .pread_with::<u32>(header.fstart, LE)
                .map_err(Error::from)? as u64;
            header.hash2 = buf
                .pread_with::<u32>(header.fstart + 4, LE)
                .map_err(Error::from)? as u64;
            header.data_start += 8;
        }

        Ok((header, header.fstart))
    }

    fn read_pkg2_random_header_from_buf(
        buf: &[u8],
        assumed_header_size: usize,
        read_data_fn: ReadHeaderDataFn,
        flags: WzHeaderFlag,
    ) -> Result<(WzHeader, usize)> {
        if buf.len() < assumed_header_size {
            return Err(Error::UnableToReadWzHeader);
        }

        let expected_fsize = (buf.len() - assumed_header_size) as u64;

        let (hash1, hash2, fsize) = read_data_fn(buf);

        if fsize != expected_fsize {
            return Err(Error::UnableToReadWzHeader);
        }

        Ok((
            WzHeader {
                ident: PKGVersion::V2,
                fsize,
                fstart: assumed_header_size,
                data_start: assumed_header_size,
                encrypt_version: None,
                hash1,
                hash2,
                flags,
            },
            assumed_header_size,
        ))
    }
}

const U32_HEADER_LEN: usize = 60 + 8;
const U64_HEADER_LEN: usize = 150;

type ReadHeaderDataFn = fn(&[u8]) -> (u64, u64, u64);

#[inline]
fn read_kmst1201_random_header(buf: &[u8]) -> (u64, u64, u64) {
    let hash1 = u32::from_le_bytes([buf[0x43], buf[0x1A], buf[0x30], buf[0x10]]) as u64;
    let hash2 = u32::from_le_bytes([buf[0x2D], buf[0x07], buf[0x3F], buf[0x2E]]) as u64;
    let fsize = u64::from_le_bytes([buf[0x15], buf[0x19], buf[0x39], buf[0x41], 0, 0, 0, 0]);
    (hash1, hash2, fsize)
}

#[inline]
fn read_kmst1202_random_header(buf: &[u8]) -> (u64, u64, u64) {
    let hash1 = u64::from_le_bytes([
        buf[0x48], buf[0x24], buf[0x0F], buf[0x31], buf[0x46], buf[0x47], buf[0x63], buf[0x67],
    ]);
    let hash2 = u64::from_le_bytes([
        buf[0x8E], buf[0x8C], buf[0x93], buf[0x0E], buf[0x64], buf[0x7B], buf[0x2E], buf[0x4D],
    ]);
    let fsize = u32::from_le_bytes([buf[0x12], buf[0x09], buf[0x02], buf[0x95]]) as u64;
    (hash1, hash2, fsize)
}
