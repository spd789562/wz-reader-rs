use super::Decryptor;
use crate::util::string_decryptor::DecrypterType;
use crate::util::version::pkg2_kmst1205;

#[derive(Debug, Clone, Copy)]
enum Pkg2Keys {
    Standard([u8; 8]),
    Len16([u8; 16]),
}

impl Default for Pkg2Keys {
    fn default() -> Self {
        Self::Standard([0; 8])
    }
}

impl Pkg2Keys {
    fn from_base_key(key: u64) -> Self {
        Self::Standard(gen_pkg2_keys(key))
    }

    fn from_kmst1206(key: u64) -> Self {
        Self::Len16(gen_pkg2_keys_len16(key))
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Standard(keys) => keys,
            Self::Len16(keys) => keys,
        }
    }
}

#[derive(Debug)]
pub struct Pkg2Decryptor {
    iv: u64,
    enc_type: DecrypterType,
    keys: Pkg2Keys,
    /// KMST1204/1205/1206: position-dependent key material
    hash1: u64,
    hash_version: u64,
}

impl Default for Pkg2Decryptor {
    fn default() -> Self {
        Self {
            iv: 0,
            enc_type: DecrypterType::KMST1199,
            keys: Pkg2Keys::default(),
            hash1: 0,
            hash_version: 0,
        }
    }
}

impl Pkg2Decryptor {
    pub fn new_with_key(key: u64, enc_type: DecrypterType) -> Self {
        let mut decryptor: Pkg2Decryptor = Self::default();

        decryptor.set_iv(key, enc_type);

        decryptor
    }
    fn calculate_keys(&mut self, key: u64) {
        self.iv = key;
        self.keys = Pkg2Keys::from_base_key(key);
    }
}

impl Decryptor for Pkg2Decryptor {
    fn is_pkg2(&self) -> bool {
        true
    }

    fn set_iv(&mut self, key: u64, enc_type: DecrypterType) {
        self.enc_type = enc_type;
        if !matches!(
            enc_type,
            DecrypterType::KMST1204 | DecrypterType::KMST1205 | DecrypterType::KMST1206
        ) {
            self.calculate_keys(key);
        }
    }

    fn set_key_material(&mut self, hash1: u64, hash_version: u64, enc_type: DecrypterType) {
        self.enc_type = enc_type;
        self.hash1 = hash1;
        self.hash_version = hash_version;
        if matches!(
            enc_type,
            DecrypterType::KMST1204 | DecrypterType::KMST1205 | DecrypterType::KMST1206
        ) {
            self.apply_file_position(0);
        } else if enc_type == DecrypterType::KMST1202 {
            self.calculate_keys(get_kmst1202_key(hash1, hash_version));
        } else if enc_type == DecrypterType::KMST1199 {
            self.calculate_keys(get_kmst1199_key(hash1 as u32, hash_version as u32) as u64);
        } else {
            self.calculate_keys(hash_version);
        }
    }

    fn apply_file_position(&mut self, file_position: u64) {
        match self.enc_type {
            DecrypterType::KMST1204 => self.calculate_keys(get_kmst1204_key(
                self.hash1,
                self.hash_version,
                file_position,
            )),
            DecrypterType::KMST1205 => {
                self.calculate_keys(pkg2_kmst1205::compute_dir_entry_name_key(
                    self.hash1,
                    self.hash_version,
                    file_position,
                ))
            }
            DecrypterType::KMST1206 => {
                self.iv = pkg2_kmst1205::compute_dir_entry_name_key(
                    self.hash1,
                    self.hash_version,
                    file_position,
                );
                self.keys = Pkg2Keys::from_kmst1206(self.iv);
            }
            _ => {}
        }
    }

    fn get_enc_type(&self) -> DecrypterType {
        self.enc_type
    }
    fn get_iv_hash(&self) -> u64 {
        self.iv
    }
    fn is_enough(&self, _size: usize) -> bool {
        true
    }

    fn at(&mut self, index: usize) -> &u8 {
        let keys = self.keys.as_slice();
        &keys[index % keys.len()]
    }

    fn try_at(&self, index: usize) -> Option<&u8> {
        let keys = self.keys.as_slice();
        keys.get(index % keys.len())
    }

    fn decrypt_slice(&self, data: &mut [u8]) {
        xor_with_keys(data, self.keys.as_slice());
    }

    fn decrypt_slice_with_offset(&self, data: &mut [u8], offset: u64) {
        let keys = match self.enc_type {
            DecrypterType::KMST1204 => {
                Pkg2Keys::from_base_key(get_kmst1204_key(self.hash1, self.hash_version, offset))
            }
            DecrypterType::KMST1205 => Pkg2Keys::from_base_key(
                pkg2_kmst1205::compute_dir_entry_name_key(self.hash1, self.hash_version, offset),
            ),
            DecrypterType::KMST1206 => Pkg2Keys::from_kmst1206(
                pkg2_kmst1205::compute_dir_entry_name_key(self.hash1, self.hash_version, offset),
            ),
            _ => self.keys,
        };
        xor_with_keys(data, keys.as_slice());
    }

    fn ensure_key_size(&mut self, _size: usize) -> Result<(), String> {
        Ok(())
    }
}

#[inline]
fn xor_with_keys(data: &mut [u8], keys: &[u8]) {
    for (i, item) in data.iter_mut().enumerate() {
        *item ^= keys[i % keys.len()];
    }
}

#[inline]
pub fn gen_pkg2_keys(key: u64) -> [u8; 8] {
    let k = key.to_le_bytes();

    [k[0], k[1], k[1], k[2], k[2], k[3], k[3], k[4]]
}

#[inline]
pub fn gen_pkg2_keys_len16(key: u64) -> [u8; 16] {
    let mut keys = [0; 16];
    for i in 0..8 {
        keys[i * 2..i * 2 + 2].copy_from_slice(&((key >> (8 * i)) as u16).to_le_bytes());
    }
    keys
}

#[inline]
pub fn get_kmst1199_key(hash1: u32, hash_version: u32) -> u32 {
    let base_hash = hash1 ^ hash_version ^ 0x6D4C3B2A;
    mix_kmst1199(mix_kmst1199(base_hash) ^ 0x4F4CB34A)
}

#[inline]
pub fn get_kmst1202_key(hash1: u64, hash_version: u64) -> u64 {
    hash1 ^ hash_version ^ 0x66B57FEE317FD3DF
}

/// KMST1204 position-dependent directory string key.
#[inline]
pub fn get_kmst1204_key(hash1: u64, hash_version: u64, file_position: u64) -> u64 {
    hash1
        ^ hash_version
        ^ 0x21810F65FEC32BDC_u64
        ^ 0x9E3779B97F4A7C15_u64.wrapping_mul(file_position)
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
    fn test_pkg2_decryptor_u32() {
        let key = 0xDEADBEEF_u32;
        let decryptor = Pkg2Decryptor::new_with_key(key as u64, DecrypterType::KMST1198);
        let mut decrypted = b"Hello, world!".to_vec();
        decryptor.decrypt_slice(&mut decrypted);
        decryptor.decrypt_slice(&mut decrypted);
        assert_eq!(decrypted, b"Hello, world!");

        let mut decrypted2 = "你好世界".bytes().collect::<Vec<_>>();

        decryptor.decrypt_slice(&mut decrypted2);
        decryptor.decrypt_slice(&mut decrypted2);
        assert_eq!(decrypted2, "你好世界".bytes().collect::<Vec<_>>());
    }

    #[test]
    fn test_pkg2_decryptor_u64() {
        let key = 0xDEADBEEFDEADBEEF;
        let decryptor = Pkg2Decryptor::new_with_key(key, DecrypterType::KMST1198);
        let mut decrypted = b"Hello, world!".to_vec();
        decryptor.decrypt_slice(&mut decrypted);
        decryptor.decrypt_slice(&mut decrypted);
        assert_eq!(decrypted, b"Hello, world!");
    }

    #[test]
    fn test_pkg2_keys_len16() {
        assert_eq!(
            gen_pkg2_keys_len16(0x0807_0605_0403_0201),
            [1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 0]
        );
    }
}
