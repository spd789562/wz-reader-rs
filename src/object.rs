use crate::{WzFile, WzDirectory, WzImage};
use crate::property::{WzSubProperty, WzValue, WzPng, WzSound, WzString, WzLua, WzRawData, Vector2D};

/// All variants of `WzObjectType`.
/// 
/// `WzObjectType` implement most of the From trait for the types that can be converted to it.
/// 
/// # Example
/// 
/// ```
/// # use wz_reader::WzObjectType;
/// # use wz_reader::property::WzValue;
/// let wz_int: WzObjectType = 1.into();
/// 
/// assert!(matches!(wz_int, WzObjectType::Value(WzValue::Int(1))));
/// ```
#[derive(Debug, Clone)]
pub enum WzObjectType {
    File(Box<WzFile>),
    Image(Box<WzImage>),
    Directory(Box<WzDirectory>),
    Property(WzSubProperty),
    Value(WzValue),
}

macro_rules! from_impl_wz_files {
    ($from_type:ident, $ot_variant:ident) => {
        impl From<$from_type> for WzObjectType {
            fn from(i: $from_type) -> Self {
                WzObjectType::$ot_variant(Box::new(i))
            }
        }
    };
}
macro_rules! from_impl_wz_value {
    ($from_type:ident, $variant:ident) => {
        impl From<$from_type> for WzObjectType {
            fn from(i: $from_type) -> Self {
                WzObjectType::Value(WzValue::$variant(i))
            }
        }
    };
}
macro_rules! from_impl_wz_property {
    ($from_type:ident, $variant:ident) => {
        impl From<$from_type> for WzObjectType {
            fn from(i: $from_type) -> Self {
                WzObjectType::Property(WzSubProperty::$variant(Box::new(i)))
            }
        }
    };
}

from_impl_wz_files!(WzFile, File);
from_impl_wz_files!(WzDirectory, Directory);
from_impl_wz_files!(WzImage, Image);

from_impl_wz_value!(i16, Short);
from_impl_wz_value!(i32, Int);
from_impl_wz_value!(i64, Long);
from_impl_wz_value!(f32, Float);
from_impl_wz_value!(f64, Double);
from_impl_wz_value!(WzString, String);
from_impl_wz_value!(Vector2D, Vector);
from_impl_wz_value!(WzRawData, RawData);
from_impl_wz_value!(WzLua, Lua);

from_impl_wz_property!(WzPng, PNG);
from_impl_wz_property!(WzSound, Sound);