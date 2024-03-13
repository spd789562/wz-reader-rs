pub mod vector;
pub mod png;
pub mod string;
pub mod sound;

pub use vector::*;
pub use png::*;
pub use string::*;
pub use sound::*;

#[derive(Debug, Clone)]
pub enum WzPropertyType {
  Null,
  Short(i16),
  Int(i32),
  Long(i64),
  Float(f32),
  Double(f64),
  String(WzStringMeta),

  SubProperty,
  Vector(Vector2D),
  Convex,
  Sound(WzSoundMeta),
  UOL(WzStringMeta),
  Lua,

  PNG(WzPng),

  RawData,
}

impl WzPropertyType {
  pub fn get_short(&self) -> &i16 {
    match self {
      WzPropertyType::Short(num) => num,
      _ => &0
    }
  }
  pub fn get_int(&self) -> &i32 {
    match self {
      WzPropertyType::Int(num) => num,
      _ => &0
    }
  }
  pub fn get_long(&self) -> &i64 {
    match self {
      WzPropertyType::Long(num) => num,
      _ => &0
    }
  }
  pub fn get_float(&self) -> &f32 {
    match self {
      WzPropertyType::Float(num) => num,
      _ => &0_f32
    }
  }
  pub fn get_double(&self) -> &f64 {
    match self {
      WzPropertyType::Double(num) => num,
      _ => &0_f64
    }
  }
  pub fn get_vector(&self) -> &Vector2D {
    match self {
      WzPropertyType::Vector(vector) => vector,
      _ => &Vector2D(0, 0)
    }
  }
  pub fn get_png(&self) -> Result<&WzPng, String> {
    match self {
      WzPropertyType::PNG(png) => Ok(png),
      _ => Err("Not a PNG".to_string())
    }
  }
}