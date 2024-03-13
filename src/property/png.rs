use flate2::{Decompress, FlushDecompress};
use image::{GenericImage, DynamicImage, ImageError};
use thiserror::Error;
use crate::reader;

#[derive(Debug, Error)]
pub enum WzPngParseError {
    #[error("inflate raw data failed")]
    InflateError(#[from] flate2::DecompressError),

    #[error("Unknown format: {0}")]
    UnknownFormat(u32),

    #[error("Unsupported header: {0}")]
    UnsupportedHeader(i32),
    
    #[error(transparent)]
    ReadColorError(#[from] scroll::Error),

    #[error(transparent)]
    SaveError(#[from] ImageError),

    #[error("Can't not resolve _inlink or _outlink")]
    LinkError,

    #[error("Not a PNG property")]
    NotPngProperty,
}

#[derive(Debug, Clone)]
pub struct WzPng {
    pub width: u32,
    pub height: u32,
    pub format1: u32,
    pub format2: u32,
    pub header: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Color(u8, u8, u8, u8);

impl Color {
    pub fn white() -> Color {
        Color(255, 255, 255, 255)
    }
    pub fn black() -> Color {
        Color(0, 0, 0, 255)
    }
    pub fn transparent() -> Color {
        Color(0, 0, 0, 0)
    }
    pub fn from_rgb565(color: u16) -> Color {
        let r = ((color & 0xF800) >> 11) as u8;
        let g = ((color & 0x07E0) >> 5) as u8;
        let b = (color & 0x001F) as u8;
    
        Color(r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2, 255)
    }
    pub fn from_argb1555(color: u16) -> Color {
        let a = if (color & 0x8000) != 0 { 255 } else { 0 };
        let r = ((color & 0x7C00) >> 10) as u8;
        let g = ((color & 0x03E0) >> 5) as u8;
        let b = (color & 0x001F) as u8;
    
        Color(r << 3 | r >> 2, g << 3 | g >> 2, b << 3 | b >> 2, a)
    }
    pub fn r(&self) -> u8 {
        self.0
    }
    pub fn g(&self) -> u8 {
        self.1
    }
    pub fn b(&self) -> u8 {
        self.2
    }
    pub fn a(&self) -> u8 {
        self.3
    }
}

impl From<Color> for image::Rgba<u8> {
    fn from(color: Color) -> Self {
        image::Rgba([color.0, color.1, color.2, color.3])
    }
}

impl WzPng {
    pub fn new(width: u32, height: u32, format1: u32, format2: u32, header: i32) -> WzPng {
        WzPng {
            width,
            height,
            format1,
            format2,
            header
        }
    }
    pub fn format(&self) -> u32 {
        self.format1 + self.format2
    }
    pub fn list_wz_used(&self) -> bool {
        self.header != 0x9C78 && self.header != 0xDA78 && self.header != 0x0178 && self.header != 0x5E78
    }
    pub fn extract_png(&self, data: &[u8]) -> Result<DynamicImage, WzPngParseError> {
        /* decompress */
        let pixels = self.get_raw_data(data)?;

        match self.format() {
            1 => {
                get_image_from_bgra4444(pixels, self.width, self.height)
            },
            2 => {
                get_image_from_bgra8888(pixels, self.width, self.height)
            },
            3 | 1026 => {
                get_image_from_dxt3(&pixels, self.width, self.height)
            },
            257 => {
                get_image_from_argb1555(&pixels, self.width, self.height)
            },
            513 => {
                get_image_from_rgb565(&pixels, self.width, self.height)
            },
            517 => {
                let decoded = get_pixel_data_form_517(&pixels, self.width, self.height);
                get_image_from_rgb565(&decoded, self.width, self.height)
            },
            2050 => {
                get_image_from_dxt5(&pixels, self.width, self.height)
            }
            _ => {
                Err(WzPngParseError::UnknownFormat(self.format()))
            }
        }
    }
    fn get_raw_data(&self, data: &[u8]) -> Result<Vec<u8>, WzPngParseError> {
        if self.list_wz_used() {
            return Err(WzPngParseError::UnsupportedHeader(self.header));
        }

        match self.format() {
            1 | 257 | 513 => {
                let size = (self.width * self.height * 2) as usize;
                inflate(data, size)
            },
            2 => {
                let size = (self.width * self.height * 4) as usize;
                inflate(data, size)
            },
            3 => {
                let size = (self.width * self.height * 4) as usize;
                inflate(data, size)
            },
            1026 | 2050 => {
                let size = (self.width * self.height) as usize;
                inflate(data, size)
            },
            517 => {
                /* 128 = 16 * 16 / 2 */
                let size = (self.width * self.height / 128) as usize;
                inflate(data, size)
            },
            _ => {
                Err(WzPngParseError::UnknownFormat(self.format()))
            }
        }
    }

}

fn inflate(data: &[u8], capacity: usize) -> Result<Vec<u8>, WzPngParseError> {
    let mut deflater = Decompress::new(true);
    let mut result = Vec::with_capacity(capacity);
    
    if let Err(e) = deflater.decompress_vec(data, &mut result, FlushDecompress::Sync) {
        return Err(WzPngParseError::from(e));
    };

    Ok(result)
}

fn get_image_from_bgra4444(raw_data: Vec<u8>, width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);
    
    let mut x = 0;
    let mut y = 0;

    for i in (0..raw_data.len()).step_by(2) {
        /* split u8 to separate 2 color value */
        let bg_pixel = raw_data[i];

        let b = bg_pixel & 0x0F;
        let b = b | (b << 4);

        let g = bg_pixel & 0xF0;
        let g = g | (g >> 4);

        let ra_pixel = raw_data[i + 1];

        let r = ra_pixel & 0x0F;
        let r = r | (r << 4);

        let a = ra_pixel & 0xF0;
        let a = a | (a >> 4);

        img.put_pixel(x, y, image::Rgba([r, g, b, a]));

        x += 1;
        if x >= width {
            x = 0;
            y += 1;
        }
    }

    Ok(img)
}

fn get_image_from_dxt3(raw_data: &[u8], width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);

    let mut color_table = [Color::transparent(); 4];
    let mut color_idx_table = [0u8; 16];
    let mut alpha_table = [0u8; 16];
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let offset = (x * 4 + y * width) as usize;
            expand_alpha_table_dxt3(&mut alpha_table, raw_data, offset);
            let u0: u16 = reader::read_u16_at(raw_data, offset + 8)?;
            let u1: u16 = reader::read_u16_at(raw_data, offset + 10)?;
            expand_color_table(&mut color_table, u0, u1);
            exnand_color_index_table(&mut color_idx_table, raw_data, offset + 12);

            for j in 0..4 {
                for i in 0..4 {
                    let color_idx = color_idx_table[j * 4 + i] as usize;
                    let color = &color_table[color_idx];
                    let alpha = alpha_table[j * 4 + i];

                    img.put_pixel(x + i as u32, y + j as u32, image::Rgba([
                        color.r(),
                        color.g(),
                        color.b(),
                        alpha
                    ]));
                }
            }
        }
    }

    Ok(img)
}

fn get_image_from_dxt5(raw_data: &[u8], width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);

    let mut color_table = [Color::transparent(); 4];
    let mut color_idx_table = [0u8; 16];
    let mut alpha_table = [0u8; 16];
    let mut alpha_idx_table = [0u8; 16];
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let offset = (x * 4 + y * width) as usize;
            expand_alpha_table_dxt5(&mut alpha_table, raw_data[offset], raw_data[offset + 1]);
            expand_alpha_index_table_dxt5(&mut alpha_idx_table, raw_data, offset + 2);
            let u0: u16 = reader::read_u16_at(raw_data, offset + 8)?;
            let u1: u16 = reader::read_u16_at(raw_data, offset + 10)?;
            expand_color_table(&mut color_table, u0, u1);
            exnand_color_index_table(&mut color_idx_table, raw_data, offset + 12);

            for j in 0..4 {
                for i in 0..4 {
                    let color_idx = color_idx_table[j * 4 + i] as usize;
                    let color = &color_table[color_idx];
                    let alpha_idx = alpha_idx_table[j * 4 + i] as usize;
                    let alpha = alpha_table[alpha_idx];

                    img.put_pixel(x + i as u32, y + j as u32, image::Rgba([
                        color.r(),
                        color.g(),
                        color.b(),
                        alpha
                    ]));
                }
            }
        }
    }

    Ok(img)
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

fn expand_color_table(color_table: &mut [Color; 4], c0: u16, c1: u16) {
    color_table[0] = Color::from_rgb565(c0);
    color_table[1] = Color::from_rgb565(c1);

    let r = color_table[0].r() as i32;
    let g = color_table[0].g() as i32;
    let b = color_table[0].b() as i32;

    let r1 = color_table[1].r() as i32;
    let g1 = color_table[1].g() as i32;
    let b1 = color_table[1].b() as i32;

    if c0 > c1 {
        color_table[2] = Color(
            ((r * 2 + r1 + 1) / 3) as u8,
            ((g * 2 + g1 + 1) / 3) as u8,
            ((b * 2 + b1 + 1) / 3) as u8,
            255,
        );
        color_table[3] = Color(
            ((r + r1 * 2 + 1) / 3) as u8,
            ((g + g1 * 2 + 1) / 3) as u8,
            ((b + b1 * 2 + 1) / 3) as u8,
            255,
        );
    } else {
        color_table[2] = Color(
            ((r + r1) / 2) as u8,
            ((g + g1) / 2) as u8,
            ((b + b1) / 2) as u8,
            255,
        );
        color_table[3] = Color::black();
    }
}

fn exnand_color_index_table(color_index_table: &mut [u8; 16], raw_data: &[u8], offset: usize) {
    for i in 0..4 {
        let local_offset = offset + i;
        let color = raw_data[local_offset];
        color_index_table[i * 4] = color & 0x03;
        color_index_table[i * 4 + 1] = color & 0x0C >> 2;
        color_index_table[i * 4 + 2] = color & 0x30 >> 4;
        color_index_table[i * 4 + 3] = color & 0xC0 >> 6;
    }
}

fn expand_alpha_table_dxt3(alpha_table: &mut [u8; 16], raw_data: &[u8], offset: usize) {
    let mut local_offset = offset;
    for i in 0..8 {
        let alpha = raw_data[local_offset];
        alpha_table[i * 2] = alpha & 0x0F;
        alpha_table[i * 2 + 1] = (alpha & 0xf0) >> 4;

        local_offset += 1;
    }
    for item in alpha_table.iter_mut().take(16) {
        *item = *item | (*item << 4);
    }
}

fn expand_alpha_table_dxt5(alpha_table: &mut [u8; 16], a0: u8, a1: u8) {
    alpha_table[0] = a0;
    alpha_table[1] = a1;
    if a0 > a1 {
        for i in 2i32..8i32 {
            alpha_table[i as usize] = (((8 - i) * a0 as i32 + (i - 1) * a1 as i32 + 3) / 7) as u8;
        }
    } else {
        for i in 2i32..6i32 {
            alpha_table[i as usize] = (((6 - i) * a0 as i32 + (i - 1) * a1 as i32 + 2) / 5) as u8;
        }
        alpha_table[6] = 0;
        alpha_table[7] = 255;
    }
}

fn expand_alpha_index_table_dxt5(alpha_index_table: &mut [u8; 16], raw_data: &[u8], offset: usize) {
    for i in 0..2 {
        let local_offset = offset + i * 3;
        let flags = (raw_data[local_offset] as u32) | 
            ((raw_data[local_offset + 1] as u32) << 8) | 
            ((raw_data[local_offset + 2] as u32) << 16);
        for j in 0..8 {
            let mask = (7 << (3 * j)) as u32;
            alpha_index_table[(i * 8)+j] = ((flags & mask) >> (3 * j)) as u8;
        }
    }
}

fn get_image_from_bgra8888(raw_data: Vec<u8>, width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);
    let mut x = 0;
    let mut y = 0;
    let size = raw_data.len();
    for i in (0..size).step_by(4) {
        img.put_pixel(x, y, image::Rgba([
            raw_data[i + 2],
            raw_data[i + 1],
            raw_data[i],
            raw_data[i + 3]
        ]));
        x += 1;
        if x >= width {
            x = 0;
            y += 1;
        }
    }
    Ok(img)
}

fn get_image_from_rgb565(raw_data: &[u8], width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);
    let mut x = 0;
    let mut y = 0;
    let size = raw_data.len();
    for i in (0..size).step_by(2) {
        let color = Color::from_rgb565(reader::read_u16_at(raw_data, i)?);
        img.put_pixel(x, y, color.into());
        x += 1;
        if x >= (width) {
            x = 0;
            y += 1;
        }
    }
    Ok(img)
}

fn get_image_from_argb1555(raw_data: &[u8], width: u32, height: u32) -> Result<DynamicImage, WzPngParseError> {
    let mut img = DynamicImage::new_rgba8(width, height);
    let mut x = 0;
    let mut y = 0;
    let size = raw_data.len();
    for i in (0..size).step_by(2) {
        let color = Color::from_argb1555(reader::read_u16_at(raw_data, i)?);
        img.put_pixel(x, y, color.into());
        x += 1;
        if x >= (width) {
            x = 0;
            y += 1;
        }
    }
    Ok(img)
}