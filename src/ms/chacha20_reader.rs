use scroll::{Pread, LE};
use c2_chacha::stream_cipher::{NewStreamCipher, SyncStreamCipher};
use c2_chacha::ChaCha20;

use crate::reader::Error;

pub(crate) const MS_CHACHA20_KEY_SIZE: usize = 32;
pub(crate) const MS_CHACHA20_NONCE_SIZE: usize = 12;
pub(crate) const MS_CHACHA20_VERSION: u8 = 4;
pub(crate) const MS_CHACHA20_KEY_BASE: [u8; 32] = [
  0x7B, 0x2F, 0x35, 0x48, 0x43, 0x95, 0x02, 0xB9,
  0xAE, 0x91, 0xA6, 0xE1, 0xD8, 0xD6, 0x24, 0xB4,
  0x33, 0x10, 0x1D, 0x3D, 0xC1, 0xBB, 0xC6, 0xF4,
  0xA5, 0xFE, 0xB3, 0x69, 0x6B, 0x56, 0xE4, 0x75
];

pub(crate) struct ChaCha20Reader<'a> {
  data: &'a [u8],
  pub offset: usize,
  decryptor: ChaCha20,
}

impl<'a> ChaCha20Reader<'a> {
  pub fn new(data: &'a [u8], key: &[u8; MS_CHACHA20_KEY_SIZE], nonce: &[u8; MS_CHACHA20_NONCE_SIZE]) -> Self {
      Self {
          data,
          offset: 0,
          decryptor: ChaCha20::new_var(key, nonce).unwrap(),
      }
  }
  pub fn read_u32(&mut self) -> Result<u32, Error> {
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(&self.data[self.offset..self.offset + 4]);
    self.decryptor.apply_keystream(
      &mut buffer
    );
    self.offset += 4;
    
    buffer.pread_with::<u32>(0, LE).map_err(Error::from)
  }
  pub fn read_i32(&mut self) -> Result<i32, Error> {
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(&self.data[self.offset..self.offset + 4]);
    self.decryptor.apply_keystream(
      &mut buffer
    );
    self.offset += 4;
    buffer.pread_with::<i32>(0, LE).map_err(Error::from)
  }
  pub fn read_utf16_string(&mut self, len: usize) -> Result<String, Error> {
    let utf16_vec = self.read_bytes(len)?
      .chunks_exact(2)
      .map(|u| u16::from_le_bytes([u[0], u[1]]))
      .collect::<Vec<_>>();

    String::from_utf16(&utf16_vec).map_err(Error::from)
  }
  pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = self.data[self.offset..self.offset + len].to_vec();
    self.decryptor.apply_keystream(
      &mut buffer
    );
    self.offset += len;
    Ok(buffer)
  }
  pub fn write_bytes_to(&mut self, dest_buffer: &mut [u8], len: usize) -> Result<(), Error> {
    if len > dest_buffer.len() {
      return Err(Error::DecryptError(len));
    }

    dest_buffer.copy_from_slice(&self.data[self.offset..self.offset + len]);
    self.decryptor.apply_keystream(
      dest_buffer
    );
    self.offset += len;
    Ok(())
  }
}