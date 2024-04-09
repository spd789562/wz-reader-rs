pub mod vector;
pub mod png;
pub mod string;
pub mod sound;
pub mod lua;
pub mod raw_data;

pub use vector::*;
pub use png::*;
pub use string::*;
pub use sound::*;
pub use lua::*;
pub use raw_data::*;

// #[derive(Debug, Clone)]
// pub enum WzPropertyType {
//   Null,
//   Short(i16),
//   Int(i32),
//   Long(i64),
//   Float(f32),
//   Double(f64),
//   String(WzStringType),

//   SubProperty,
//   Vector(Vector2D),
//   Convex,
//   Sound(WzSoundMeta),
//   UOL(WzStringType),
//   Lua,

//   PNG(WzPng),

//   RawData,
// }

/* has subproperties */
#[derive(Debug, Clone)]

pub enum WzSubProperty {
  Property,
  Convex,
  Sound(Box<WzSound>),
  PNG(Box<WzPng>),
}

#[derive(Debug, Clone)]

pub enum WzValue {
  Null,
  RawData(WzRawData),
  Lua(WzLua),
  Short(i16),
  Int(i32),
  Long(i64),
  Float(f32),
  Double(f64),
  Vector(Vector2D),
  UOL(WzString),
  String(WzString),
}