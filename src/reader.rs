use scroll::{Pread, LE};
use std::cell::Cell;
use memmap2::Mmap;

use crate::WzHeader;

#[derive(Debug, Clone, PartialEq)]
pub enum WzStringType {
    Ascii,
    Unicode,
    Empty,
}

#[derive(Debug, Clone)]
pub struct WzStringMeta {
    /// string start offset
    pub offset: usize,
    /// string length
    pub length: u32,
    pub string_type: WzStringType,
}

#[derive(Debug)]
pub struct WzReader {
    pub map: Mmap,
    pub hash: Cell<usize>,
}
#[derive(Debug, Clone)]
pub struct WzSliceReader<'a> {
    pub buf: &'a [u8],
    pub pos: Cell<usize>,
    _save_pos: Cell<usize>,
    pub header: WzHeader<'a>,
    pub hash: usize,
}

static WZ_OFFSET: i32 = 0x581C3F6D;

pub trait Reader<'a> {
    fn update_hash(&self, hash: usize);
    fn get_size(&self) -> usize;
    fn get_pos(&self) -> usize;
    fn set_pos(&self, pos: usize);
    fn get_save_pos(&self) -> usize;
    fn set_save_pos(&self, pos: usize);
    fn is_valid_pos(&self, pos: usize) -> bool {
        pos <= self.get_size()
    }
    fn available(&self) -> usize {
        self.get_size() - self.get_pos()
    }
    fn seek(&self, pos: usize) {
        self.set_pos(pos);
    }
    fn skip(&self, len: usize) {
        self.set_pos(self.get_pos() + len);
    }
    fn save_pos(&self) {
        self.set_save_pos(self.get_pos());
    }
    fn restore_pos(&self) {
        self.set_pos(self.get_save_pos());
    }
    fn read_u8(&self) -> Result<u8, scroll::Error> {
        self.set_pos(self.get_pos() + 1);
        self.read_u8_at(self.get_pos() - 1)
    }
    fn read_u16(&self) -> Result<u16, scroll::Error> {
        self.set_pos(self.get_pos() + 2);
        self.read_u16_at(self.get_pos() - 2)
    }
    fn read_u32(&self) -> Result<u32, scroll::Error> {
        self.set_pos(self.get_pos() + 4);
        self.read_u32_at(self.get_pos() - 4)
    }
    fn read_u64(&self) -> Result<u64, scroll::Error> {
        self.set_pos(self.get_pos() + 8);
        self.read_u64_at(self.get_pos() - 8)
    }
    fn read_i8(&self) -> Result<i8, scroll::Error> {
        self.set_pos(self.get_pos() + 1);
        self.read_i8_at(self.get_pos() - 1)
    }
    fn read_i16(&self) -> Result<i16, scroll::Error> {
        self.set_pos(self.get_pos() + 2);
        self.read_i16_at(self.get_pos() - 2)
    }
    fn read_i32(&self) -> Result<i32, scroll::Error> {
        self.set_pos(self.get_pos() + 4);
        self.read_i32_at(self.get_pos() - 4)
    }
    fn read_i64(&self) -> Result<i64, scroll::Error> {
        self.set_pos(self.get_pos() + 8);
        self.read_i64_at(self.get_pos() - 8)
    }
    fn read_float(&self) -> Result<f32, scroll::Error> {
        self.set_pos(self.get_pos() + 4);
        self.read_float_at(self.get_pos() - 4)
    }
    fn read_double(&self) -> Result<f64, scroll::Error> {
        self.set_pos(self.get_pos() + 8);
        self.read_double_at(self.get_pos() - 8)
    }
    fn read_u8_at(&self, pos: usize) -> Result<u8, scroll::Error>;
    fn read_u16_at(&self, pos: usize) -> Result<u16, scroll::Error>;
    fn read_u32_at(&self, pos: usize) -> Result<u32, scroll::Error>;
    fn read_u64_at(&self, pos: usize) -> Result<u64, scroll::Error>;
    fn read_i8_at(&self, pos: usize) -> Result<i8, scroll::Error>;
    fn read_i16_at(&self, pos: usize) -> Result<i16, scroll::Error>;
    fn read_i32_at(&self, pos: usize) -> Result<i32, scroll::Error>;
    fn read_i64_at(&self, pos: usize) -> Result<i64, scroll::Error>;
    fn read_float_at(&self, pos: usize) -> Result<f32, scroll::Error>;
    fn read_double_at(&self, pos: usize) -> Result<f64, scroll::Error>;

    fn read_string_by_len(&self, len: usize) -> String {
        let strvec: Vec<u8> = (0..len).map(|_| self.read_u8().unwrap()).collect();

        String::from_utf8_lossy(&strvec).to_string()
    }
    fn get_wz_string_type(&self, t: i8) -> WzStringType {
        match t {
            0 => WzStringType::Empty,
            t if t > 0 => WzStringType::Unicode,
            _ => WzStringType::Ascii
        }
    }
    fn read_wz_string_meta_at(&self, offset: usize) -> Result<WzStringMeta, scroll::Error> {
        self.save_pos();

        self.set_pos(offset);
        let meta = self.read_wz_string_meta();
        
        self.restore_pos();
        meta
    }
    fn read_wz_string_meta(&self) -> Result<WzStringMeta, scroll::Error> {
        let small_len = self.read_i8()?;

        let string_type = self.get_wz_string_type(small_len);

        match self.get_wz_string_type(small_len) {
            WzStringType::Empty => {
                Ok(WzStringMeta {
                    offset: 0, // empty string's offset is doesn't matter
                    length: 0,
                    string_type
                })
            },
            WzStringType::Unicode => {
                if small_len == i8::MAX {
                    let length = self.read_i32()? as u32 * 2;
                    /* remember skip char reading */
                    let meta = WzStringMeta {
                        offset: self.get_pos(),
                        length,
                        string_type
                    };
                    self.skip(length as usize);
                    Ok(meta)
                } else {
                    let length = small_len as u32 * 2;
                    let meta = WzStringMeta {
                        offset: self.get_pos(),
                        length,
                        string_type
                    };
                    self.skip(length as usize);
                    Ok(meta)
                }
            },
            WzStringType::Ascii => {
                if small_len == i8::MIN {
                    let length = self.read_i32()?;
                    let meta = WzStringMeta {
                        offset: self.get_pos(),
                        length: length as u32,
                        string_type
                    };
                    self.skip(length as usize);
                    Ok(meta)
                } else {
                    let length = (-small_len) as u32;
                    let meta = WzStringMeta {
                        offset: self.get_pos(),
                        length,
                        string_type
                    };
                    self.skip(length as usize);
                    Ok(meta)
                }
            }
        }
    }

    fn read_wz_string(&self) -> Result<String, scroll::Error> {

        let small_len = self.read_i8()?;

        match self.get_wz_string_type(small_len) {
            WzStringType::Empty => {
                Ok(String::new())
            },
            WzStringType::Unicode => {
                self.read_unicode_string(small_len)
            },
            WzStringType::Ascii => {
                self.read_ascii_string(small_len)
            }
        }
    }
    fn read_wz_string_at_offset(&self, offset: usize) -> Result<String, scroll::Error> {
        self.save_pos();

        self.set_pos(offset);
        let string = self.read_wz_string();

        self.restore_pos();
        string
    }
    fn read_wz_string_block(&self, offset: usize) -> Result<String, scroll::Error> {
        let string_type = self.read_u8()?;

        match string_type {
            0 | 0x73 => {
                self.read_wz_string()
            },
            1 | 0x1B => {
                let append_offset = self.read_i32()?;
                self.read_wz_string_at_offset(offset + append_offset as usize)
            },
            _ => {
                Ok(String::new())
            }
        }
    }
    fn read_wz_string_block_meta(&self, wz_img_offset: usize) -> Result<WzStringMeta, scroll::Error> {
        let string_type = self.read_u8()?;
        
        match string_type {
            0 | 0x73 => {
                self.read_wz_string_meta()
            },
            1 | 0x1B => {
                let append_offset = self.read_i32().unwrap();
                self.read_wz_string_meta_at(wz_img_offset + append_offset as usize)
            },
            _ => {
                Ok(WzStringMeta {
                    offset: self.get_pos(),
                    length: 0,
                    string_type: WzStringType::Empty
                })
            }
        }
    }
    fn resolve_wz_string_meta(&self, meta: &WzStringMeta) -> Result<String, scroll::Error> {
        let offset = meta.offset;
        let length = meta.length as usize;

        match meta.string_type {
            WzStringType::Empty => {
                Ok(String::new())
            },
            WzStringType::Unicode => {
                let mask: i32 = 0xAAAA;
                let len = length / 2;
                let strvec: Vec<u16> = (0..len)
                    .map(|i| {
                        let c = self.read_u16_at(offset + i * 2).unwrap() as i32;
                        (c ^ (mask+ i  as i32)) as u16
                    })
                    .collect();

                Ok(String::from_utf16_lossy(&strvec))
            },
            WzStringType::Ascii => {
                let mask: i32 = 0xAA;
                let strvec: Vec<u8> = (0..length)
                    .map(|i| {
                        let c = self.read_u8_at(offset + i).unwrap() as i32;
                        (c ^ (mask + i as i32)) as u8
                    })
                    .collect();

                Ok(String::from_utf8_lossy(&strvec).to_string())
            }
        }
    }
    
    fn read_wz_int(&self) -> Result<i32, scroll::Error> {
        let small_len = self.read_i8()?;
        
        if small_len == i8::MIN {
            return self.read_i32();
        }

        Ok(small_len as i32)
    }
    fn read_wz_int64(&self) -> Result<i64, scroll::Error> {
        let small_len = self.read_i8()?;
        
        if small_len == i8::MIN {
            return self.read_i64();
        }

        Ok(small_len as i64)
    }
    fn read_wz_long(&self) -> Result<i64, scroll::Error> {
        self.read_wz_int64()
    }

    fn read_wz_offset(&self, offset: Option<usize>) -> Result<usize, scroll::Error>;

    fn read_unicode_str_len_at(&self, pos: usize, sl: i8) -> i32 {
        if sl == i8::MAX {
            self.read_i32_at(pos).unwrap()
        } else {
            sl as i32
        }
    }
    fn read_unicode_str_len(&self, sl: i8) -> i32 {
        if sl == i8::MAX {
            self.read_i32().unwrap()
        } else {
            sl as i32
        }
    }
    fn read_unicode_string(&self, sl: i8) -> Result<String, scroll::Error> {
        let mask: i32 = 0xAAAA;
        let len = self.read_unicode_str_len(sl);

        if len == 0 {
            return Ok(String::new());
        }

        let strvec: Vec<u16> = (0..len)
            .map(|i| {
                let c = self.read_u16().unwrap() as i32;
                (c ^ (mask + i)) as u16
            })
            .collect();

        Ok(String::from_utf16_lossy(&strvec))
    }

    fn read_ascii_str_len_at(&self, pos: usize, sl: i8) -> i32 {
        if sl == i8::MIN {
            self.read_i32_at(pos).unwrap()
        } else {
            (-sl).try_into().unwrap()
        }
    }
    fn read_ascii_str_len(&self, sl: i8) -> i32 {
        if sl == i8::MIN {
            self.read_i32().unwrap()
        } else {
            (-sl).try_into().unwrap()
        }
    }
    fn read_ascii_string(&self, sl: i8) -> Result<String, scroll::Error> {
        let mask = 0xAA;
        let len: i32 = self.read_ascii_str_len(sl);
        if len == 0 {
            return Ok(String::new());
        }

        let strvec: Vec<u8> = (0..len)
            .map(|i| {
                let c = self.read_u8().unwrap() as i32;
                (c ^ (mask + i)) as u8
            })
            .collect();

        Ok(String::from_utf8_lossy(&strvec).to_string())
    }
}

impl WzReader {
    pub fn new(map: Mmap) -> Self {
        WzReader {
            map,
            hash: Cell::new(0),
        }
    }
    
    pub fn create_header(&self) -> WzHeader {
        self.map.pread::<WzHeader>(0).unwrap()
    }
    pub fn get_ref_slice(&self) -> &[u8] {
        &self.map
    }
    pub fn get_slice(&self, range: (usize, usize)) -> &[u8] {
        &self.map[range.0..range.1]
    }
    pub fn get_wz_fstart(&self) -> Result<u32, scroll::Error> {
        WzHeader::get_wz_fstart(&self.map)
    }
    pub fn get_wz_fsize(&self) -> Result<u64, scroll::Error> {
        WzHeader::get_wz_fsize(&self.map)
    }
    pub fn create_slice_reader(&self) -> WzSliceReader {
        WzSliceReader::new_with_existing_header(&self.map, self.create_header(), Some(self.hash.get()))
    }
}

impl<'a> WzSliceReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let header = buf.pread::<WzHeader>(0).unwrap();
        WzSliceReader {
            buf,
            pos: Cell::new(0),
            _save_pos: Cell::new(0),
            header,
            hash: 0
        }
    }
    pub fn new_with_existing_header(buf: &'a [u8], header: WzHeader<'a>, hash: Option<usize>) -> Self {
        WzSliceReader {
            buf,
            pos: Cell::new(0),
            _save_pos: Cell::new(0),
            header: header.to_owned(),
            hash: hash.unwrap_or(0)
        }
    }
    pub fn get_slice(&self, range: (usize, usize)) -> &[u8] {
        &self.buf[range.0..range.1]
    }
    pub fn get_slice_from_current(&self, len: usize) -> &[u8] {
        &self.buf[self.get_pos()..self.get_pos() + len]
    }
}

impl<'a> Reader<'a> for WzReader {
    fn update_hash(&self, hash: usize) {
        self.hash.set(hash);
    }
    fn read_u8_at(&self, pos: usize) -> Result<u8, scroll::Error> {
        self.map.pread_with::<u8>(pos, LE)
    }
    fn read_u16_at(&self, pos: usize) -> Result<u16, scroll::Error> {
        self.map.pread_with::<u16>(pos, LE)
    }
    fn read_u32_at(&self, pos: usize) -> Result<u32, scroll::Error> {
        self.map.pread_with::<u32>(pos, LE)
    }
    fn read_u64_at(&self, pos: usize) -> Result<u64, scroll::Error> {
        self.map.pread_with::<u64>(pos, LE)
    }
    fn read_i8_at(&self, pos: usize) -> Result<i8, scroll::Error> {
        self.map.pread_with::<i8>(pos, LE)
    }
    fn read_i16_at(&self, pos: usize) -> Result<i16, scroll::Error> {
        self.map.pread_with::<i16>(pos, LE)
    }
    fn read_i32_at(&self, pos: usize) -> Result<i32, scroll::Error> {
        self.map.pread_with::<i32>(pos, LE)
    }
    fn read_i64_at(&self, pos: usize) -> Result<i64, scroll::Error> {
        self.map.pread_with::<i64>(pos, LE)
    }
    fn read_float_at(&self, pos: usize) -> Result<f32, scroll::Error> {
        self.map.pread_with::<f32>(pos, LE)
    }
    fn read_double_at(&self, pos: usize) -> Result<f64, scroll::Error> {
        self.map.pread_with::<f64>(pos, LE)
    }

    fn get_size(&self) -> usize {
        self.map.len()
    }
    fn get_pos(&self) -> usize {
        0
    }
    fn set_pos(&self, _pos: usize) {
        
    }
    fn get_save_pos(&self) -> usize {
        0
    }
    fn set_save_pos(&self, _pos: usize) {
        
    }
    fn read_wz_offset(&self, offset: Option<usize>) -> Result<usize, scroll::Error> {
        // let offset: usize = self.get_pos();
        let offset = offset.unwrap_or(self.get_pos());

        let hash = self.hash.get();
        
        let fstart = WzHeader::get_wz_fstart(&self.map)? as usize;
        let offset = (offset - fstart) ^ 0xFFFFFFFF;
        let offset = (offset * hash) & 0xFFFFFFFF;
        let offset = offset - WZ_OFFSET as usize;
        let offset = offset.rotate_left((offset as u32) & 0x1F) & 0xFFFFFFFF;
        
        let encrypted_offset = self.read_u32()?;
        let offset = (offset ^ encrypted_offset as usize) & 0xFFFFFFFF;
        let offset = (offset + fstart * 2) & 0xFFFFFFFF;
    
        Ok(offset)
    }
}

impl<'a> Reader<'a> for WzSliceReader<'a> {
    fn update_hash(&self, _hash: usize) {}
    fn read_u8_at(&self, pos: usize) -> Result<u8, scroll::Error> {
        self.buf.pread_with::<u8>(pos, LE)
    }
    fn read_u16_at(&self, pos: usize) -> Result<u16, scroll::Error> {
        self.buf.pread_with::<u16>(pos, LE)
    }
    fn read_u32_at(&self, pos: usize) -> Result<u32, scroll::Error> {
        self.buf.pread_with::<u32>(pos, LE)
    }
    fn read_u64_at(&self, pos: usize) -> Result<u64, scroll::Error> {
        self.buf.pread_with::<u64>(pos, LE)
    }
    fn read_i8_at(&self, pos: usize) -> Result<i8, scroll::Error> {
        self.buf.pread_with::<i8>(pos, LE)
    }
    fn read_i16_at(&self, pos: usize) -> Result<i16, scroll::Error> {
        self.buf.pread_with::<i16>(pos, LE)
    }
    fn read_i32_at(&self, pos: usize) -> Result<i32, scroll::Error> {
        self.buf.pread_with::<i32>(pos, LE)
    }
    fn read_i64_at(&self, pos: usize) -> Result<i64, scroll::Error> {
        self.buf.pread_with::<i64>(pos, LE)
    }
    fn read_float_at(&self, pos: usize) -> Result<f32, scroll::Error> {
        self.buf.pread_with::<f32>(pos, LE)
    }
    fn read_double_at(&self, pos: usize) -> Result<f64, scroll::Error> {
        self.buf.pread_with::<f64>(pos, LE)
    }

    fn get_size(&self) -> usize {
        self.buf.len()
    }
    fn get_pos(&self) -> usize {
        self.pos.get()
    }
    fn set_pos(&self, pos: usize) {
        self.pos.set(pos);
    }
    fn get_save_pos(&self) -> usize {
        self._save_pos.get()
    }
    fn set_save_pos(&self, pos: usize) {
        self._save_pos.set(pos);
    }
    fn read_wz_offset(&self, offset: Option<usize>) -> Result<usize, scroll::Error> {
        // let offset: usize = self.get_pos();
        let offset = offset.unwrap_or(self.get_pos());

        let fstart = self.header.fstart;

        let offset = (offset - fstart) ^ 0xFFFFFFFF;
        let offset = (offset * self.hash) & 0xFFFFFFFF;
        let offset = offset - (WZ_OFFSET as usize);
        let offset = (offset as i32).rotate_left((offset as u32) & 0x1F) as usize & 0xFFFFFFFF;
        
        let encrypted_offset = self.read_u32()? as usize;
        let offset = (offset ^ encrypted_offset) & 0xFFFFFFFF;
        let offset = (offset + fstart * 2) & 0xFFFFFFFF;
    
        Ok(offset)
    }
}


pub fn read_u8_at(buf: &[u8], pos: usize) -> Result<u8, scroll::Error> {
    buf.pread_with::<u8>(pos, LE)
}
pub fn read_u16_at(buf: &[u8], pos: usize) -> Result<u16, scroll::Error> {
    buf.pread_with::<u16>(pos, LE)
}
pub fn read_u32_at(buf: &[u8], pos: usize) -> Result<u32, scroll::Error> {
    buf.pread_with::<u32>(pos, LE)
}
pub fn read_u64_at(buf: &[u8], pos: usize) -> Result<u64, scroll::Error> {
    buf.pread_with::<u64>(pos, LE)
}
pub fn read_i8_at(buf: &[u8], pos: usize) -> Result<i8, scroll::Error> {
    buf.pread_with::<i8>(pos, LE)
}
pub fn read_i16_at(buf: &[u8], pos: usize) -> Result<i16, scroll::Error> {
    buf.pread_with::<i16>(pos, LE)
}
pub fn read_i32_at(buf: &[u8], pos: usize) -> Result<i32, scroll::Error> {
    buf.pread_with::<i32>(pos, LE)
}
pub fn read_i64_at(buf: &[u8], pos: usize) -> Result<i64, scroll::Error> {
    buf.pread_with::<i64>(pos, LE)
}
pub fn read_string_by_len(buf: &[u8], len: usize, offset: Option<usize>) -> String {
    let offset = offset.unwrap_or(0);
    let strvec: Vec<u8> = (0..len).map(|index| {
        buf[offset + index]
    }).collect();

    String::from_utf8_lossy(&strvec).to_string()
}
pub fn read_wz_string(buf: &[u8]) -> Result<String, scroll::Error> {

    let small_len = read_i8_at(buf, 0)?;

    if small_len == 0 {
        return Ok(String::new());
    }

    if small_len > 0 {
        return read_unicode_string(&buf[1..], small_len);
    }
    read_ascii_string(&buf[1..], small_len)
}
pub fn read_wz_string_block(buf: &[u8], offset: usize) -> Result<String, scroll::Error> {
    let string_type = read_u8_at(buf, 0)?;
    
    match string_type {
        0 | 0x73 => {
            read_wz_string(&buf[1..])
        },
        1 | 0x1B => {
            let append_offset = read_i32_at(buf, 1)? as usize;
            read_wz_string(&buf[append_offset + offset..])
        },
        _ => {
            Ok(String::new())
        }
    }
}

pub fn read_wz_int(buf: &[u8], offset: Option<usize>) -> Result<i32, scroll::Error> {
    let offset = offset.unwrap_or(0);
    let small_len = read_i8_at(buf, offset)?;
    
    if small_len == i8::MIN {
        return read_i32_at(buf, offset + 1);
    }

    Ok(small_len as i32)
}
pub fn read_wz_int64(buf: &[u8], offset: Option<usize>) -> Result<i64, scroll::Error> {
    let offset = offset.unwrap_or(0);
    let small_len = read_i8_at(buf, offset)?;
    
    if small_len == i8::MIN {
        return read_i64_at(buf, offset + 1);
    }

    Ok(small_len as i64)
}
pub fn read_wz_long(buf: &[u8], offset: Option<usize>) -> Result<i64, scroll::Error> {
    read_wz_int64(buf, offset)
}

pub fn read_wz_offset(buf: &[u8], encrypted_offset: usize, fstart: usize, offset: usize, hash: usize) -> Result<usize, scroll::Error> {
    let offset = (offset - fstart) ^ 0xFFFFFFFF;
    let offset = (offset * hash) & 0xFFFFFFFF;
    let offset = offset - WZ_OFFSET as usize;
    let offset = offset.rotate_left((offset as u32) & 0x1F) & 0xFFFFFFFF;
    
    let encrypted_offset = buf.pread_with::<u32>(encrypted_offset, LE)?;
    let offset = (offset ^ encrypted_offset as usize) & 0xFFFFFFFF;
    let offset = (offset + fstart * 2) & 0xFFFFFFFF;

    Ok(offset)
}

pub fn read_unicode_string(buf: &[u8], sl: i8) -> Result<String, scroll::Error> {
    let mask: i32 = 0xAAAA;
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

    let strvec: Vec<u8> = (0..len)
        .map(|i| {
            let c = read_u16_at(buf, (i * 2 + offset) as usize).unwrap() as i32;
            (c ^ (mask + i)) as u8
        })
        .collect();

    Ok(String::from_utf8_lossy(&strvec).to_string())
}

pub fn read_ascii_string(buf: &[u8], sl: i8) -> Result<String, scroll::Error> {

    let mask: i32 = 0xAA;
    let len: i32;
    let mut offset: i32 = 0;

    if sl == i8::MIN {
        len = read_i32_at(buf, 0)?;
        offset = 4;
    } else {
        len = (-sl).try_into().unwrap();
    }

    if len == 0 {
        return Ok(String::new());
    }

    let strvec: Vec<u8> = (0..len)
        .map(|i| {
            let mut c = read_u8_at(buf, (i + offset) as usize).unwrap() as i32;
            c ^= mask + i;
            c as u8
        })
        .collect();

    Ok(String::from_utf8_lossy(&strvec).to_string())
}