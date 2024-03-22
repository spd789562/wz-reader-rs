use image::{Rgba, Rgb};

pub trait SimpleColor 
    where Self: Sized {
    fn create(r: u8, g: u8, b: u8) -> Self;
    fn white() -> Self;
    fn black() -> Self;
    fn from_rgb565(color: u16) -> Self {
        let r = ((color & 0xF800) >> 11) as u8;
        let g = ((color & 0x07E0) >> 5) as u8;
        let b = (color & 0x001F) as u8;
    
        Self::create(r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2)
    }
    fn r(&self) -> u8;
    fn g(&self) -> u8;
    fn b(&self) -> u8;
}

pub trait SimpleColorAlpha 
    where Self: Sized {
    fn create_alpha(r: u8, g: u8, b: u8, a: u8) -> Self;
    fn transparent() -> Self;
    fn from_argb1555(color: u16) -> Self {
        let a = if (color & 0x8000) != 0 { 255 } else { 0 };
        let r = ((color & 0x7C00) >> 10) as u8;
        let g = ((color & 0x03E0) >> 5) as u8;
        let b = (color & 0x001F) as u8;
    
        Self::create_alpha(r << 3 | r >> 2, g << 3 | g >> 2, b << 3 | b >> 2, a)
    }
    fn a(&self) -> u8;
}

impl SimpleColor for Rgba<u8> {
    fn create(r: u8, g: u8, b: u8) -> Self {
        Rgba([r, g, b, 255])
    }
    fn white() -> Self {
        Rgba([255, 255, 255, 255])
    }
    fn black() -> Self {
        Rgba([0, 0, 0, 255])
    }
    fn r(&self) -> u8 {
        self[0]
    }
    fn g(&self) -> u8 {
        self[1]
    }
    fn b(&self) -> u8 {
        self[2]
    }
}

impl SimpleColorAlpha for Rgba<u8> {
    fn create_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Rgba([r, g, b, a])
    }
    fn transparent() -> Self {
        Rgba([0, 0, 0, 0])
    }
    fn a(&self) -> u8 {
        self[3]
    }
}

impl SimpleColor for Rgb<u8> {
    fn create(r: u8, g: u8, b: u8) -> Self {
        Rgb([r, g, b])
    }
    fn white() -> Self {
        Rgb([255, 255, 255])
    }
    fn black() -> Self {
        Rgb([0, 0, 0])
    }
    fn r(&self) -> u8 {
        self[0]
    }
    fn g(&self) -> u8 {
        self[1]
    }
    fn b(&self) -> u8 {
        self[2]
    }
}