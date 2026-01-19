use memmap2::Mmap;
use scroll::{Pread, LE};
use std::cell::Cell;
use std::sync::{Arc, RwLock};

use crate::property::{encrypt_str, WzStringMeta, WzStringType};
use crate::util::WzMutableKey;
use crate::WzHeader;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Decryption error with len {0}")]
    DecryptError(usize),
    #[error("Error reading binary: {0}")]
    ReadError(#[from] scroll::Error),
    #[error("Error reading utf8 string: {0}")]
    ReadUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Error reading utf16 string: {0}")]
    ReadUtf16Error(#[from] std::string::FromUtf16Error),
}

type Result<T> = std::result::Result<T, Error>;

pub type SharedWzMutableKey = Arc<RwLock<WzMutableKey>>;

/// A basic reader for reading data, it store original data, and can't not
/// read data without provide offset of the data.
#[derive(Debug)]
pub struct WzBaseReader<T: Sized + AsRef<[u8]>> {
    pub map: T,
    pub wz_iv: [u8; 4],
    pub keys: Arc<RwLock<WzMutableKey>>,
}

/// the Mmap impl for WzBaseReader
pub type WzReader = WzBaseReader<Mmap>;

impl Default for WzBaseReader<Mmap> {
    fn default() -> Self {
        let memmap = memmap2::MmapMut::map_anon(1)
            .unwrap()
            .make_read_only()
            .unwrap();
        WzBaseReader {
            map: memmap,
            wz_iv: [0; 4],
            keys: Arc::new(RwLock::new(WzMutableKey::new([0; 4], [0; 32]))),
        }
    }
}

/// A reader that only hold part of the original data, and it hold a position of current reading.
#[derive(Debug, Clone)]
pub struct WzSliceReader<'a> {
    pub buf: &'a [u8],
    /// current reading position
    pub pos: Cell<usize>,
    _save_pos: Cell<usize>,
    pub header: WzHeader<'a>,
    pub keys: Arc<RwLock<WzMutableKey>>,
}

static WZ_OFFSET: i32 = 0x581C3F6D;

pub trait Reader {
    fn get_size(&self) -> usize;
    fn get_decrypt_slice(&self, range: std::ops::Range<usize>) -> Result<Vec<u8>>;
    fn read_u8_at(&self, pos: usize) -> Result<u8>;
    fn read_u16_at(&self, pos: usize) -> Result<u16>;
    fn read_u32_at(&self, pos: usize) -> Result<u32>;
    fn read_u64_at(&self, pos: usize) -> Result<u64>;
    fn read_i8_at(&self, pos: usize) -> Result<i8>;
    fn read_i16_at(&self, pos: usize) -> Result<i16>;
    fn read_i32_at(&self, pos: usize) -> Result<i32>;
    fn read_i64_at(&self, pos: usize) -> Result<i64>;
    fn read_float_at(&self, pos: usize) -> Result<f32>;
    fn read_double_at(&self, pos: usize) -> Result<f64>;

    #[inline]
    fn get_wz_string_type(&self, t: i8) -> WzStringType {
        match t {
            0 => WzStringType::Empty,
            t if t > 0 => WzStringType::Unicode,
            _ => WzStringType::Ascii,
        }
    }
    fn resolve_unicode_raw(&self, offset: usize, length: usize) -> Result<Vec<u16>> {
        let decrypted = self.get_decrypt_slice(offset..(offset + length))?;
        let mut strvec = Vec::with_capacity(length / 2);

        for (i, chunk) in decrypted.chunks(2).enumerate() {
            let c = u16::from_le_bytes([chunk[0], chunk[1]]);
            strvec.push(resolve_unicode_char(c, i as i32));
        }

        Ok(strvec)
    }
    fn resolve_ascii_raw(&self, offset: usize, length: usize) -> Result<Vec<u8>> {
        let mut decrypted = self.get_decrypt_slice(offset..(offset + length))?;

        decrypted.iter_mut().enumerate().for_each(|(i, byte)| {
            *byte = resolve_ascii_char(*byte, i as i32);
        });

        Ok(decrypted)
    }
    #[inline]
    fn resolve_wz_string_meta(
        &self,
        meta_type: &WzStringType,
        offset: usize,
        length: usize,
    ) -> Result<String> {
        match meta_type {
            WzStringType::Empty => Ok(String::new()),
            WzStringType::Unicode => {
                let strvec = self.resolve_unicode_raw(offset, length)?;

                Ok(String::from_utf16_lossy(&strvec))
            }
            WzStringType::Ascii => {
                let strvec = self.resolve_ascii_raw(offset, length)?;

                Ok(String::from_utf8_lossy(&strvec).to_string())
            }
        }
    }
    #[inline]
    fn try_resolve_wz_string_meta(
        &self,
        meta_type: &WzStringType,
        offset: usize,
        length: usize,
    ) -> Result<String> {
        match meta_type {
            WzStringType::Empty => Ok(String::new()),
            WzStringType::Unicode => {
                let strvec = self.resolve_unicode_raw(offset, length)?;

                String::from_utf16(&strvec).map_err(Error::from)
            }
            WzStringType::Ascii => {
                let strvec = self.resolve_ascii_raw(offset, length)?;

                String::from_utf8(strvec).map_err(Error::from)
            }
        }
    }
}

impl<T: AsRef<[u8]>> WzBaseReader<T> {
    pub fn new(map: T) -> Self {
        WzBaseReader {
            map,
            keys: Arc::new(RwLock::new(WzMutableKey::new([0; 4], [0; 32]))),
            wz_iv: [0; 4],
        }
    }
    pub fn with_iv(self, iv: [u8; 4]) -> Self {
        WzBaseReader {
            wz_iv: iv,
            keys: Arc::new(RwLock::new(WzMutableKey::from_iv(iv))),
            ..self
        }
    }

    // using the existing keys if the iv is the same to save the memory
    pub fn with_existing_keys(self, keys: Arc<RwLock<WzMutableKey>>) -> Self {
        let keys_iv = keys.read().unwrap().iv;
        if keys_iv != self.wz_iv {
            self
        } else {
            WzBaseReader { keys, ..self }
        }
    }

    #[inline]
    pub fn try_header(&self) -> Result<WzHeader> {
        self.map.as_ref().pread::<WzHeader>(0)
    }
    #[inline]
    pub fn create_header(&self) -> WzHeader {
        self.map
            .as_ref()
            .pread::<WzHeader>(0)
            .unwrap_or(WzHeader::default())
    }
    #[inline]
    pub fn get_ref_slice(&self) -> &[u8] {
        self.map.as_ref()
    }
    #[inline]
    pub fn get_slice(&self, range: std::ops::Range<usize>) -> &[u8] {
        &self.map.as_ref()[range]
    }
    #[inline]
    pub fn get_wz_fstart(&self) -> Result<u32> {
        WzHeader::get_wz_fstart(self.map.as_ref())
    }
    #[inline]
    pub fn get_wz_fsize(&self) -> Result<u64> {
        WzHeader::get_wz_fsize(self.map.as_ref())
    }
    #[inline]
    pub fn create_slice_reader_without_hash(&self) -> WzSliceReader {
        WzSliceReader::new(self.map.as_ref(), &self.keys).with_header(WzHeader::default())
    }
    #[inline]
    pub fn create_slice_reader(&self) -> WzSliceReader {
        WzSliceReader::new(self.map.as_ref(), &self.keys).with_header(self.create_header())
    }
    /// create a encrypt string from current `WzReader`
    #[inline]
    pub fn encrypt_str(&self, str: &str, meta_type: &WzStringType) -> Vec<u8> {
        if meta_type == &WzStringType::Empty {
            return Vec::new();
        }
        let mut keys = self.keys.write().unwrap();
        encrypt_str(&mut keys, str, meta_type)
    }
}

impl WzBaseReader<Mmap> {
    pub fn from_buff(buff: &[u8]) -> Self {
        let is_empty = buff.is_empty();
        let len = if is_empty { 1 } else { buff.len() };
        let mut memmap = memmap2::MmapMut::map_anon(len).unwrap();
        if !is_empty {
            memmap.copy_from_slice(buff);
        }
        WzReader {
            map: memmap.make_read_only().unwrap(),
            keys: Arc::new(RwLock::new(WzMutableKey::new([0; 4], [0; 32]))),
            wz_iv: [0; 4],
        }
    }
}

impl<'a> WzSliceReader<'a> {
    pub fn new(buf: &'a [u8], key: &Arc<RwLock<WzMutableKey>>) -> Self {
        WzSliceReader {
            buf,
            pos: Cell::new(0),
            _save_pos: Cell::new(0),
            header: Default::default(),
            keys: Arc::clone(key),
        }
    }
    #[inline]
    pub fn with_header(self, header: WzHeader<'a>) -> Self {
        WzSliceReader { header, ..self }
    }
    #[inline]
    pub fn get_slice(&self, range: std::ops::Range<usize>) -> &[u8] {
        &self.buf[range]
    }
    #[inline]
    pub fn get_slice_from_current(&self, len: usize) -> &[u8] {
        &self.buf[self.pos.get()..self.pos.get() + len]
    }
    #[inline]
    pub fn is_valid_pos(&self, pos: usize) -> bool {
        pos <= self.get_size()
    }
    #[inline]
    pub fn available(&self) -> usize {
        self.get_size() - self.pos.get()
    }
    #[inline]
    pub fn seek(&self, pos: usize) {
        self.pos.set(pos);
    }
    #[inline]
    pub fn skip(&self, len: usize) {
        self.pos.set(self.pos.get() + len);
    }
    #[inline]
    pub fn save_pos(&self) {
        self._save_pos.set(self.pos.get());
    }
    #[inline]
    pub fn restore_pos(&self) {
        self.pos.set(self._save_pos.get());
    }
    #[inline]
    pub fn read_u8(&self) -> Result<u8> {
        let res = self.read_u8_at(self.pos.get());
        self.pos.set(self.pos.get() + 1);
        res
    }
    #[inline]
    pub fn read_u16(&self) -> Result<u16> {
        let res = self.read_u16_at(self.pos.get());
        self.pos.set(self.pos.get() + 2);
        res
    }
    #[inline]
    pub fn read_u32(&self) -> Result<u32> {
        let res = self.read_u32_at(self.pos.get());
        self.pos.set(self.pos.get() + 4);
        res
    }
    #[inline]
    pub fn read_u64(&self) -> Result<u64> {
        let res = self.read_u64_at(self.pos.get());
        self.pos.set(self.pos.get() + 8);
        res
    }
    #[inline]
    pub fn read_i8(&self) -> Result<i8> {
        let res = self.read_i8_at(self.pos.get());
        self.pos.set(self.pos.get() + 1);
        res
    }
    #[inline]
    pub fn read_i16(&self) -> Result<i16> {
        let res = self.read_i16_at(self.pos.get());
        self.pos.set(self.pos.get() + 2);
        res
    }
    #[inline]
    pub fn read_i32(&self) -> Result<i32> {
        let res = self.read_i32_at(self.pos.get());
        self.pos.set(self.pos.get() + 4);
        res
    }
    #[inline]
    pub fn read_i64(&self) -> Result<i64> {
        let res = self.read_i64_at(self.pos.get());
        self.pos.set(self.pos.get() + 8);
        res
    }
    #[inline]
    pub fn read_float(&self) -> Result<f32> {
        let res = self.read_float_at(self.pos.get());
        self.pos.set(self.pos.get() + 4);
        res
    }
    #[inline]
    pub fn read_double(&self) -> Result<f64> {
        let res = self.read_double_at(self.pos.get());
        self.pos.set(self.pos.get() + 8);
        res
    }
    #[inline]
    pub fn read_unicode_str_len(&self, sl: i8) -> Result<i32> {
        if sl == i8::MAX {
            self.read_i32()
        } else {
            Ok(sl as i32)
        }
    }
    #[inline]
    pub fn read_ascii_str_len(&self, sl: i8) -> Result<i32> {
        if sl == i8::MIN {
            self.read_i32()
        } else {
            Ok((-sl).into())
        }
    }
    pub fn read_unicode_string(&self, sl: i8) -> Result<String> {
        let len = self.read_unicode_str_len(sl)?;

        if len == 0 {
            return Ok(String::new());
        }

        let unicode_u8_len = (len * 2) as usize;

        let string =
            self.resolve_wz_string_meta(&WzStringType::Unicode, self.pos.get(), unicode_u8_len)?;

        self.skip(unicode_u8_len);

        Ok(string)
    }
    pub fn read_ascii_string(&self, sl: i8) -> Result<String> {
        let len = self.read_ascii_str_len(sl)? as usize;
        if len == 0 {
            return Ok(String::new());
        }

        let string = self.resolve_wz_string_meta(&WzStringType::Ascii, self.pos.get(), len)?;

        self.skip(len);

        Ok(string)
    }
    #[inline]
    pub fn read_wz_string_meta_at(&self, offset: usize) -> Result<WzStringMeta> {
        self.save_pos();

        self.pos.set(offset);
        let meta = self.read_wz_string_meta();

        self.restore_pos();
        meta
    }
    pub fn read_wz_string_meta(&self) -> Result<WzStringMeta> {
        let small_len = self.read_i8()?;

        let string_type = self.get_wz_string_type(small_len);

        match string_type {
            WzStringType::Empty => Ok(WzStringMeta::empty()),
            WzStringType::Unicode => {
                if small_len == i8::MAX {
                    let length = self.read_i32()? as u32 * 2;
                    /* remember skip char reading */
                    let meta = WzStringMeta::new_unicode(self.pos.get(), length);
                    self.skip(length as usize);
                    Ok(meta)
                } else {
                    let length = small_len as u32 * 2;
                    let meta = WzStringMeta::new_unicode(self.pos.get(), length);
                    self.skip(length as usize);
                    Ok(meta)
                }
            }
            WzStringType::Ascii => {
                if small_len == i8::MIN {
                    let length = self.read_i32()?;
                    let meta = WzStringMeta::new_ascii(self.pos.get(), length as u32);
                    self.skip(length as usize);
                    Ok(meta)
                } else {
                    let length = (-small_len) as u32;
                    let meta = WzStringMeta::new_ascii(self.pos.get(), length);
                    self.skip(length as usize);
                    Ok(meta)
                }
            }
        }
    }
    #[inline]
    pub fn read_wz_string(&self) -> Result<String> {
        let small_len = self.read_i8()?;

        match self.get_wz_string_type(small_len) {
            WzStringType::Empty => Ok(String::new()),
            WzStringType::Unicode => self.read_unicode_string(small_len),
            WzStringType::Ascii => self.read_ascii_string(small_len),
        }
    }
    #[inline]
    pub fn read_wz_string_at_offset(&self, offset: usize) -> Result<String> {
        self.save_pos();

        self.pos.set(offset);
        let string = self.read_wz_string();

        self.restore_pos();
        string
    }
    #[inline]
    pub fn read_wz_string_block(&self, offset: usize) -> Result<String> {
        let string_type = self.read_u8()?;

        match string_type {
            0 | 0x73 => self.read_wz_string(),
            1 | 0x1B => {
                let append_offset = self.read_i32()?;
                self.read_wz_string_at_offset(offset + append_offset as usize)
            }
            _ => Ok(String::new()),
        }
    }
    #[inline]
    pub fn read_wz_string_block_meta(&self, wz_img_offset: usize) -> Result<WzStringMeta> {
        let string_type = self.read_u8()?;

        match string_type {
            0 | 0x73 => self.read_wz_string_meta(),
            1 | 0x1B => {
                let append_offset = self.read_i32()?;
                self.read_wz_string_meta_at(wz_img_offset + append_offset as usize)
            }
            _ => Ok(WzStringMeta::empty()),
        }
    }
    #[inline]
    pub fn read_wz_int(&self) -> Result<i32> {
        let small_len = self.read_i8()?;

        if small_len == i8::MIN {
            return self.read_i32();
        }

        Ok(small_len as i32)
    }
    #[inline]
    pub fn read_wz_int64(&self) -> Result<i64> {
        let small_len = self.read_i8()?;

        if small_len == i8::MIN {
            return self.read_i64();
        }

        Ok(small_len as i64)
    }
    #[inline]
    pub fn read_wz_long(&self) -> Result<i64> {
        self.read_wz_int64()
    }
    #[inline]
    pub fn read_wz_offset(&self, hash: usize, offset: Option<usize>) -> Result<usize> {
        // let offset: usize = self.pos.get();
        let offset = offset.unwrap_or(self.pos.get());

        let fstart = self.header.fstart;

        let offset = offset.wrapping_sub(fstart) ^ 0xFFFFFFFF;
        let offset = offset.wrapping_mul(hash) & 0xFFFFFFFF;
        let offset = offset.wrapping_sub(WZ_OFFSET as usize);
        let offset = (offset as i32).rotate_left((offset as u32) & 0x1F) as usize & 0xFFFFFFFF;

        let encrypted_offset = self.read_u32()? as usize;
        let offset = (offset ^ encrypted_offset) & 0xFFFFFFFF;
        let offset = offset.wrapping_add(fstart * 2) & 0xFFFFFFFF;

        Ok(offset)
    }
}

impl<T: AsRef<[u8]>> Reader for WzBaseReader<T> {
    #[inline]
    fn read_u8_at(&self, pos: usize) -> Result<u8> {
        self.map
            .as_ref()
            .pread_with::<u8>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_u16_at(&self, pos: usize) -> Result<u16> {
        self.map
            .as_ref()
            .pread_with::<u16>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_u32_at(&self, pos: usize) -> Result<u32> {
        self.map
            .as_ref()
            .pread_with::<u32>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_u64_at(&self, pos: usize) -> Result<u64> {
        self.map
            .as_ref()
            .pread_with::<u64>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_i8_at(&self, pos: usize) -> Result<i8> {
        self.map
            .as_ref()
            .pread_with::<i8>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_i16_at(&self, pos: usize) -> Result<i16> {
        self.map
            .as_ref()
            .pread_with::<i16>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_i32_at(&self, pos: usize) -> Result<i32> {
        self.map
            .as_ref()
            .pread_with::<i32>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_i64_at(&self, pos: usize) -> Result<i64> {
        self.map
            .as_ref()
            .pread_with::<i64>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn read_float_at(&self, pos: usize) -> Result<f32> {
        self.map
            .as_ref()
            .pread_with::<f32>(pos, LE)
            .map_err(Error::from)
    }
    fn read_double_at(&self, pos: usize) -> Result<f64> {
        self.map
            .as_ref()
            .pread_with::<f64>(pos, LE)
            .map_err(Error::from)
    }
    #[inline]
    fn get_size(&self) -> usize {
        self.map.as_ref().len()
    }
    #[inline]
    fn get_decrypt_slice(&self, range: std::ops::Range<usize>) -> Result<Vec<u8>> {
        let len = range.len();
        get_decrypt_slice(&self.map.as_ref()[range], len, &self.keys)
    }
}

impl<'a> Reader for WzSliceReader<'a> {
    #[inline]
    fn get_size(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    fn read_u8_at(&self, pos: usize) -> Result<u8> {
        self.buf.pread_with::<u8>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_u16_at(&self, pos: usize) -> Result<u16> {
        self.buf.pread_with::<u16>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_u32_at(&self, pos: usize) -> Result<u32> {
        self.buf.pread_with::<u32>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_u64_at(&self, pos: usize) -> Result<u64> {
        self.buf.pread_with::<u64>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_i8_at(&self, pos: usize) -> Result<i8> {
        self.buf.pread_with::<i8>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_i16_at(&self, pos: usize) -> Result<i16> {
        self.buf.pread_with::<i16>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_i32_at(&self, pos: usize) -> Result<i32> {
        self.buf.pread_with::<i32>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_i64_at(&self, pos: usize) -> Result<i64> {
        self.buf.pread_with::<i64>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_float_at(&self, pos: usize) -> Result<f32> {
        self.buf.pread_with::<f32>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn read_double_at(&self, pos: usize) -> Result<f64> {
        self.buf.pread_with::<f64>(pos, LE).map_err(Error::from)
    }
    #[inline]
    fn get_decrypt_slice(&self, range: std::ops::Range<usize>) -> Result<Vec<u8>> {
        let len = range.len();
        get_decrypt_slice(&self.buf[range], len, &self.keys)
    }
}

#[inline]
pub fn read_u8_at(buf: &[u8], pos: usize) -> Result<u8> {
    buf.pread_with::<u8>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_u16_at(buf: &[u8], pos: usize) -> Result<u16> {
    buf.pread_with::<u16>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_u32_at(buf: &[u8], pos: usize) -> Result<u32> {
    buf.pread_with::<u32>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_u64_at(buf: &[u8], pos: usize) -> Result<u64> {
    buf.pread_with::<u64>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_i8_at(buf: &[u8], pos: usize) -> Result<i8> {
    buf.pread_with::<i8>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_i16_at(buf: &[u8], pos: usize) -> Result<i16> {
    buf.pread_with::<i16>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_i32_at(buf: &[u8], pos: usize) -> Result<i32> {
    buf.pread_with::<i32>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_i64_at(buf: &[u8], pos: usize) -> Result<i64> {
    buf.pread_with::<i64>(pos, LE).map_err(Error::from)
}
#[inline]
pub fn read_string_by_len(buf: &[u8], len: usize, offset: Option<usize>) -> String {
    let offset = offset.unwrap_or(0);
    let strvec: Vec<u8> = (0..len).map(|index| buf[offset + index]).collect();

    String::from_utf8_lossy(&strvec).to_string()
}
#[inline]
pub fn read_wz_string(buf: &[u8]) -> Result<String> {
    let small_len = read_i8_at(buf, 0)?;

    if small_len == 0 {
        return Ok(String::new());
    }

    if small_len > 0 {
        return read_unicode_string(&buf[1..], small_len);
    }
    read_ascii_string(&buf[1..], small_len)
}
#[inline]
pub fn read_wz_string_block(buf: &[u8], offset: usize) -> Result<String> {
    let string_type = read_u8_at(buf, 0)?;

    match string_type {
        0 | 0x73 => read_wz_string(&buf[1..]),
        1 | 0x1B => {
            let append_offset = read_i32_at(buf, 1)? as usize;
            read_wz_string(&buf[append_offset + offset..])
        }
        _ => Ok(String::new()),
    }
}
#[inline]
pub fn read_wz_int(buf: &[u8], offset: Option<usize>) -> Result<i32> {
    let offset = offset.unwrap_or(0);
    let small_len = read_i8_at(buf, offset)?;

    if small_len == i8::MIN {
        return read_i32_at(buf, offset + 1);
    }

    Ok(small_len as i32)
}
#[inline]
pub fn read_wz_int64(buf: &[u8], offset: Option<usize>) -> Result<i64> {
    let offset = offset.unwrap_or(0);
    let small_len = read_i8_at(buf, offset)?;

    if small_len == i8::MIN {
        return read_i64_at(buf, offset + 1);
    }

    Ok(small_len as i64)
}
#[inline]
pub fn read_wz_long(buf: &[u8], offset: Option<usize>) -> Result<i64> {
    read_wz_int64(buf, offset)
}

pub fn read_wz_offset(
    buf: &[u8],
    encrypted_offset: usize,
    fstart: usize,
    offset: usize,
    hash: usize,
) -> Result<usize> {
    let offset = offset.wrapping_sub(fstart) ^ 0xFFFFFFFF;
    let offset = offset.wrapping_mul(hash) & 0xFFFFFFFF;
    let offset = offset.wrapping_sub(WZ_OFFSET as usize);
    let offset = offset.rotate_left((offset as u32) & 0x1F) & 0xFFFFFFFF;

    let encrypted_offset = buf.pread_with::<u32>(encrypted_offset, LE)?;
    let offset = (offset ^ encrypted_offset as usize) & 0xFFFFFFFF;
    let offset = offset.wrapping_add(fstart * 2) & 0xFFFFFFFF;

    Ok(offset)
}

pub fn read_unicode_string(buf: &[u8], sl: i8) -> Result<String> {
    let len;
    let mut offset: i32 = 0;

    if sl == i8::MAX {
        len = read_i32_at(buf, 0)?;
        offset = 4;
    } else {
        len = sl as i32;
    }

    if len == 0 {
        return Ok(String::new());
    }

    let strvec: Vec<u16> = (0..len)
        .map(|i| resolve_unicode_char(read_u16_at(buf, (i * 2 + offset) as usize).unwrap(), i))
        .collect();

    Ok(String::from_utf16_lossy(&strvec).to_string())
}

pub fn read_ascii_string(buf: &[u8], sl: i8) -> Result<String> {
    let len: i32;
    let mut offset: i32 = 0;

    if sl == i8::MIN {
        len = read_i32_at(buf, 0)?;
        offset = 4;
    } else {
        len = (-sl).into();
    }

    if len == 0 {
        return Ok(String::new());
    }

    let strvec: Vec<u8> = (0..len)
        .map(|i| resolve_ascii_char(read_u8_at(buf, (i + offset) as usize).unwrap(), i))
        .collect();

    Ok(String::from_utf8_lossy(&strvec).to_string())
}

#[inline]
fn resolve_ascii_char(c: u8, i: i32) -> u8 {
    c ^ (i as u8).wrapping_add(0xAA)
}
#[inline]
fn resolve_unicode_char(c: u16, i: i32) -> u16 {
    c ^ (i as u16).wrapping_add(0xAAAA)
}

pub fn get_decrypt_slice(
    buf: &[u8],
    len: usize,
    keys: &Arc<RwLock<WzMutableKey>>,
) -> Result<Vec<u8>> {
    let is_need_mut = {
        let read = keys.read().unwrap();
        !read.is_enough(len) && !read.without_decrypt
    };

    if is_need_mut {
        let mut key = keys.write().unwrap();
        key.ensure_key_size(len)
            .map_err(|_| Error::DecryptError(len))?;
    }

    let keys = keys.read().unwrap();

    let mut original = buf.to_vec();

    if !keys.without_decrypt {
        keys.decrypt_slice(&mut original);
    }

    Ok(original)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::maple_crypto_constants::{WZ_GMSIV, WZ_MSEAIV};
    use crate::util::WzMutableKey;

    type Error = Box<dyn std::error::Error>;
    type Result<T> = std::result::Result<T, Error>;

    type WzVecReader = WzBaseReader<Vec<u8>>;

    fn generate_ascii_string(len: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(len as usize);
        for i in 0..len {
            buf.push(((0xAA + i) ^ 97) as u8);
        }
        buf
    }
    fn generate_unicode_string(len: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(len as usize);
        for i in 0..len {
            let encrypt_str = ((0xAAAA + i) ^ 97) as u16;
            buf.extend_from_slice(&encrypt_str.to_le_bytes());
        }
        buf
    }
    fn generate_encrypted_ascii_string(len: i32, iv: [u8; 4]) -> Result<Vec<u8>> {
        let mut key = WzMutableKey::from_iv(iv);

        key.ensure_key_size(len as usize)?;

        let mut buf = Vec::with_capacity(len as usize);
        for i in 0..len {
            let key: i32 = *(key.try_at(i as usize).unwrap_or(&0)) as i32;
            buf.push(((0xAA + i) ^ 97 ^ key) as u8);
        }

        Ok(buf)
    }
    fn generate_encrypted_unicode_string(len: i32, iv: [u8; 4]) -> Result<Vec<u8>> {
        let mut key = WzMutableKey::from_iv(iv);

        key.ensure_key_size(len as usize)?;

        let mut buf = Vec::with_capacity(len as usize);
        for i in 0..len {
            let key1 = *(key.try_at((i * 2) as usize).unwrap_or(&0)) as i32;
            let key2 = *(key.try_at((i * 2) as usize + 1).unwrap_or(&0)) as i32;
            let key = (key2 << 8) + key1;
            let encrypt_str = ((0xAAAA + i) ^ 97 ^ key) as u16;
            buf.extend_from_slice(&encrypt_str.to_le_bytes());
        }

        Ok(buf)
    }

    fn setup() -> Result<Vec<u8>> {
        let mut setup_vec = Vec::with_capacity(1024);

        let mock_wz_header = [
            0x50, 0x4b, 0x47, 0x31, // PKG1
            0x6c, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // fsize
            0x3c, 0x00, 0x00, 0x00, // fstart
            // copyright Package file v1.0 Copyright 2002 Wizet, {REGION}
            0x50, 0x61, 0x63, 0x6b, 0x61, 0x67, 0x65, 0x20, 0x66, 0x69, 0x6c, 0x65, 0x20, 0x76,
            0x31, 0x2e, 0x30, 0x20, 0x43, 0x6f, 0x70, 0x79, 0x72, 0x69, 0x67, 0x68, 0x74, 0x20,
            0x32, 0x30, 0x30, 0x32, 0x20, 0x57, 0x69, 0x7a, 0x65, 0x74, 0x2c, 0x20, 0x5a, 0x4d,
            0x53, 0x00, // 0x00 is string end mark
        ];

        let mock_data = [
            // i8, i16, i32, i64
            0x01, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, // u8, u16, u32, u64
            0x01, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, // f32(1.1), f64(2.22)
            0xcd, 0xcc, 0x8c, 0x3f, 0xc3, 0xf5, 0x28, 0x5c, 0x8f, 0xc2, 0x01, 0x40,
            // wz_int without i32
            0x01, // wz_int with i32
            0x80, 0x02, 0x00, 0x00, 0x00, // wz_int64 without i64
            0x01, // wz_int64 with i64
            0x80, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // 60
        setup_vec.extend_from_slice(&mock_wz_header);

        //58
        setup_vec.extend_from_slice(&mock_data);

        // empty string
        setup_vec.extend_from_slice(&[0x00]);

        // len = 20 ascii
        setup_vec.extend_from_slice(&(-20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_ascii_string(20));

        // len = 200 ascii
        setup_vec.extend_from_slice(&(i8::MIN).to_le_bytes());
        setup_vec.extend_from_slice(&200_i32.to_le_bytes());
        setup_vec.extend_from_slice(&generate_ascii_string(200));

        // len = 20 unicode
        setup_vec.extend_from_slice(&(20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_unicode_string(20));

        // len = 200 unicode
        setup_vec.extend_from_slice(&(i8::MAX).to_le_bytes());
        setup_vec.extend_from_slice(&200_i32.to_le_bytes());
        setup_vec.extend_from_slice(&generate_unicode_string(200));

        // len = 20 encrypted(GMS_OLD) ascii
        setup_vec.extend_from_slice(&(-20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_encrypted_ascii_string(20, WZ_GMSIV)?);

        // len = 20 encrypted(GMS_OLD) unicode
        setup_vec.extend_from_slice(&(20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_encrypted_unicode_string(20, WZ_GMSIV)?);

        // len = 20 encrypted(MSEA) ascii
        setup_vec.extend_from_slice(&(-20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_encrypted_ascii_string(20, WZ_MSEAIV)?);

        // len = 20 encrypted(MSEA) unicode
        setup_vec.extend_from_slice(&(20_i8).to_le_bytes());
        setup_vec.extend_from_slice(&generate_encrypted_unicode_string(20, WZ_MSEAIV)?);

        Ok(setup_vec)
    }

    #[test]
    fn test_wz_header() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let wz_header = reader.create_header();
        assert_eq!(wz_header.ident, "PKG1");
        assert_eq!(wz_header.fsize, 364);
        assert_eq!(wz_header.fstart, 60);
        assert_eq!(
            wz_header.copyright,
            "Package file v1.0 Copyright 2002 Wizet, ZMS"
        );

        Ok(())
    }

    #[test]
    fn test_wz_create_encrypt_str_ascii() -> Result<()> {
        let mut reader = WzVecReader::new(Vec::new());
        let test1 = "test1";
        reader.map = reader.encrypt_str(test1, &WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&WzStringType::Ascii, 0, 5)?,
            test1.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_wz_create_encrypt_str_unicode() -> Result<()> {
        let mut reader = WzVecReader::new(Vec::new());
        let test1 = "測試";
        reader.map = reader.encrypt_str(test1, &WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&WzStringType::Unicode, 0, 4)?,
            test1.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_wz_create_encrypt_str_ascii_with_iv() -> Result<()> {
        let mut reader = WzVecReader::new(Vec::new()).with_iv(WZ_MSEAIV);
        let test1 = "test1";
        reader.map = reader.encrypt_str(test1, &WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&WzStringType::Ascii, 0, 5)?,
            test1.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_wz_create_encrypt_str_unicode_with_iv() -> Result<()> {
        let mut reader = WzVecReader::new(Vec::new()).with_iv(WZ_MSEAIV);
        let test1 = "測試";
        reader.map = reader.encrypt_str(test1, &WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&WzStringType::Unicode, 0, 4)?,
            test1.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_wz_signed() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        assert_eq!(reader.read_i8_at(60)?, 1);
        assert_eq!(reader.read_i16_at(61)?, 2);
        assert_eq!(reader.read_i32_at(63)?, 3);
        assert_eq!(reader.read_i64_at(67)?, 4);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(60);

        assert_eq!(slice_reader.read_i8()?, 1);
        assert_eq!(slice_reader.read_i16()?, 2);
        assert_eq!(slice_reader.read_i32()?, 3);
        assert_eq!(slice_reader.read_i64()?, 4);

        Ok(())
    }

    #[test]
    fn test_wz_unsigned() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        assert_eq!(reader.read_u8_at(75)?, 1);
        assert_eq!(reader.read_u16_at(76)?, 2);
        assert_eq!(reader.read_u32_at(78)?, 3);
        assert_eq!(reader.read_u64_at(82)?, 4);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(75);

        assert_eq!(slice_reader.read_u8()?, 1);
        assert_eq!(slice_reader.read_u16()?, 2);
        assert_eq!(slice_reader.read_u32()?, 3);
        assert_eq!(slice_reader.read_u64()?, 4);

        Ok(())
    }

    #[test]
    fn test_wz_float() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        assert_eq!(reader.read_float_at(90)?, 1.1);
        assert_eq!(reader.read_double_at(94)?, 2.22);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(90);

        assert_eq!(slice_reader.read_float()?, 1.1);
        assert_eq!(slice_reader.read_double()?, 2.22);

        Ok(())
    }

    #[test]
    fn test_wz_int() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(102);

        assert_eq!(slice_reader.read_wz_int()?, 1);
        assert_eq!(slice_reader.read_wz_int()?, 2);

        Ok(())
    }

    #[test]
    fn test_wz_int64() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(108);

        assert_eq!(slice_reader.read_wz_int64()?, 1);
        assert_eq!(slice_reader.read_wz_int64()?, 2);

        Ok(())
    }

    #[test]
    fn test_wz_empty_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(118);

        assert_eq!(slice_reader.read_wz_string()?, "");

        let meta = slice_reader.read_wz_string_meta_at(114)?;

        assert_eq!(meta.length, 0);
        assert_eq!(meta.offset, 0);
        assert_eq!(meta.string_type, WzStringType::Empty);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            ""
        );

        Ok(())
    }

    #[test]
    fn test_wz_ascii_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(119);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(119)?;

        assert_eq!(meta.length, 20);
        assert_eq!(meta.offset, 120);
        assert_eq!(meta.string_type, WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_ascii_string_gt_128() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(140);

        let result_string = "a".repeat(200);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(140)?;

        assert_eq!(meta.length, 200);
        assert_eq!(meta.offset, 145);
        assert_eq!(meta.string_type, WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_unicode_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(345);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(345)?;

        assert_eq!(meta.length, 40);
        assert_eq!(meta.offset, 346);
        assert_eq!(meta.string_type, WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_unicode_string_gt_128() -> Result<()> {
        let reader = WzVecReader::new(setup()?);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(386);

        let result_string = "a".repeat(200);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(386)?;

        assert_eq!(meta.length, 400);
        assert_eq!(meta.offset, 391);
        assert_eq!(meta.string_type, WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_encrypted_gms_ascii_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?).with_iv(WZ_GMSIV);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(791);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(791)?;

        assert_eq!(meta.length, 20);
        assert_eq!(meta.offset, 792);
        assert_eq!(meta.string_type, WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_encrypted_gms_unicode_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?).with_iv(WZ_GMSIV);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(812);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(812)?;

        assert_eq!(meta.length, 40);
        assert_eq!(meta.offset, 813);
        assert_eq!(meta.string_type, WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_encrypted_msea_ascii_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?).with_iv(WZ_MSEAIV);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(853);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(853)?;

        assert_eq!(meta.length, 20);
        assert_eq!(meta.offset, 854);
        assert_eq!(meta.string_type, WzStringType::Ascii);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }

    #[test]
    fn test_wz_encrypted_msea_unicode_string() -> Result<()> {
        let reader = WzVecReader::new(setup()?).with_iv(WZ_MSEAIV);

        let slice_reader = reader.create_slice_reader();

        slice_reader.seek(874);

        let result_string = "a".repeat(20);

        assert_eq!(slice_reader.read_wz_string()?, result_string);

        let meta = slice_reader.read_wz_string_meta_at(874)?;

        assert_eq!(meta.length, 40);
        assert_eq!(meta.offset, 875);
        assert_eq!(meta.string_type, WzStringType::Unicode);

        assert_eq!(
            reader.resolve_wz_string_meta(&meta.string_type, meta.offset, meta.length as usize)?,
            result_string
        );

        Ok(())
    }
}
