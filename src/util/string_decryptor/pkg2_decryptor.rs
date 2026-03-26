use crate::util::string_decryptor::DecrypterType;

use super::Decryptor;

#[derive(Debug, Default)]
pub struct Pkg2Decryptor {
    iv: u32,
    keys: [u8; 8],
}

impl Pkg2Decryptor {
    pub fn new_with_key(key: u32) -> Self {
        let mut keys = [0u8; 8];

        for i in 0_usize..4_usize {
            let sh = (key >> (8 * i)) as u16;
            let [k1, k2] = sh.to_le_bytes();
            keys[i as usize * 2] = k1;
            keys[i as usize * 2 + 1] = k2;
        }

        return Self { keys, iv: key };
    }
}

impl Decryptor for Pkg2Decryptor {
    fn is_pkg2(&self) -> bool {
        true
    }

    fn get_enc_type(&self) -> DecrypterType {
        DecrypterType::KMST1198
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pkg2_decryptor() {
        let decryptor = Pkg2Decryptor::new_with_key(0xDEADBEEF);
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
