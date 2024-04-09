use crate::{WzFile, WzDirectory, WzImage};
use crate::property::{WzSubProperty, WzValue};

#[derive(Debug, Clone)]
pub enum WzObjectType {
    File(Box<WzFile>),
    Image(Box<WzImage>),
    Directory(Box<WzDirectory>),
    Property(WzSubProperty),
    Value(WzValue),
}