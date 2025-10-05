#![allow(dead_code)]
use crate::reader::{self, Reader, WzReader};
use scroll::{Pread, LE};

use super::chacha20_reader::{
    ChaCha20Reader, MS_CHACHA20_KEY_BASE, MS_CHACHA20_KEY_SIZE, MS_CHACHA20_NONCE_SIZE,
    MS_CHACHA20_VERSION,
};
use super::snow2_decryptor::Snow2Decryptor;
use super::snow2_reader::{MS_SNOW2_KEY_SIZE, MS_SNOW2_VERSION};
use super::utils::{get_ascii_file_name, sum_str};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error reading binary")]
    ReaderError(#[from] reader::Error),
    #[error("Error reading binary: {0}")]
    ReadError(#[from] scroll::Error),
    #[error("Unsupported snow version, expected 2 but got {0}")]
    UnsupportedSnowVersion(u8),
    #[error("Unsupported chacha20 version, expected 4 but got {0}")]
    UnsupportedChacha20Version(u8),
    #[error("Hash mismatch, expected {0} but got {1}")]
    HashMismatch(i32, i32),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MsHeader {
    // name: String,
    pub key_salt: String,
    pub name_with_salt: String,
    pub header_hash: i32,
    // snow version, it should only be 2 currently
    pub version: u8,
    pub entry_count: i32,

    // header start
    pub hstart: usize,
    // entry start
    pub estart: usize,

    pub ms_file_version: u8,
}

impl MsHeader {
    pub fn from_ms_file<P>(path: P, reader: &WzReader) -> Result<Self, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file_name = get_ascii_file_name(path);

        let mut offset = 0;

        // all the code is from https://github.com/Kagamia/WzComparerR2/pull/271/files#diff-d0d53b2411f7d680fb0c7c32bbf10138be0f7e662555cbc28d27353fbd2741d0
        // 1. random bytes
        let rand_byte_count = sum_str(&file_name) % 312 + 30;
        let rand_bytes = reader.get_slice(offset..rand_byte_count);
        offset += rand_byte_count;

        // 2. encrypted snowKeySalt
        let hashed_salt_len = reader.read_u8_at(offset)?;
        let hashed_salt_len_i32 = reader.read_i32_at(offset)?;
        // plus 1 and skip 3 bytes
        offset += 4;
        let salt_len = hashed_salt_len ^ rand_bytes[0];
        let salt_byte_len = (salt_len * 2) as usize;
        let salt_bytes = reader.get_slice(offset..offset + salt_byte_len);
        offset += salt_byte_len;

        let salt_string = (0..salt_len as usize)
            .map(|i| {
                let byte = rand_bytes[i];
                let byte2 = salt_bytes[i * 2];
                (byte ^ byte2) as char
            })
            .collect::<String>();
        // 3. decrypt 9 bytes header with snow cipher
        // generate snow key based on filename+keySalt
        let file_name_with_salt = format!("{}{}", file_name, &salt_string);

        let file_name_with_salt_bytes = file_name_with_salt.as_bytes();
        let snow_key: [u8; MS_SNOW2_KEY_SIZE] = core::array::from_fn(|i| {
            file_name_with_salt_bytes[i % file_name_with_salt.len()] + i as u8
        });
        let mut snow_decryptor = Snow2Decryptor::new(snow_key);

        let hstart = offset;
        // the snow decryptor is decrypting 4 bytes at a time, so we need to decrypt 12 bytes for only 9 bytes of data
        let header_bytes = reader.get_slice(offset..offset + 12);

        let mut decrypt_data = [0_u8; 12];
        decrypt_data.copy_from_slice(&header_bytes);
        snow_decryptor.decrypt_slice(&mut decrypt_data);

        let hash = decrypt_data.pread_with::<i32>(0, LE)?;
        let version = decrypt_data[4];
        let entry_count = decrypt_data.pread_with::<i32>(5, LE)?;

        // verify  snowversion and hash
        const EXPECTED_VERSION: u8 = MS_SNOW2_VERSION;
        if version != EXPECTED_VERSION {
            return Err(Error::UnsupportedSnowVersion(version));
        }

        let sum_of_salt_byte = salt_bytes
            .chunks_exact(2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]))
            .fold(0, |acc: i32, x| acc.wrapping_add(x as i32));

        let actual_hash = hashed_salt_len_i32 + version as i32 + entry_count + sum_of_salt_byte;
        if hash != actual_hash {
            return Err(Error::HashMismatch(hash, actual_hash));
        }

        // 4. skip random bytes
        let estart = hstart + 9 + sum_str(&file_name) * 3 % 212 + 33;

        Ok(MsHeader {
            key_salt: salt_string,
            name_with_salt: file_name_with_salt,
            header_hash: hash,
            version,
            entry_count,
            hstart,
            estart,
            ms_file_version: 1,
        })
    }

    pub fn from_ms_file_v2<P>(path: P, reader: &WzReader) -> Result<Self, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file_name = get_ascii_file_name(path);

        let mut offset = 0;

        // all the code is from https://github.com/Kagamia/WzComparerR2/pull/347/files#diff-c14e6750eb85e2b36e81f13448c1afe29e229b58f639b6a373b5cc72352ba07b
        // 1. random bytes
        let rand_byte_count = sum_str(&file_name) % 312 + 30;
        let rand_bytes = reader.get_slice(offset..rand_byte_count);
        offset += rand_byte_count;

        // 2. check file version
        const EXPECTED_VERSION: u8 = MS_CHACHA20_VERSION;
        let version = reader.read_u8_at(offset)?;
        if version != EXPECTED_VERSION {
            return Err(Error::UnsupportedChacha20Version(version));
        }
        offset += 1;

        // 3. encrypted chacha20 key
        let hashed_salt_len = reader.read_u8_at(offset)?;
        let hashed_salt_len_i32 = reader.read_i32_at(offset)?;
        offset += 4;
        let salt_len = (hashed_salt_len ^ rand_bytes[0]) as usize;
        let salt_byte_len = (salt_len * 2) as usize;
        let mut salt_bytes = reader.get_slice(offset..offset + salt_byte_len).to_vec();
        offset += salt_byte_len;

        for i in 0..salt_len {
            let a = rand_bytes[i] ^ salt_bytes[i * 2];
            salt_bytes[i] = ((a | 0x4B) << 1) - a - 75;
        }
        let salt_string = (0..salt_len as usize)
            .map(|i| {
                let a = rand_bytes[i] ^ salt_bytes[i * 2];
                let b = ((a | 0x4B) << 1) - a - 75;
                b as char
            })
            .collect::<String>();

        let hstart = offset;

        // 4. decrypt 8 bytes header with chacha20 cipher
        // generate chacha20 key based on filename+keySalt
        let file_name_with_salt = format!("{}{}", file_name, &salt_string);
        let file_name_with_salt_bytes = file_name_with_salt.as_bytes();
        let chacha20_key: [u8; MS_CHACHA20_KEY_SIZE] = core::array::from_fn(|i| {
            (file_name_with_salt_bytes[i % file_name_with_salt.len()] + i as u8)
                ^ MS_CHACHA20_KEY_BASE[i]
        });
        let empty_nonce = [0; MS_CHACHA20_NONCE_SIZE];

        let mut chacha20_reader = ChaCha20Reader::new(
            reader.get_slice(offset..offset + 8),
            &chacha20_key,
            &empty_nonce,
        );
        let hash = chacha20_reader.read_i32()?;
        let entry_count = chacha20_reader.read_i32()?;

        // hash checking?

        // 5. skip random bytes
        let estart = hstart + 8 + sum_str(&file_name) * 3 % 212 + 64;

        Ok(MsHeader {
            key_salt: salt_string,
            name_with_salt: file_name_with_salt,
            header_hash: hash,
            version,
            entry_count,
            hstart,
            estart,
            ms_file_version: 2,
        })
    }
}
