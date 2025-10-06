use crate::{WzImage, WzReader};
use c2_chacha::stream_cipher::{NewStreamCipher, SyncStreamCipher};
use c2_chacha::Ietf;
use std::cmp;
use std::sync::Arc;

use super::chacha20_reader::{MS_CHACHA20_KEY_BASE, MS_CHACHA20_KEY_SIZE, MS_CHACHA20_NONCE_SIZE};
use super::snow2_decryptor::Snow2Decryptor;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default)]
pub struct MsEntryMeta {
    pub key_salt: String,
    pub entry_name: String,
    pub check_sum: i32,
    pub flags: i32,
    pub start_pos: i32,
    pub size: i32,
    pub size_aligned: i32,
    pub unk1: i32,
    pub unk2: i32,
    pub entry_key: [u8; 16],
    pub unk3: i32, // for ms file v2
    pub unk4: i32, // for ms file v2
    pub version: u8,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Debug, Clone, Default)]
pub struct MsImage {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub meta: MsEntryMeta,
}

impl MsImage {
    pub fn new(meta: MsEntryMeta, reader: &Arc<WzReader>) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset: meta.start_pos as usize,
            block_size: meta.size_aligned as usize,
            meta,
        }
    }

    /// make a WzImage from the MsImage, this process will allocate a new buffer instead of using MsFile's buffer
    pub fn to_wz_image(&self) -> WzImage {
        match self.meta.version {
            1 => self.to_wz_image_v1(),
            2 => self.to_wz_image_v2(),
            _ => panic!("Invalid MsImage version: {}", self.meta.version),
        }
    }

    fn to_wz_image_v1(&self) -> WzImage {
        // calc snow key for entry
        let mut key_hash = 0x811C9DC5;
        for b in self.meta.key_salt.chars() {
            key_hash = (key_hash ^ b as u32).wrapping_mul(0x1000193);
        }

        // extract each  digit from key_hash
        let key_hash_digits: Vec<u8> = key_hash
            .to_string()
            .chars()
            .map(|c| c as u8 - b'0')
            .collect();

        let mut img_key = [0_u8; 16];

        for i in 0..16 {
            let char = self
                .meta
                .entry_name
                .chars()
                .nth(i % self.meta.entry_name.len())
                .unwrap() as u8;
            let digit = key_hash_digits[i % key_hash_digits.len()] % 2;
            let ekey = self.meta.entry_key[((key_hash_digits[(i + 2) % key_hash_digits.len()]
                + i as u8)
                % self.meta.entry_key.len() as u8)
                as usize];
            let digit2 = (key_hash_digits[(i + 1) % key_hash_digits.len()] + i as u8) % 5;

            img_key[i] = (i as u8)
                .wrapping_add(char.wrapping_mul(digit.wrapping_add(ekey).wrapping_add(digit2)));
        }

        let mut image_buffer = self
            .reader
            .get_slice(self.offset..self.offset + self.block_size)
            .to_owned();

        // decrypt initial 1024 bytes twice
        {
            let mut snow_decryptor = Snow2Decryptor::new(img_key);
            let min_len = cmp::min(1024, image_buffer.len());
            snow_decryptor.decrypt_slice(&mut image_buffer[..min_len]);
        }

        let mut snow_decryptor = Snow2Decryptor::new(img_key);

        snow_decryptor.decrypt_slice(&mut image_buffer[..]);

        // create a new allocate reader,but only grab the size of the image, not size_aligned
        let image_reader = WzReader::from_buff(&image_buffer[..self.meta.size as usize]);

        WzImage {
            reader: Arc::new(image_reader),
            name: self.meta.entry_name.clone().into(),
            offset: 0,
            block_size: self.meta.size as usize,
            is_parsed: false,
        }
    }

    fn to_wz_image_v2(&self) -> WzImage {
        // calc chacha20 key for entry
        let mut key_hash: u32 = 0x811C9DC5;

        for b in self.meta.key_salt.chars() {
            key_hash = (key_hash ^ b as u32).wrapping_mul(0x1000193);
        }
        let key_hash2 = key_hash >> 1;
        let key_hash3 = key_hash2 ^ 0x6C;
        let _key_hash4 = key_hash3 << 2; // not used
        let key_hash_digits: Vec<u8> = key_hash
            .to_string()
            .chars()
            .map(|c| c as u8 - b'0')
            .collect();

        // key
        let mut img_key = [0_u8; MS_CHACHA20_KEY_SIZE];
        for i in 0..MS_CHACHA20_KEY_SIZE {
            let char = self
                .meta
                .entry_name
                .chars()
                .nth(i % self.meta.entry_name.len())
                .unwrap() as u8;
            let digit = key_hash_digits[i % key_hash_digits.len()] % 2;

            let ekey_idx = ((key_hash_digits[(i + 2) % key_hash_digits.len()] + i as u8)
                % self.meta.entry_key.len() as u8) as usize;
            let ekey = self.meta.entry_key[ekey_idx];

            let digit2 = (key_hash_digits[(i + 1) % key_hash_digits.len()] + i as u8) % 5;

            img_key[i] = (i as u8)
                .wrapping_add(char.wrapping_mul(digit.wrapping_add(ekey).wrapping_add(digit2)));
            img_key[i] ^= MS_CHACHA20_KEY_BASE[i];
        }

        // nonce and counter
        let mut key_hash_data = [0_u8; 12];
        key_hash_data[0..4].copy_from_slice(&key_hash.to_le_bytes());
        key_hash_data[4..8].copy_from_slice(&key_hash2.to_le_bytes());
        key_hash_data[8..12].copy_from_slice(&key_hash3.to_le_bytes());

        let (mut a, mut b, mut c, mut d) = (0_i32, 0_i32, 90_i32, 0_i32);
        for i in 0..12 {
            key_hash_data[i as usize] ^=
                (d + (11 * (i as u32 / 11) as i32) + (c ^ (i as u32 >> 2) as i32) + (a ^ b)) as u8;
            d = d.wrapping_sub(1);
            a = a.wrapping_add(8);
            b = b.wrapping_add(17);
            c = c.wrapping_add(43);
        }
        let mut nonce = [0_u8; MS_CHACHA20_NONCE_SIZE];
        nonce[4..].copy_from_slice(&key_hash_data[0..8]);
        let counter = u32::from_le_bytes(key_hash_data[8..12].try_into().unwrap());

        let mut image_buffer = self
            .reader
            .get_slice(self.offset..self.offset + self.block_size)
            .to_owned();

        // only decrypt initial 1024 bytes
        {
            let mut chacha20_cipher = Ietf::new_var(&img_key, &nonce).unwrap();
            chacha20_cipher
                .state
                .state
                .set_stream_param(0, counter as u64);
            let min_len = cmp::min(1024, image_buffer.len());
            chacha20_cipher.apply_keystream(&mut image_buffer[..min_len]);
        }

        // create a new allocate reader,but only grab the size of the image, not size_aligned
        let image_reader = WzReader::from_buff(&image_buffer[..self.meta.size as usize]);

        WzImage {
            reader: Arc::new(image_reader),
            name: self.meta.entry_name.clone().into(),
            offset: 0,
            block_size: self.meta.size as usize,
            is_parsed: false,
        }
    }
}
