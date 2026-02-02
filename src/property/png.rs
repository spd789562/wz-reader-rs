use crate::property::string::resolve_string_from_node;
use crate::util::color::{SimpleColor, SimpleColorAlpha};
use crate::{
    property::WzSubProperty,
    reader::{self, Reader},
    util::node_util,
    WzNodeArc, WzObjectType,
};
// use block_compression::BC7Settings;
// use block_compression::{decode::decompress_blocks_as_rgba8, CompressionVariant};
use flate2::{Decompress, FlushDecompress};
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use std::sync::Arc;
use texture2ddecoder::decode_bc7;
use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Error)]
pub enum WzPngParseError {
    #[error("inflate raw data failed")]
    InflateError(#[from] flate2::DecompressError),

    #[error("decode bc7 failed: {0}")]
    DecodeBc7Error(String),

    #[error("Unknown format: {0:?}")]
    UnknownFormat(WzPngFormat),

    #[error("Unsupported header: {0}")]
    UnsupportedHeader(i32),

    #[error("Error reading color: {0}")]
    ReadColorError(#[from] reader::Error),

    #[error(transparent)]
    SaveError(#[from] image::ImageError),

    #[error("Can't not resolve _inlink or _outlink")]
    LinkError,

    #[error("Not a PNG property")]
    NotPngProperty,
}

type ImageBufferRgbaChunk = ImageBuffer<Rgba<u8>, Vec<u8>>;
type ImageBufferRgba16Chunk = ImageBuffer<Rgba<u16>, Vec<u16>>;
type ImageBufferRgbChunk = ImageBuffer<Rgb<u8>, Vec<u8>>;

/// A helper get image from `WzNodeArc`, will also resolve `_inlink` or `_outlink`
pub fn get_image(node: &WzNodeArc) -> Result<DynamicImage, WzPngParseError> {
    let node_read = node.read().unwrap();
    match &node_read.object_type {
        WzObjectType::Property(WzSubProperty::PNG(png)) => {
            let inlink_target = node_read
                .at("_inlink")
                .and_then(|node| resolve_string_from_node(&node).ok())
                .and_then(|inlink| node_util::resolve_inlink(&inlink, node));

            if let Some(target) = inlink_target {
                return get_image(&target);
            }

            let outlink_target = node_read
                .at("_outlink")
                .and_then(|node| resolve_string_from_node(&node).ok())
                .and_then(|outlink| node_util::resolve_outlink(&outlink, node, true));

            if let Some(target) = outlink_target {
                return get_image(&target);
            }

            png.extract_png()
        }
        _ => Err(WzPngParseError::NotPngProperty),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum WzPngFormat {
    #[default]
    Unknown,
    BGRA4444 = 1,
    BGRA8888 = 2,
    ARGB1555 = 257,
    RGB565 = 513,
    RGB565_517 = 517,

    DXT1 = 4097, // bc1
    DXT3 = 1026, // bc3
    DXT3SM = 3,
    DXT5 = 2050, // bc5
    BC7 = 4098,

    R16 = 769,
    A8 = 2304,
    RGBA1010102 = 2562,
    RGBA32Float = 4100,
}

impl From<u32> for WzPngFormat {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::BGRA4444,
            2 => Self::BGRA8888,
            257 => Self::ARGB1555,
            513 => Self::RGB565,
            517 => Self::RGB565_517,
            4097 => Self::DXT1,
            1026 => Self::DXT3,
            3 => Self::DXT3SM,
            2050 => Self::DXT5,
            4098 => Self::BC7,
            769 => Self::R16,
            2304 => Self::A8,
            2562 => Self::RGBA1010102,
            4100 => Self::RGBA32Float,
            _ => Self::Unknown,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct WzPng {
    #[cfg_attr(feature = "serde", serde(skip))]
    reader: Arc<reader::WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    offset: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    format1: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    format2: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub header: i32,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub pages: i32,

    pub width: u32,
    pub height: u32,
}

impl WzPng {
    pub fn new(
        reader: &Arc<reader::WzReader>,
        size: (u32, u32),
        format: (u32, u32),
        data_range: (usize, usize),
        header: i32,
        pages: i32,
    ) -> WzPng {
        WzPng {
            reader: Arc::clone(reader),
            offset: data_range.0,
            block_size: data_range.1,
            width: size.0,
            height: size.1,
            format1: format.0,
            format2: format.1,
            header,
            pages,
        }
    }
    #[inline]
    pub fn format(&self) -> WzPngFormat {
        WzPngFormat::from(self.format1 + self.format2)
    }
    #[inline]
    fn has_zlib_header(&self) -> bool {
        self.header == 0x9C78
            || self.header == 0xDA78
            || self.header == 0x0178
            || self.header == 0x5E78
    }
    pub fn extract_png(&self) -> Result<DynamicImage, WzPngParseError> {
        let data = self
            .reader
            .get_slice(self.offset..(self.offset + self.block_size));
        /* decompress */
        let pixels = self.get_raw_data(data)?;

        match self.format() {
            WzPngFormat::BGRA4444 => get_image_from_bgra4444(pixels, self.width, self.height),
            WzPngFormat::BGRA8888 => get_image_from_bgra8888(pixels, self.width, self.height),
            WzPngFormat::DXT3 | WzPngFormat::DXT3SM => {
                get_image_from_dxt3(&pixels, self.width, self.height)
            }
            WzPngFormat::ARGB1555 => get_image_from_argb1555(&pixels, self.width, self.height),
            WzPngFormat::RGB565 => get_image_from_rgb565(&pixels, self.width, self.height),
            WzPngFormat::RGB565_517 => {
                let decoded = get_pixel_data_form_517(&pixels, self.width, self.height);
                get_image_from_rgb565(&decoded, self.width, self.height)
            }
            WzPngFormat::DXT5 => get_image_from_dxt5(&pixels, self.width, self.height),
            WzPngFormat::R16 => get_image_from_a16(&pixels, self.width, self.height),
            WzPngFormat::RGBA1010102 => {
                get_image_from_r10g10b10a2(&pixels, self.width, self.height)
            }
            WzPngFormat::BC7 => get_image_from_bc7(&pixels, self.width, self.height),
            _ => Err(WzPngParseError::UnknownFormat(self.format())),
        }
    }
    fn get_buff_size(&self) -> Result<usize, WzPngParseError> {
        match self.format() {
            WzPngFormat::BGRA4444
            | WzPngFormat::ARGB1555
            | WzPngFormat::RGB565
            | WzPngFormat::R16 => Ok((self.width * self.height * 2) as usize),
            WzPngFormat::BGRA8888 | WzPngFormat::RGBA1010102 => {
                Ok((self.width * self.height * 4) as usize)
            }
            WzPngFormat::DXT3SM => Ok((self.width * self.height * 4) as usize),
            WzPngFormat::DXT3 | WzPngFormat::DXT5 => {
                Ok((((self.width + 3) / 4) * ((self.height + 3) / 4) * 16) as usize)
            }
            WzPngFormat::BC7 => Ok((self.width * (self.height & !3)) as usize),
            WzPngFormat::A8 => Ok((self.width * self.height) as usize),
            WzPngFormat::RGBA32Float => Ok((self.width * self.height * 16) as usize),
            WzPngFormat::RGB565_517 => {
                /* 128 = 16 * 16 / 2 */
                Ok((self.width * self.height / 128) as usize)
            }
            _ => Err(WzPngParseError::UnknownFormat(self.format())),
        }
    }
    fn get_raw_data(&self, data: &[u8]) -> Result<Vec<u8>, WzPngParseError> {
        let capacity = self.get_buff_size()?;

        if self.has_zlib_header() {
            inflate(true, data, capacity)
        } else {
            let mut keys = self.reader.keys.write().unwrap();

            let total_end = self.offset + self.block_size;

            let mut offset = self.offset;
            let mut end = 0;

            let mut decrypted = Vec::with_capacity(self.block_size);

            while offset < total_end {
                let block_size = self.reader.read_i32_at(offset).unwrap() as usize;
                offset += 4;

                let data = self.reader.get_slice(offset..(offset + block_size));
                offset += block_size;

                decrypted.extend_from_slice(data);

                keys.ensure_key_size(data.len()).unwrap();

                keys.decrypt_slice(&mut decrypted[end..(end + block_size)]);

                end += block_size;
            }

            /* the total chunk shoud start decryption at index 2 */
            inflate(false, &decrypted[2..], capacity)
        }
    }
}

#[inline]
fn inflate(with_header: bool, data: &[u8], capacity: usize) -> Result<Vec<u8>, WzPngParseError> {
    let mut deflater = Decompress::new(with_header);
    let mut result = Vec::with_capacity(capacity);

    if let Err(e) = deflater.decompress_vec(data, &mut result, FlushDecompress::Sync) {
        return Err(WzPngParseError::from(e));
    };

    Ok(result)
}

#[inline]
fn get_image_from_bgra4444(
    raw_data: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    #[cfg(feature = "rayon")]
    let imgbuffer = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 2;
        let pixel = raw_data[i];

        let b = pixel & 0x0F;
        let b = b | (b << 4);

        let g = pixel & 0xF0;
        let g = g | (g >> 4);

        let pixel = raw_data[i + 1];

        let r = pixel & 0x0F;
        let r = r | (r << 4);

        let a = pixel & 0xF0;
        let a = a | (a >> 4);

        image::Rgba([r, g, b, a])
    });
    #[cfg(not(feature = "rayon"))]
    let imgbuffer = image::ImageBuffer::from_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 2;
        let pixel = raw_data[i];

        let b = pixel & 0x0F;
        let b = b | (b << 4);

        let g = pixel & 0xF0;
        let g = g | (g >> 4);

        let pixel = raw_data[i + 1];

        let r = pixel & 0x0F;
        let r = r | (r << 4);

        let a = pixel & 0xF0;
        let a = a | (a >> 4);

        image::Rgba([r, g, b, a])
    });

    Ok(imgbuffer.into())
}

#[cfg(feature = "rayon")]
fn get_image_from_dxt3(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let image_buffer_chunks = raw_data
        .par_chunks(16)
        .try_fold_with::<_, Vec<ImageBufferRgbaChunk>, Result<_, WzPngParseError>>(
            Vec::new(),
            |mut v, chunk| {
                let alpha_table = create_alpha_table_dxt3(&chunk[0..8]);

                let u0: u16 = reader::read_u16_at(chunk, 8)?;
                let u1: u16 = reader::read_u16_at(chunk, 10)?;

                let color_table = create_color_table(u0, u1);
                let color_idx_table = create_color_index_table(&chunk[12..]);

                let mut img_buffer = image::ImageBuffer::new(4, 4);
                for y in 0..4 {
                    for x in 0..4 {
                        let idx = (y * 4 + x) as usize;
                        let color_idx = color_idx_table[idx] as usize;
                        let color = color_table[color_idx];
                        let alpha = alpha_table[idx];

                        img_buffer.put_pixel(
                            x,
                            y,
                            image::Rgba([color.r(), color.g(), color.b(), alpha]),
                        );
                    }
                }
                v.push(img_buffer);
                Ok(v)
            },
        )
        .try_reduce(Vec::new, |mut acc, v| {
            acc.extend(v);
            Ok(acc)
        })?;

    // combine image buffer
    let grid_row_count = width / 4;
    let img_buffer = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        *image_buffer_chunks[(x / 4 + y / 4 * grid_row_count) as usize].get_pixel(x % 4, y % 4)
    });

    Ok(img_buffer.into())
}

#[cfg(not(feature = "rayon"))]
fn get_image_from_dxt3(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let mut img_buffer = image::ImageBuffer::new(width, height);

    let mut alpha_table = [0u8; 16];
    let mut color_table = [Rgb::black(); 4];
    let mut color_idx_table = [0u8; 16];

    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let offset = (y * width + x * 4) as usize;
            let chunk = &raw_data[offset..offset + 16];
            expand_alpha_table_dxt3(&mut alpha_table, &chunk[..8]);

            let u0: u16 = reader::read_u16_at(chunk, 8)?;
            let u1: u16 = reader::read_u16_at(chunk, 10)?;

            expand_color_table(&mut color_table, u0, u1);
            expand_color_index_table(&mut color_idx_table, &chunk[12..]);

            for j in 0..4 {
                for i in 0..4 {
                    let idx = (j * 4 + i) as usize;
                    let color_idx = color_idx_table[idx] as usize;
                    let color = color_table[color_idx];
                    let alpha = alpha_table[idx];

                    img_buffer.put_pixel(
                        x + i,
                        y + j,
                        image::Rgba([color.r(), color.g(), color.b(), alpha]),
                    );
                }
            }
        }
    }

    Ok(img_buffer.into())
}

#[cfg(feature = "rayon")]
fn get_image_from_dxt5(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let image_buffer_chunks = raw_data
        .par_chunks(16)
        .try_fold_with::<_, Vec<ImageBufferRgbaChunk>, Result<_, WzPngParseError>>(
            Vec::new(),
            |mut v, chunk| {
                let alpha_table = create_alpha_table_dxt5(chunk[0], chunk[1]);
                let alpha_idx_table = create_alpha_index_table_dxt5(&chunk[2..8]);

                let u0: u16 = reader::read_u16_at(chunk, 8)?;
                let u1: u16 = reader::read_u16_at(chunk, 10)?;

                let color_table = create_color_table(u0, u1);
                let color_idx_table = create_color_index_table(&chunk[12..]);

                let mut img_buffer = image::ImageBuffer::new(4, 4);
                for y in 0..4 {
                    for x in 0..4 {
                        let idx = (y * 4 + x) as usize;
                        let color_idx = color_idx_table[idx] as usize;
                        let color = color_table[color_idx];
                        let alpha_idx = alpha_idx_table[idx] as usize;
                        let alpha = alpha_table[alpha_idx];

                        img_buffer.put_pixel(
                            x,
                            y,
                            image::Rgba([color.r(), color.g(), color.b(), alpha]),
                        );
                    }
                }
                v.push(img_buffer);
                Ok(v)
            },
        )
        .try_reduce(Vec::new, |mut acc, v| {
            acc.extend(v);
            Ok(acc)
        })?;

    let grid_row_count = width / 4;
    let img_buffer = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        *image_buffer_chunks[(x / 4 + y / 4 * grid_row_count) as usize].get_pixel(x % 4, y % 4)
    });

    Ok(img_buffer.into())
}

#[cfg(not(feature = "rayon"))]
fn get_image_from_dxt5(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let mut img_buffer = image::ImageBuffer::new(width, height);

    let mut alpha_table = [0u8; 8];
    let mut alpha_idx_table = [0u8; 16];
    let mut color_table = [Rgb::black(); 4];
    let mut color_idx_table = [0u8; 16];

    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let offset = (y * width + x * 4) as usize;
            let chunk = &raw_data[offset..offset + 16];
            expand_alpha_table_dxt5(&mut alpha_table, chunk[0], chunk[1]);
            expand_alpha_index_table_dxt5(&mut alpha_idx_table, &chunk[2..8]);

            let u0: u16 = reader::read_u16_at(chunk, 8)?;
            let u1: u16 = reader::read_u16_at(chunk, 10)?;

            expand_color_table(&mut color_table, u0, u1);
            expand_color_index_table(&mut color_idx_table, &chunk[12..]);

            for j in 0..4 {
                for i in 0..4 {
                    let idx = (j * 4 + i) as usize;
                    let color_idx = color_idx_table[idx] as usize;
                    let color = color_table[color_idx];
                    let alpha_idx = alpha_idx_table[idx] as usize;
                    let alpha = alpha_table[alpha_idx];

                    img_buffer.put_pixel(
                        x + i,
                        y + j,
                        image::Rgba([color.r(), color.g(), color.b(), alpha]),
                    );
                }
            }
        }
    }

    Ok(img_buffer.into())
}

fn get_pixel_data_form_517(raw_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    /* pixels is 256 times of raw_data */
    let mut pixels: Vec<u8> = vec![0; (width * height * 2) as usize];
    let mut line_index: usize = 0;
    // 16 probably means rgb565, it store 2 bytes for each pixel.
    let j_steps = (height / 16) as usize;
    let i_steps = (width / 16) as usize;
    for j in 0..j_steps {
        let mut dst_idx = line_index;
        for i in 0..i_steps {
            let idx = (i + j * i_steps) * 2;

            for _ in 0..16 {
                pixels[dst_idx] = raw_data[idx];
                dst_idx += 1;
                pixels[dst_idx] = raw_data[idx + 1];
                dst_idx += 1;
            }
        }
        // dst_idx totally add (width / 16) * 2 * 16 => width * 2 here

        // copy data from previous loop to next 16 chunks
        // don't know why it start from 1, and plus 32(16*2) later.
        for _ in 1..16 {
            let copy_len = (width * 2) as usize;
            let source_range = line_index..(line_index + copy_len);
            // copy source_range to dst_idx..(dst_idx + copy_len), and dst_idx would add copy_len in this loop.
            for suroce_idx in source_range {
                pixels[dst_idx] = pixels[suroce_idx];
                dst_idx += 1;
            }
        }

        line_index += (width * 32) as usize;
    }

    pixels
}

#[inline]
fn create_color_table(c0: u16, c1: u16) -> [Rgb<u8>; 4] {
    let color1 = Rgb::<u8>::from_rgb565(c0);
    let color2 = Rgb::<u8>::from_rgb565(c1);
    let color3: Rgb<u8>;
    let color4: Rgb<u8>;

    let r = color1.r() as i32;
    let g = color1.g() as i32;
    let b = color1.b() as i32;

    let r1 = color2.r() as i32;
    let g1 = color2.g() as i32;
    let b1 = color2.b() as i32;

    if c0 > c1 {
        color3 = Rgb([
            ((r * 2 + r1 + 1) / 3) as u8,
            ((g * 2 + g1 + 1) / 3) as u8,
            ((b * 2 + b1 + 1) / 3) as u8,
        ]);
        color4 = Rgb([
            ((r + r1 * 2 + 1) / 3) as u8,
            ((g + g1 * 2 + 1) / 3) as u8,
            ((b + b1 * 2 + 1) / 3) as u8,
        ]);
    } else {
        color3 = Rgb([
            ((r + r1) / 2) as u8,
            ((g + g1) / 2) as u8,
            ((b + b1) / 2) as u8,
        ]);
        color4 = Rgb::<u8>::black();
    }

    [color1, color2, color3, color4]
}

#[inline]
#[allow(dead_code)]
fn expand_color_table(color_table: &mut [Rgb<u8>; 4], c0: u16, c1: u16) {
    color_table[0] = Rgb::from_rgb565(c0);
    color_table[1] = Rgb::from_rgb565(c1);

    let r = color_table[0].r() as i32;
    let g = color_table[0].g() as i32;
    let b = color_table[0].b() as i32;

    let r1 = color_table[1].r() as i32;
    let g1 = color_table[1].g() as i32;
    let b1 = color_table[1].b() as i32;

    if c0 > c1 {
        color_table[2][0] = ((r * 2 + r1 + 1) / 3) as u8;
        color_table[2][1] = ((g * 2 + g1 + 1) / 3) as u8;
        color_table[2][2] = ((b * 2 + b1 + 1) / 3) as u8;

        color_table[3][0] = ((r + r1 * 2 + 1) / 3) as u8;
        color_table[3][1] = ((g + g1 * 2 + 1) / 3) as u8;
        color_table[3][2] = ((b + b1 * 2 + 1) / 3) as u8;
    } else {
        color_table[2][0] = ((r + r1) / 2) as u8;
        color_table[2][1] = ((g + g1) / 2) as u8;
        color_table[2][2] = ((b + b1) / 2) as u8;
        // set to black
        color_table[3][0] = 0;
        color_table[3][1] = 0;
        color_table[3][2] = 0;
    }
}

#[inline]
fn create_color_index_table(raw_data: &[u8]) -> [u8; 16] {
    let mut color_index_table = [0u8; 16];

    expand_color_index_table(&mut color_index_table, raw_data);

    color_index_table
}

#[inline]
fn expand_color_index_table(color_index_table: &mut [u8; 16], raw_data: &[u8]) {
    // raw_data should be a [u8; 4];
    for i in 0..4 {
        let color = raw_data[i];
        color_index_table[i * 4] = color & 0x03;
        color_index_table[i * 4 + 1] = (color & 0x0C) >> 2;
        color_index_table[i * 4 + 2] = (color & 0x30) >> 4;
        color_index_table[i * 4 + 3] = (color & 0xC0) >> 6;
    }
}

#[inline]
fn create_alpha_table_dxt3(raw_data: &[u8]) -> [u8; 16] {
    let mut alpha_table = [0u8; 16];

    expand_alpha_table_dxt3(&mut alpha_table, raw_data);

    alpha_table
}

#[inline]
fn expand_alpha_table_dxt3(alpha_table: &mut [u8; 16], raw_data: &[u8]) {
    // raw_data should be a [u8; 8];
    for i in 0..8 {
        let alpha = raw_data[i];
        alpha_table[i * 2] = alpha & 0x0F;
        alpha_table[i * 2 + 1] = (alpha & 0xf0) >> 4;
    }
    for item in alpha_table.iter_mut().take(16) {
        *item = *item | (*item << 4);
    }
}

#[inline]
fn create_alpha_table_dxt5(a0: u8, a1: u8) -> [u8; 8] {
    let mut alpha_table = [0u8; 8];

    expand_alpha_table_dxt5(&mut alpha_table, a0, a1);

    alpha_table
}

#[inline]
fn expand_alpha_table_dxt5(alpha_table: &mut [u8; 8], a0: u8, a1: u8) {
    alpha_table[0] = a0;
    alpha_table[1] = a1;
    if a0 > a1 {
        for i in 2..8 {
            // ((8 - i) * a0 + (i - 1) * a1 + 3) / 7
            alpha_table[i as usize] = (((8 - i) * a0 as i32 + (i - 1) * a1 as i32 + 3) / 7) as u8;
        }
    } else {
        for i in 2..6 {
            // ((6 - i) * a0 + (i - 1) * a1 + 2) / 5
            alpha_table[i as usize] = (((6 - i) * a0 as i32 + (i - 1) * a1 as i32 + 2) / 5) as u8;
        }
        alpha_table[6] = 0;
        alpha_table[7] = 255;
    }
}

#[inline]
fn create_alpha_index_table_dxt5(raw_data: &[u8]) -> [u8; 16] {
    let mut alpha_index_table = [0u8; 16];

    expand_alpha_index_table_dxt5(&mut alpha_index_table, raw_data);

    alpha_index_table
}

#[inline]
fn expand_alpha_index_table_dxt5(alpha_index_table: &mut [u8; 16], raw_data: &[u8]) {
    // raw_data should be a [u8; 6];
    for i in 0..2 {
        let local_offset = i * 3;
        let flags = (raw_data[local_offset] as u32)
            | ((raw_data[local_offset + 1] as u32) << 8)
            | ((raw_data[local_offset + 2] as u32) << 16);
        for j in 0..8 {
            let mask = (7 << (3 * j)) as u32;
            alpha_index_table[(i * 8) + j] = ((flags & mask) >> (3 * j)) as u8;
        }
    }
}

#[inline]
fn get_image_from_bgra8888(
    raw_data: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    #[cfg(feature = "rayon")]
    let img_buffer = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 4;
        image::Rgba([
            raw_data[i + 2],
            raw_data[i + 1],
            raw_data[i],
            raw_data[i + 3],
        ])
    });
    #[cfg(not(feature = "rayon"))]
    let img_buffer = image::ImageBuffer::from_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 4;
        image::Rgba([
            raw_data[i + 2],
            raw_data[i + 1],
            raw_data[i],
            raw_data[i + 3],
        ])
    });

    Ok(img_buffer.into())
}

#[inline]
fn get_image_from_rgb565(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let mut img_buffer: ImageBufferRgbChunk = image::ImageBuffer::new(width, height);
    img_buffer
        .enumerate_pixels_mut()
        .try_for_each::<_, Result<(), WzPngParseError>>(|(x, y, pixel)| {
            let i = (x + y * width) as usize * 2;
            let color = reader::read_u16_at(raw_data, i)?;
            *pixel = Rgb::<u8>::from_rgb565(color);
            Ok(())
        })?;

    Ok(img_buffer.into())
}

#[inline]
fn get_image_from_argb1555(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let mut img_buffer: ImageBufferRgbaChunk = image::ImageBuffer::new(width, height);
    img_buffer
        .enumerate_pixels_mut()
        .try_for_each::<_, Result<(), WzPngParseError>>(|(x, y, pixel)| {
            let i = (x + y * width) as usize * 2;
            let color = reader::read_u16_at(raw_data, i)?;
            *pixel = Rgba::<u8>::from_argb1555(color);
            Ok(())
        })?;

    Ok(img_buffer.into())
}

#[inline]
fn get_image_from_a16(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    let mut img_buffer: ImageBufferRgba16Chunk = image::ImageBuffer::new(width, height);
    img_buffer
        .enumerate_pixels_mut()
        .try_for_each::<_, Result<(), WzPngParseError>>(|(x, y, pixel)| {
            let i = (x + y * width) as usize * 2;
            let pixel_16 = u16::from_le_bytes([raw_data[i], raw_data[i + 1]]);
            *pixel = Rgba([pixel_16, 0u16, 0u16, 0xffff]);
            Ok(())
        })?;

    Ok(img_buffer.into())
}

#[inline]
fn get_image_from_r10g10b10a2(
    raw_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    #[inline(always)]
    fn pixel_to_rgba(chunk: &[u8]) -> Rgba<u8> {
        let pixel = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let r = ((pixel & 0x3ff) >> 2) as u8;
        let g = (((pixel >> 10) & 0x3ff) >> 2) as u8;
        let b = (((pixel >> 20) & 0x3ff) >> 2) as u8;
        let a = (((pixel >> 30) & 0x03) * 85) as u8;
        Rgba([r, g, b, a])
    }

    #[cfg(feature = "rayon")]
    let img_buffer = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 4;
        pixel_to_rgba(&raw_data[i..i + 4])
    });
    #[cfg(not(feature = "rayon"))]
    let img_buffer = image::ImageBuffer::from_fn(width, height, |x, y| {
        let i = (x + y * width) as usize * 4;
        pixel_to_rgba(&raw_data[i..i + 4])
    });

    Ok(img_buffer.into())
}

fn get_image_from_bc7(
    block_data: &[u8],
    width: u32,
    height: u32,
) -> Result<DynamicImage, WzPngParseError> {
    // let mut output_rgba = vec![0_u8; (width * height * 4) as usize];
    // decompress_blocks_as_rgba8(
    //     CompressionVariant::BC7(BC7Settings::opaque_ultra_fast()),
    //     width,
    //     height,
    //     block_data,
    //     &mut output_rgba,
    // );
    let mut output_rgba_u32 = vec![0_u32; (width * height) as usize];

    /* TODO: maybe implement this without external crate */
    decode_bc7(
        block_data,
        width as usize,
        height as usize,
        &mut output_rgba_u32,
    )
    .map_err(|e| WzPngParseError::DecodeBc7Error(e.to_string()))?;

    let output_rgba: Vec<u8> = output_rgba_u32
        .into_iter()
        .flat_map(|pixel| {
            let bgra_bytes = pixel.to_le_bytes();
            [bgra_bytes[2], bgra_bytes[1], bgra_bytes[0], bgra_bytes[3]]
        })
        .collect();

    let img_buffer = ImageBufferRgbaChunk::from_raw(width, height, output_rgba)
        .ok_or(WzPngParseError::UnsupportedHeader(0))?;
    Ok(img_buffer.into())
}
