use super::reader::Error;
use crate::util::version::PKGVersion;
use scroll::{ctx, Endian, Error as ScrollError, Pread, LE};

type Result<T> = std::result::Result<T, Error>;

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
    pub hash1: u32,
    pub hash2: u32,
}

impl ctx::TryFromCtx<'_, Endian> for WzHeader {
    type Error = Error;
    fn try_from_ctx(src: &'_ [u8], _: Endian) -> std::result::Result<(Self, usize), Self::Error> {
        Self::read_from_buf(src)
    }
}

impl WzHeader {
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
            .map(|s| PKGVersion::from(s))
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
            return WzHeader::read_pkg2_random_header_from_buf(buf, read_kmst1201_random_header);
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
                .map_err(Error::from)?;
            header.hash2 = buf
                .pread_with::<u32>(header.fstart + 4, LE)
                .map_err(Error::from)?;
            header.data_start += 8;
        }

        Ok((header, header.fstart))
    }

    fn read_pkg2_random_header_from_buf(
        buf: &[u8],
        read_data_fn: ReadHeaderDataFn,
    ) -> Result<(WzHeader, usize)> {
        // pkg1 header size usually is 60, but pkg2 has two extra u32
        let assumed_header_size = 60 + 8;

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
            },
            assumed_header_size,
        ))
    }
}

type ReadHeaderDataFn = fn(&[u8]) -> (u32, u32, u64);

#[inline]
fn read_kmst1201_random_header(buf: &[u8]) -> (u32, u32, u64) {
    let hash1 = u32::from_le_bytes([buf[0x43], buf[0x1A], buf[0x30], buf[0x10]]);
    let hash2 = u32::from_le_bytes([buf[0x2D], buf[0x07], buf[0x3F], buf[0x2E]]);
    let fsize = u64::from_le_bytes([buf[0x15], buf[0x19], buf[0x39], buf[0x41], 0, 0, 0, 0]);
    (hash1, hash2, fsize)
}
