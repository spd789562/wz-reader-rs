use std::sync::Arc;
use crate::WzReader;

#[derive(Debug, Clone)]
pub struct WzRawData {
    pub reader: Arc<WzReader>,
    pub offset: usize,
    pub length: usize,
}

impl WzRawData {
    pub fn new(reader: &Arc<WzReader>, offset: usize, length: usize) -> Self {
        Self {
            reader: Arc::clone(reader),
            offset,
            length,
        }
    }
}