use openssl::symm::{Cipher, Crypter, Mode};
use crate::reader::read_i32_at;

const BATCH_SIZE: f64 = 4096_f64;

pub struct WzMutableKey {
    iv: Vec<u8>,
    keys: Vec<u8>,
    aes_key: Vec<u8>,
}

impl WzMutableKey {
    pub fn new(iv: Vec<u8>, aes_key: Vec<u8>) -> Self {
        Self {
            iv,
            keys: vec![],
            aes_key,
        }
    }
    pub fn at(&mut self, index: usize) -> Option<&u8> {
        if self.keys.len() <= index {
            self.ensure_key_size(index + 1).unwrap();
        }
        self.keys.get(index)
    }
    pub fn ensure_key_size(&mut self, size: usize) -> Result<(), String> {
        if self.keys.len() >= size {
            return Ok(());
        }

        self.keys.reserve(size - self.keys.len());

        let size = ((1.0 * (size as f64) / BATCH_SIZE).ceil() * BATCH_SIZE) as usize;

        let tmp = read_i32_at(&self.iv, 0).unwrap_or(0);

        if tmp == 0 {
            // self.keys = new_keys;
            return Ok(());
        }

        let mut start_index = 0;

        self.keys.reserve(size - self.keys.len());

        if self.keys.len() > 0 {
            // new_keys.extend_from_slice(&self.keys);
            start_index = self.keys.len();
        }

        let chiper = Cipher::aes_256_ecb();
        let block_size = chiper.block_size();
        let mut crypter = Crypter::new(chiper, Mode::Encrypt, &self.aes_key, None).unwrap();
        crypter.pad(true);

        for i in (start_index..size).step_by(block_size) {
            if i == 0 {
                let mut block = [0_u8; 16];
                for (index, item) in block.iter_mut().enumerate() {
                    *item = self.iv[index % 4];
                }
                crypter.update(&block, &mut self.keys).unwrap();
            } else {
                crypter.update(&Vec::from(&self.keys[i - block_size..i]), &mut self.keys[i..]).unwrap();
            }
        }


        Ok(())
    }
}

