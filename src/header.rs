use scroll::{ctx, Endian, Pread, LE};

#[derive(Debug, Clone, Copy, Default)]
pub struct WzHeader<'a> {
    pub ident: &'a str,
    pub fsize: u64,
    pub fstart: usize,
    pub copyright: &'a str,
}

impl<'a> ctx::TryFromCtx<'a, Endian> for WzHeader<'a> {
    type Error = scroll::Error;
    fn try_from_ctx(src: &'a [u8], _: Endian) -> Result<(Self, usize), Self::Error> {
        Self::read_from_buf(src)
    }
}

impl WzHeader<'_> {
    pub fn get_header_slice(buf: &[u8]) -> &[u8] {
        let fstart = Self::get_wz_fstart(buf).unwrap() as usize;
        &buf[0..fstart]
    }
    pub fn get_ident(buf: &[u8]) -> Result<&str, scroll::Error> {
        buf[0..4].pread::<&str>(0)
    }
    pub fn get_wz_fsize(buf: &[u8]) -> Result<u64, scroll::Error> {
        buf.pread_with::<u64>(4, LE)
    }
    pub fn get_wz_fstart(buf: &[u8]) -> Result<u32, scroll::Error> {
        buf.pread_with::<u32>(12, LE)
    }
    pub fn get_wz_copyright(buf: &[u8]) -> Result<&str, scroll::Error> {
        let fstart = Self::get_wz_fstart(buf)? as usize;
        buf[16..fstart - 17].pread::<&str>(0)
    }
    pub fn read_from_buf(buf: &[u8]) -> Result<(WzHeader, usize), scroll::Error> {
        let ident = Self::get_ident(buf)?;

        let fsize = Self::get_wz_fsize(buf)?;

        let fstart = Self::get_wz_fstart(buf)? as usize;

        let copyright = Self::get_wz_copyright(buf)?;

        let offset = fstart - 17;

        Ok((WzHeader {
            ident,
            fsize,
            fstart,
            copyright,
        }, offset))
    }
}