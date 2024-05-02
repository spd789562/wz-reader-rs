use std::ops::Range;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use crate::reader::{read_i32_at, WzReader};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WzSoundError {
    #[error("Unsupported sound format")]
    UnsupportedFormat,

    #[error(transparent)]
    SaveError(#[from] std::io::Error),

    #[error("Not a Sound property")]
    NotSoundProperty,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub enum WzSoundType {
    Mp3,
    Wav,
    #[default]
    Binary,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct WzSound {
    #[cfg_attr(feature = "serde", serde(skip))]
    reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    length: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    header_offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    header_size: usize,
    pub duration: u32,
    pub sound_type: WzSoundType,
}

const WAV_HEADER: [u8; 44] = [
    0x52,0x49,0x46,0x46, //"RIFF"
    0,0,0,0, //ChunkSize
    0x57,0x41,0x56,0x45, //"WAVE"

    0x66,0x6d,0x74,0x20, //"fmt "
    0x10,0,0,0, //chunk1Size
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // copy16char

    0x64,0x61,0x74,0x61, //"data"
    0,0,0,0 //chunk2Size
];

fn get_frequency_header(header: &[u8]) -> u32 {
    if header.len() <= 0x3c {
        0
    } else {
        read_i32_at(header, 0x38).unwrap() as u32
    }
}

pub fn get_sound_type_from_header(header: &[u8], file_size: u32, duration: u32) -> WzSoundType {
    let frequency = get_frequency_header(header);
    if header.len() == 0x46 {
        if frequency == file_size && duration == 1000 {
            WzSoundType::Binary
        } else {
            WzSoundType::Wav
        }
    } else {
        WzSoundType::Mp3
    }
}

impl WzSound {
    pub fn new(reader: &Arc<WzReader>, offset: usize, length: u32, header_offset: usize, header_size: usize, duration: u32, sound_type: WzSoundType) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset,
            length,
            header_offset,
            header_size,
            duration,
            sound_type,
        }
    }
    fn get_buffer_range(&self) -> Range<usize> {
        self.offset..self.offset + self.length as usize
    }
    fn get_header_range(&self) -> Range<usize> {
        self.header_offset..self.header_offset + self.header_size
    }
    pub fn get_wav_header(&self) -> Vec<u8> {
        let header = self.reader.get_slice(self.get_header_range());
        let chunk_size = (self.length + 36).to_le_bytes();
        let u8_16_from_header = &header[0x34..0x34+16];
        let chunk2_size = self.length.to_le_bytes();

        let mut wav_header = WAV_HEADER.to_vec();

        wav_header[4..8].copy_from_slice(&chunk_size);
        wav_header[20..36].copy_from_slice(u8_16_from_header);
        wav_header[40..44].copy_from_slice(&chunk2_size);

        wav_header
    }
    pub fn get_buffer(&self) -> Vec<u8> {
        let buffer = self.reader.get_slice(self.get_buffer_range());
        match self.sound_type {
            WzSoundType::Wav => {
                let mut wav_buffer = Vec::with_capacity(44 + buffer.len());
                wav_buffer.extend_from_slice(&self.get_wav_header());
                wav_buffer.extend_from_slice(buffer);
                wav_buffer
            },
            _ => {
                buffer.to_vec()
            }
        }
    }
    pub fn extract_sound(&self, path: PathBuf) -> Result<(), WzSoundError> {
        let buffer = self.reader.get_slice(self.get_buffer_range());

        match self.sound_type {
            WzSoundType::Wav => {
                let mut file = File::create(path.with_extension("wav"))?;
                let wav_header = self.get_wav_header();
                file.write_all(&wav_header)?;
                file.write_all(buffer)?;
            },
            WzSoundType::Mp3 => {
                let mut file = File::create(path.with_extension("mp3"))?;
                file.write_all(buffer)?;
            },
            _ => {
            let mut file = File::create(path)?;
                file.write_all(buffer)?;
            }
        }

        Ok(())
    }
}