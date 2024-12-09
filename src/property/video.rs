use crate::WzReader;
use std::ops::Range;
use std::sync::Arc;

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
    #[inline]
    fn get_buffer_range(&self) -> Range<usize> {
        self.offset..self.offset + self.length
    }
    #[inline]
    pub fn get_buffer(&self) -> &[u8] {
        let range = self.get_buffer_range();
        self.reader.get_slice(range)
    }
}
