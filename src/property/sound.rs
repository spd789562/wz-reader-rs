use crate::reader::read_i32_at;
use std::ops::Range;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum WzSoundType {
    Mp3,
    Wav,
    Binary,
}

#[derive(Debug, Clone)]
pub struct WzSoundMeta {
    pub offset: usize,
    pub length: u32,
    pub header_offset: usize,
    pub header_size: usize,
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

pub fn get_frequency_header(header: &[u8]) -> u32 {
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

impl WzSoundMeta {
    pub fn new(offset: usize, length: u32, header_offset: usize, header_size: usize, duration: u32, sound_type: WzSoundType) -> Self {
        Self {
            offset,
            length,
            header_offset,
            header_size,
            duration,
            sound_type,
        }
    }
    pub fn get_buffer_range(&self) -> Range<usize> {
        self.offset..self.offset + self.length as usize
    }
    pub fn get_header_range(&self) -> Range<usize> {
        self.header_offset..self.header_offset + self.header_size
    }
    pub fn get_wav_header(&self, data: &[u8]) -> Vec<u8> {
        let header = &data[self.get_header_range()];
        let chunk_size = (self.length + 36).to_le_bytes();
        let u8_16_from_header = &header[0x34..0x34+16];
        let chunk2_size = self.length.to_le_bytes();

        let mut wav_header = WAV_HEADER.to_vec();

        wav_header[4..8].copy_from_slice(&chunk_size);
        wav_header[20..36].copy_from_slice(u8_16_from_header);
        wav_header[40..44].copy_from_slice(&chunk2_size);

        wav_header
    }
    pub fn extract_sound(&self, data: &[u8], path: PathBuf) -> Result<(), String> {
        let buffer = &data[self.get_buffer_range()];

        match self.sound_type {
            WzSoundType::Wav => {
                let mut file = File::create(path.with_extension("wav")).unwrap();
                let wav_header = self.get_wav_header(data);
                file.write_all(&wav_header).unwrap();
                file.write_all(buffer).unwrap();
            },
            WzSoundType::Mp3 => {
                let mut file = File::create(path.with_extension("mp3")).unwrap();
                file.write_all(buffer).unwrap();
            },
            _ => {
            let mut file = File::create(path).unwrap();
                file.write_all(buffer).unwrap();
            }
        }

        Ok(())
    }
}