use aes::Aes256;
use aes::cipher::{KeyInit, BlockSizeUser, block_padding::Pkcs7, BlockEncryptMut};
use crate::reader::read_i32_at;
use super::maple_crypto_constants::{MAPLESTORY_USERKEY_DEFAULT, WZ_MSEAIV, get_trimmed_user_key};

const BATCH_SIZE: f64 = 4096_f64;

#[derive(Debug)]
pub struct WzMutableKey {
    iv: [u8; 4],
    keys: Vec<u8>,
    aes_key: [u8; 32],
}

impl WzMutableKey {
    pub fn new(iv: [u8; 4], aes_key: [u8; 32]) -> Self {
        Self {
            iv,
            keys: vec![],
            aes_key,
        }
    }
    pub fn new_lua() -> Self {
        Self {
            iv: WZ_MSEAIV,
            keys: vec![],
            aes_key: get_trimmed_user_key(&MAPLESTORY_USERKEY_DEFAULT),
        }
    }
    pub fn from_iv(iv: [u8; 4]) -> Self {
        Self {
            iv,
            keys: vec![],
            aes_key: get_trimmed_user_key(&MAPLESTORY_USERKEY_DEFAULT),
        }
    }
    pub fn at(&mut self, index: usize) -> &u8 {
        if self.keys.len() <= index {
            self.ensure_key_size(index + 1).unwrap();
        }
        &self.keys[index]
    }
    pub fn try_at(&self, index: usize) -> Option<&u8> {
        self.keys.get(index)
    }
    pub fn is_enough(&self, size: usize) -> bool {
        self.keys.len() >= size
    }
    pub fn ensure_key_size(&mut self, size: usize) -> Result<(), String> {
        if self.is_enough(size) {
            return Ok(());
        }

        let size = (((size as f64) / BATCH_SIZE).ceil() * BATCH_SIZE) as usize;

        let tmp = read_i32_at(&self.iv, 0).unwrap_or(0);

        if tmp == 0 {
            // self.keys = new_keys;
            return Ok(());
        }

        if self.keys.capacity() < size {
            self.keys.reserve(size - self.keys.capacity());
        }

        // initialize the first block
        if self.keys.len() == 0 {
            self.keys.resize(32, 0);

            let mut block = [0_u8; 16];
            for (index, item) in block.iter_mut().enumerate() {
                *item = self.iv[index % 4];
            }
            ecb::Encryptor::<Aes256>::new(&self.aes_key.into())
                .encrypt_padded_b2b_mut::<Pkcs7>(&block, &mut self.keys)
                .unwrap();

            self.keys.truncate(16);
        }

        let start_index = self.keys.len();

        // fill enouth 0 for later, 
        if self.keys.len() < size {
            // + 16 is prevent encryption not enough to padding
            self.keys.resize(size + 16, 0);
        }

        let block_size = aes::Aes256::block_size();

        for i in (start_index..size).step_by(16) {
            let in_buf = self.keys[i - block_size..i].to_vec();
            ecb::Encryptor::<Aes256>::new(&self.aes_key.into())
                // im not sure why this will actually append block_size * 2 to out_buff, so will be trimed at the end
                .encrypt_padded_b2b_mut::<Pkcs7>(&in_buf, &mut self.keys[i..])
                .unwrap();
        }

        if self.keys.len() > size {
            self.keys.truncate(size);
        }

        Ok(())
    }
}

