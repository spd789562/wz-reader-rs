use crate::util::string_decryptor::DecrypterType;

use super::Decryptor;

#[derive(Debug)]
pub struct Pkg2Decryptor {
    iv: u32,
    enc_type: DecrypterType,
    keys: [u8; 8],
}

impl Default for Pkg2Decryptor {
    fn default() -> Self {
        Self {
            iv: 0,
            enc_type: DecrypterType::KMST1199,
            keys: [0; 8],
        }
    }
}

impl Pkg2Decryptor {
    pub fn new_with_key(key: u32, enc_type: DecrypterType) -> Self {
        let mut decryptor: Pkg2Decryptor = Self::default();

        decryptor.set_iv(key, enc_type);

        decryptor
    }
    fn calculate_keys(&mut self, key: u32) {
        self.iv = key;

        let k = key.to_le_bytes();

        self.keys[0] = k[0];
        self.keys[1] = k[1];

        self.keys[2] = k[1];
        self.keys[3] = k[2];

        self.keys[4] = k[2];
        self.keys[5] = k[3];

        self.keys[6] = k[3];
        self.keys[7] = 0;
    }
}

impl Decryptor for Pkg2Decryptor {
    fn is_pkg2(&self) -> bool {
        true
    }

    fn set_iv(&mut self, key: u32, enc_type: DecrypterType) {
        self.enc_type = enc_type;
        self.calculate_keys(key);
    }

    fn get_enc_type(&self) -> DecrypterType {
        self.enc_type
    }
    fn get_iv_hash(&self) -> u64 {
        self.iv.into()
    }
    fn is_enough(&self, _size: usize) -> bool {
        true
    }

    fn at(&mut self, index: usize) -> &u8 {
        &self.keys[index % 8]
    }

    fn try_at(&self, index: usize) -> Option<&u8> {
        self.keys.get(index % 8)
    }

    fn decrypt_slice(&self, data: &mut [u8]) {
        for i in 0..data.len() {
            data[i] ^= self.keys[i % 8];
        }
    }

    fn ensure_key_size(&mut self, _size: usize) -> Result<(), String> {
        Ok(())
    }
}

pub fn get_kmst1199_key(hash1: u32, hash_version: u32) -> u32 {
    let base_hash = hash1 ^ hash_version ^ 0x6D4C3B2A;
    mix_kmst1199(mix_kmst1199(base_hash) ^ 0x4F4CB34A)
}

#[inline(always)]
pub(crate) fn mix_kmst1199(mut key: u32) -> u32 {
    key ^= key >> 16;
    key = key.wrapping_mul(0x7FEB352D);
    key ^= key >> 15;
    key = key.wrapping_mul(0x846CA68B);
    key ^ (key >> 16)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pkg2_decryptor() {
        let decryptor = Pkg2Decryptor::new_with_key(0xDEADBEEF, DecrypterType::KMST1198);
        let mut decrypted = b"Hello, world!".to_vec();
        decryptor.decrypt_slice(&mut decrypted);
        decryptor.decrypt_slice(&mut decrypted);
        assert_eq!(decrypted, b"Hello, world!");

        let mut decrypted2 = "你好世界".bytes().collect::<Vec<_>>();

        decryptor.decrypt_slice(&mut decrypted2);
        decryptor.decrypt_slice(&mut decrypted2);
        assert_eq!(decrypted2, "你好世界".bytes().collect::<Vec<_>>());
    }
}
