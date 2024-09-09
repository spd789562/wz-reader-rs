#![allow(dead_code)]
use crate::reader::{self, Reader, WzReader};
use scroll::{Pread, LE};

use super::snow2_decryptor::Snow2Decryptor;

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
    #[error("Hash mismatch, expected {0} but got {1}")]
    HashMismatch(i32, i32),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MsHeader {
    // name: String,
    key_salt: String,
    name_with_salt: String,
    header_hash: i32,
    // snow version, it should only be 2 currently
    version: u8,
    entry_count: i32,

    // header start
    hstart: usize,
    // entry start
    estart: usize,
}

impl MsHeader {
    pub fn from_ms_file<P>(path: P, reader: &WzReader) -> Result<Self, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file_name = path
            .as_ref()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_ascii_lowercase();

        let mut offset = 0;

        // all the code is from https://github.com/Kagamia/WzComparerR2/pull/271/files#diff-d0d53b2411f7d680fb0c7c32bbf10138be0f7e662555cbc28d27353fbd2741d0
        // 1. random bytes
        let rand_byte_count = file_name
            .as_bytes()
            .iter()
            .map(|&b| b as usize)
            .sum::<usize>()
            % 312
            + 30;
        let rand_bytes = reader.get_slice(offset..rand_byte_count);
        offset += rand_byte_count;

        // 2. encrypted snowKeySalt
        let hashed_salt_len = reader.read_u8_at(offset)?;
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
        let file_name_with_salt = format!("{}{}", file_name.to_string(), &salt_string);

        let file_name_with_salt_bytes = file_name_with_salt.as_bytes();
        let mut snow_key: [u8; 16] = [0; 16];
        for i in 0..16 {
            snow_key[i] = file_name_with_salt_bytes[i % file_name_with_salt.len()] + i as u8;
        }
        let mut snow_decryptor = Snow2Decryptor::new(snow_key.clone());

        let hstart = offset;
        let header_bytes = reader.get_slice(offset..offset + 12);

        // the snow decryptor is decrypting 4 bytes at a time, so we need to decrypt 12 bytes for only 9 bytes of data
        let decrypt_data = snow_decryptor.make_decrypt_slice(&header_bytes[..]);

        let hash = decrypt_data.pread_with::<i32>(0, LE)?;
        let version = decrypt_data[4];
        let entry_count = decrypt_data.pread_with::<i32>(5, LE)?;

        // verify  snowversion and hash
        const EXPECTED_VERSION: u8 = 2;
        if version != EXPECTED_VERSION {
            return Err(Error::UnsupportedSnowVersion(version));
        }

        let sum_of_salt_byte = salt_bytes
            .chunks_exact(2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]))
            .fold(0, |acc: i32, x| acc.wrapping_add(x as i32));

        let actual_hash = hashed_salt_len as i32 + version as i32 + entry_count + sum_of_salt_byte;
        if hash != actual_hash {
            return Err(Error::HashMismatch(hash, actual_hash));
        }

        // 4. skip random bytes
        let estart = hstart
            + 9
            + file_name
                .as_bytes()
                .iter()
                .map(|&b| b as usize * 3)
                .sum::<usize>()
                % 212
            + 33;

        Ok(MsHeader {
            key_salt: salt_string,
            name_with_salt: file_name_with_salt,
            header_hash: hash,
            version,
            entry_count,
            hstart,
            estart,
        })
    }
}
