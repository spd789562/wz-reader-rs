use scroll::{Pread, LE};

use super::snow2_decryptor::Snow2Decryptor;
use crate::reader::Error;

pub(crate) const MS_SNOW2_KEY_SIZE: usize = 16;
pub(crate) const MS_SNOW2_VERSION: u8 = 2;

pub(crate) struct Snow2Reader<'a> {
    data: &'a [u8],
    pub offset: usize,
    decryptor: Snow2Decryptor,
    pub buffer: [u8; 4],
    pub buffer_len: usize,
}

impl<'a> Snow2Reader<'a> {
    pub fn new(data: &'a [u8], snow_key: [u8; MS_SNOW2_KEY_SIZE]) -> Self {
        Self {
            data,
            offset: 0,
            decryptor: Snow2Decryptor::new(snow_key),
            buffer: [0_u8; 4],
            buffer_len: 0,
        }
    }
    #[allow(dead_code)]
    pub fn read_u32(&mut self) -> Result<u32, Error> {
        let decrypted = self
            .decryptor
            .decrypt_block(&self.data.pread_with::<u32>(self.offset, LE)?);
        self.offset += 4;
        if self.buffer_len > 0 {
            let decrypted_bytes = decrypted.to_le_bytes();
            let mut buffer = [0_u8; 4];
            buffer[..self.buffer_len].copy_from_slice(&self.buffer[..self.buffer_len]);
            buffer[self.buffer_len..].copy_from_slice(&decrypted_bytes[..4 - self.buffer_len]);
            let result = buffer.pread_with::<u32>(0, LE)?;
            self.buffer[..(4 - self.buffer_len)]
                .copy_from_slice(&decrypted_bytes[self.buffer_len..]);
            Ok(result)
        } else {
            Ok(decrypted)
        }
    }
    pub fn read_i32(&mut self) -> Result<i32, Error> {
        let decrypted = self
            .decryptor
            .decrypt_block(&self.data.pread_with::<u32>(self.offset, LE)?);
        self.offset += 4;

        let decrypted_bytes = decrypted.to_le_bytes();

        if self.buffer_len > 0 {
            // merge buffer and decrypted_bytes then read u32
            let mut buffer = [0_u8; 4];
            buffer[..self.buffer_len].copy_from_slice(&self.buffer[..self.buffer_len]);
            buffer[self.buffer_len..].copy_from_slice(&decrypted_bytes[..4 - self.buffer_len]);
            let result = buffer.pread_with::<i32>(0, LE)?;
            // deal with remaining bytes
            self.buffer[..(4 - self.buffer_len)]
                .copy_from_slice(&decrypted_bytes[self.buffer_len..]);

            Ok(result)
        } else {
            Ok(i32::from_le_bytes(decrypted_bytes))
        }
    }
    pub fn read_utf16_string(&mut self, len: usize) -> Result<String, Error> {
        let string_vec = self.read_bytes(len)?;
        let utf16_vec = string_vec
            .chunks_exact(2)
            .map(|u| u16::from_le_bytes([u[0], u[1]]))
            .collect::<Vec<_>>();

        String::from_utf16(&utf16_vec).map_err(Error::from)
    }
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        let mut vec = vec![0_u8; len];

        self.write_bytes_to(&mut vec, len)?;

        Ok(vec)
    }
    pub fn write_bytes_to(&mut self, dest_buffer: &mut [u8], len: usize) -> Result<(), Error> {
        let mut remaining_len = len as i32;

        /* first: deal remain buffer from previous */
        if self.buffer_len > 0 {
            if len < self.buffer_len {
                (&mut dest_buffer[0..len]).copy_from_slice(&self.buffer[0..len]);
                self.buffer_len -= len;

                // move the remaining bytes to the beginning of the buffer
                let mut temp_buffer = [0_u8; 4];
                temp_buffer.copy_from_slice(&self.buffer[len..]);

                self.buffer.copy_from_slice(&temp_buffer);

                return Ok(());
            }
            (&mut dest_buffer[0..self.buffer_len])
                .copy_from_slice(&self.buffer[0..self.buffer_len]);
            // self.buffer.fill(0);
            remaining_len -= self.buffer_len as i32;
            self.buffer_len = 0;
        }

        while remaining_len > 0 {
            let decrypted = self
                .decryptor
                .decrypt_block(&self.data.pread_with::<u32>(self.offset, LE)?);
            let bytes = decrypted.to_le_bytes();
            let start = len - remaining_len as usize;
            let dest_range = start..std::cmp::min(start + 4, len);
            let data_len = dest_range.len();
            (&mut dest_buffer[dest_range]).copy_from_slice(&bytes[..data_len]);
            self.offset += 4;
            remaining_len -= 4;

            if remaining_len < 0 {
                let remaining_bytes = &bytes[data_len..];
                self.buffer_len = remaining_bytes.len();
                self.buffer[..self.buffer_len].copy_from_slice(&remaining_bytes);
            }
        }

        Ok(())
    }
}
