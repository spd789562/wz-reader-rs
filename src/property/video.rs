use crate::{reader, WzReader};
use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct WzVideo {
    pub reader: Arc<WzReader>,
    offset: usize,
    length: usize,
}

impl WzVideo {
    pub fn new(reader: &Arc<WzReader>, offset: usize, length: usize) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset,
            length,
        }
    }

    pub fn get_header(&self) -> Result<VideoHeader, VideoHeaderError> {
        let reader = self.reader.create_slice_reader();
        let mut signature = String::with_capacity(4);
        for _ in 0..4 {
            signature.push(reader.read_u8()? as char);
        }
        if signature != "MCV0" {
            return Err(VideoHeaderError::MissingSignature);
        }
        reader.skip(2);
        let header_len = reader.read_u16()? as usize;
        let four_cc = reader.read_u32()? ^ 0xa5a5a5a5;
        let width = reader.read_u16()? as usize;
        let height = reader.read_u16()? as usize;
        let frame_count = reader.read_i32()? as usize;
        let data_flag = reader.read_u8()?;
        reader.skip(3);

        let frame_delay_unit = reader.read_i64()?;
        let default_delay = reader.read_i32()? as i64;
        reader.seek(self.offset + header_len as usize);

        let mut frames: Vec<FrameInfo> = Vec::with_capacity(frame_count);

        for _ in 0..frame_count {
            let mut frame = FrameInfo::default();
            frame.offset = reader.read_i32()? as usize;
            frame.length = reader.read_i32()? as usize;
            frames.push(frame);
        }

        // alpha map
        if data_flag & DataFlag::AlphaMap as u8 != 0 {
            for frame in &mut frames {
                let alpha_offset = reader.read_i32()?;
                if alpha_offset == -1 {
                    frame.alpha_offset = usize::MAX;
                } else {
                    frame.alpha_offset = alpha_offset as usize;
                }
                frame.alpha_length = reader.read_i32()? as usize;
            }
        }

        // frame delay
        if data_flag & DataFlag::PerFrameDelay as u8 != 0 {
            for frame in &mut frames {
                frame.delay_ns = reader.read_i64()? * frame_delay_unit;
            }
        } else {
            for frame in &mut frames {
                frame.delay_ns = default_delay * frame_delay_unit;
            }
        }

        // frame timeline
        if data_flag & DataFlag::PerFrameTimeline as u8 != 0 {
            for frame in &mut frames {
                frame.start_time_ns = reader.read_i64()? * frame_delay_unit;
            }
        } else {
            let mut start_time = 0;
            for frame in &mut frames {
                frame.start_time_ns = start_time;
                start_time += frame.delay_ns;
            }
        }

        let data_offset = reader.pos.get() - self.offset;
        for frame in &mut frames {
            frame.offset += data_offset;
            if frame.alpha_offset != usize::MAX && frame.alpha_length > 0 {
                frame.alpha_offset += data_offset;
            }
        }

        Ok(VideoHeader {
            signature,
            header_len,
            four_cc: four_cc.to_le_bytes(),
            width,
            height,
            frame_count,
            data_flag,
            frames,
        })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum DataFlag {
    Default = 0,
    AlphaMap = 1,
    PerFrameDelay = 2,
    PerFrameTimeline = 4,
}

#[derive(Error, Debug)]
pub enum VideoHeaderError {
    #[error("Error parsing WzString: {0}")]
    ParseError(#[from] reader::Error),
    #[error("Missing signature")]
    MissingSignature,
    #[error("Invalid video header")]
    InvalidHeader,
}

#[derive(Debug, Default)]
pub struct VideoHeader {
    signature: String,
    header_len: usize,
    four_cc: [u8; 4],
    width: usize,
    height: usize,
    frame_count: usize,
    data_flag: u8,
    frames: Vec<FrameInfo>,
}

#[derive(Debug, Default)]
pub struct FrameInfo {
    offset: usize,
    length: usize,
    alpha_offset: usize,
    alpha_length: usize,
    delay_ns: i64,
    start_time_ns: i64,
}
