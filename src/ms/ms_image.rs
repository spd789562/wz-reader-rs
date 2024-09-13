use crate::{WzImage, WzReader};
use std::cmp;
use std::sync::Arc;

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
        // calc snow key for entry
        let mut key_hash = 0x811C9DC5;
        for b in self.meta.key_salt.chars() {
            key_hash = (key_hash ^ b as u32).wrapping_mul(0x1000193);
        }

        // extract each  digit from key_hash
        let key_hash_digits: Vec<u8> = key_hash.to_string().chars().map(|c| c as u8 - 48).collect();

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
}
