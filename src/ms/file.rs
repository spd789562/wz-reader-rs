use crate::{reader, WzNode, WzNodeArc, WzNodeArcVec, WzNodeName, WzReader};
use memmap2::Mmap;
use std::fs::File;
use std::sync::Arc;

use super::chacha20_reader::{
    ChaCha20Reader, MS_CHACHA20_KEY_BASE, MS_CHACHA20_KEY_SIZE, MS_CHACHA20_NONCE_SIZE,
};
use super::header::{self, MsHeader};
use super::ms_image::{MsEntryMeta, MsImage};
use super::snow2_reader::{Snow2Reader, MS_SNOW2_KEY_SIZE};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    FileError(#[from] std::io::Error),
    #[error("invald ms file")]
    InvalidMsFile,
    #[error("Failed, in this case the causes are undetermined.")]
    FailedUnknown,
    #[error("Binary reading error")]
    ReaderError(#[from] reader::Error),
    #[error(transparent)]
    HeaderReadError(#[from] header::Error),
    #[error("Unknown ms file, unable to parse")]
    UnknownMsFile,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MsFile {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: bool,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub header: MsHeader,
}

impl MsFile {
    pub fn from_file<P>(path: P) -> Result<MsFile, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file: File = File::open(&path)?;
        let map = unsafe { Mmap::map(&file)? };

        let block_size = map.len();

        let reader = WzReader::new(map);

        let ms_header = if let Ok(header) = MsHeader::from_ms_file(&path, &reader) {
            header
        } else if let Ok(header) = MsHeader::from_ms_file_v2(&path, &reader) {
            header
        } else {
            return Err(Error::UnknownMsFile);
        };

        Ok(MsFile {
            block_size,
            is_parsed: false,
            reader: Arc::new(reader),
            header: ms_header,
        })
    }
    pub fn parse(&mut self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        match self.header.ms_file_version {
            1 => self.parse_v1(parent),
            2 => self.parse_v2(parent),
            _ => Err(Error::UnknownMsFile),
        }
    }

    fn parse_v1(&mut self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        // decrypt with another snow key
        let file_name_with_salt_bytes = self.header.name_with_salt.as_bytes();
        let file_name_with_salt_len = file_name_with_salt_bytes.len();
        let mut snow_key: [u8; 16] = [0; MS_SNOW2_KEY_SIZE];
        for i in 0_u8..16_u8 {
            let byte = file_name_with_salt_bytes
                [file_name_with_salt_len - 1 - i as usize % file_name_with_salt_len];
            snow_key[i as usize] = i + (i % 3 + 2).wrapping_mul(byte);
        }

        /* maybe I need to turn this to a struct but it ok for now */
        let data = self.reader.get_slice(0..self.block_size);
        let mut snow_reader = Snow2Reader::new(data, snow_key);

        snow_reader.offset = self.header.estart;

        let mut ms_images = Vec::with_capacity(self.header.entry_count as usize);

        for _ in 0..self.header.entry_count {
            let entry_name_len = snow_reader.read_i32()?;
            let entry_name = snow_reader.read_utf16_string(entry_name_len as usize * 2)?;
            let check_sum = snow_reader.read_i32()?;
            let flags = snow_reader.read_i32()?;
            let start_pos = snow_reader.read_i32()?;
            let size = snow_reader.read_i32()?;
            let size_aligned = snow_reader.read_i32()?;
            let unk1 = snow_reader.read_i32()?;
            let unk2 = snow_reader.read_i32()?;
            let mut entry_key = [0_u8; 16];
            snow_reader.write_bytes_to(&mut entry_key, 16)?;

            let meta = MsEntryMeta {
                key_salt: self.header.key_salt.clone(),
                entry_name,
                check_sum,
                flags,
                start_pos,
                size,
                size_aligned,
                unk1,
                unk2,
                entry_key,
                unk3: 0,
                unk4: 0,
                version: self.header.version,
            };
            let image = MsImage::new(meta, &self.reader);

            ms_images.push(image);
        }

        let mut data_start = snow_reader.offset;
        // align to 1024 bytes
        if (data_start & 0x3FF) != 0 {
            data_start = data_start - (data_start & 0x3FF) + 0x400;
        }

        for image in ms_images.iter_mut() {
            let actual_start = data_start + image.offset * 1024;
            image.offset = actual_start;
            image.meta.start_pos = actual_start as i32;
        }

        return Ok(ms_images
            .drain(..)
            .map(|image| {
                let name = WzNodeName::from(image.meta.entry_name.clone());
                let node = WzNode::new(&name, image, Some(parent));
                (name, node.into_lock())
            })
            .collect());
    }

    fn parse_v2(&mut self, parent: &WzNodeArc) -> Result<WzNodeArcVec, Error> {
        if self.header.entry_count == 0 {
            return Ok(vec![]);
        }

        // decrypt with another chacha20 key
        let file_name_with_salt_bytes = self.header.name_with_salt.as_bytes();
        let file_name_with_salt_len = file_name_with_salt_bytes.len();
        let chacha20_key2: [u8; MS_CHACHA20_KEY_SIZE] = core::array::from_fn(|i| {
            let byte = file_name_with_salt_bytes
                [file_name_with_salt_len - 1 - i % file_name_with_salt_len];
            MS_CHACHA20_KEY_BASE[i] ^ ((i as u8) + (((i as u8) % 3 + 2).wrapping_mul(byte)))
        });
        let empty_nonce = [0; MS_CHACHA20_NONCE_SIZE];

        let mut chacha20_reader = ChaCha20Reader::new(
            self.reader.get_slice(0..self.block_size),
            &chacha20_key2,
            &empty_nonce,
        );
        chacha20_reader.offset = self.header.estart;

        let mut ms_images = Vec::<MsImage>::with_capacity(self.header.entry_count as usize);

        for _ in 0..self.header.entry_count {
            let entry_name_len = chacha20_reader.read_i32()?;
            let entry_name = chacha20_reader.read_utf16_string(entry_name_len as usize * 2)?;
            let check_sum = chacha20_reader.read_i32()?;
            let flags = chacha20_reader.read_i32()?;
            let start_pos = chacha20_reader.read_i32()?;
            let size = chacha20_reader.read_i32()?;
            let size_aligned = chacha20_reader.read_i32()?;
            let unk1 = chacha20_reader.read_i32()?;
            let unk2 = chacha20_reader.read_i32()?;
            let mut entry_key = [0_u8; 16];
            chacha20_reader.write_bytes_to(&mut entry_key, 16)?;
            let unk3 = chacha20_reader.read_i32()?;
            let unk4 = chacha20_reader.read_i32()?;

            let meta = MsEntryMeta {
                key_salt: self.header.key_salt.clone(),
                entry_name,
                check_sum,
                flags,
                start_pos,
                size,
                size_aligned,
                unk1,
                unk2,
                entry_key,
                unk3,
                unk4,
                version: self.header.ms_file_version,
            };

            let image = MsImage::new(meta, &self.reader);

            ms_images.push(image);
        }

        let mut data_start = chacha20_reader.offset;
        // align to 1024 bytes
        if (data_start & 0x3FF) != 0 {
            data_start = data_start - (data_start & 0x3FF) + 0x400;
        }

        for image in ms_images.iter_mut() {
            let actual_start = data_start + image.offset * 1024;
            image.offset = actual_start;
            image.meta.start_pos = actual_start as i32;
        }

        return Ok(ms_images
            .drain(..)
            .map(|image| {
                let name = WzNodeName::from(image.meta.entry_name.clone());
                let node = WzNode::new(&name, image, Some(parent));
                (name, node.into_lock())
            })
            .collect());
    }
}
