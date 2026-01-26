use super::reader::Error;
use scroll::{ctx, Endian, Pread, LE};

type Result<T> = std::result::Result<T, Error>;

/// Wz file's header.
#[derive(Debug, Clone, Copy, Default)]
pub struct WzHeader<'a> {
    pub ident: PKGVersion,
    pub fsize: u64,
    /// when wz file's content actually start
    pub fstart: usize,
    pub copyright: &'a str,
}

impl<'a> ctx::TryFromCtx<'a, Endian> for WzHeader<'a> {
    type Error = Error;
    fn try_from_ctx(src: &'a [u8], _: Endian) -> std::result::Result<(Self, usize), Self::Error> {
        Self::read_from_buf(src)
    }
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

impl WzHeader<'_> {
    #[inline]
    pub fn get_header_slice(buf: &[u8]) -> &[u8] {
        let fstart = Self::get_wz_fstart(buf).unwrap() as usize;
        &buf[0..fstart]
    }
    #[inline]
    pub fn get_ident(buf: &[u8]) -> Result<PKGVersion> {
        buf[0..4]
            .pread::<&str>(0)
            .map(|s| PKGVersion::from(s))
            .map_err(Error::from)
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
    pub fn get_wz_copyright(buf: &[u8]) -> Result<&str> {
        let fstart = Self::get_wz_fstart(buf)? as usize;
        buf[16..fstart].pread::<&str>(0).map_err(Error::from)
    }
    #[inline]
    pub fn read_encrypted_version(buf: &[u8]) -> Option<u16> {
        let fstart = Self::get_wz_fstart(buf).ok()? as usize;
        let fsize = Self::get_wz_fsize(buf).ok()?;

        Self::get_encrypted_version(buf, fstart, fsize)
    }
    #[inline]
    pub fn get_encrypted_version(buf: &[u8], fstart: usize, fsize: u64) -> Option<u16> {
        let encrypted_version = buf.pread_with::<u16>(fstart, LE).ok()?;

        if fsize < 2 || encrypted_version > 0xff {
            return None;
        }

        if encrypted_version == 0x80 {
            let entry_count = buf.pread_with::<i32>(fstart + 2, LE).ok()?;
            //check entry count is valid
            if entry_count > 0 && (entry_count & 0xff) == 0 && entry_count <= 0xffff {
                return None;
            }
        }

        Some(encrypted_version)
    }
    pub fn read_from_buf(buf: &[u8]) -> Result<(WzHeader, usize)> {
        let ident = Self::get_ident(buf)?;

        let fsize = Self::get_wz_fsize(buf)?;

        let fstart = Self::get_wz_fstart(buf)? as usize;

        let copyright = Self::get_wz_copyright(buf)?;

        let offset = fstart;

        Ok((
            WzHeader {
                ident,
                fsize,
                fstart,
                copyright,
            },
            offset,
        ))
    }
}
