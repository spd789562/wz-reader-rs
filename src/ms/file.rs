use crate::{
    directory, reader, version, Reader, SharedWzMutableKey, WzDirectory, WzNodeArc, WzNodeArcVec,
    WzNodeCast, WzObjectType, WzReader, WzSliceReader,
};
use memmap2::Mmap;
use std::fs::File;
use std::sync::Arc;

use super::header::{self, MsHeader};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    FileError(#[from] std::io::Error),
    #[error("invald ms file")]
    InvalidMsFile,
    #[error("Failed, in this case the causes are undetermined.")]
    FailedUnknown,
    #[error("Binary reading error")]
    ReaderError(#[from] reader::Error),
    #[error(transparent)]
    HeaderReadError(#[from] header::Error),
    #[error("[MsFile] New Wz image header found. checkByte = {0}, File Name = {1}")]
    UnknownImageHeader(u8, String),
}

/// Root of the `WzNode`, represents the Wz file itself and contains `MsFileMeta`
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct MsFile {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub reader: Arc<WzReader>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub block_size: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub is_parsed: bool,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub header: MsHeader,
}

impl MsFile {
    pub fn from_file<P>(path: P, wz_iv: Option<[u8; 4]>) -> Result<MsFile, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file: File = File::open(&path)?;
        let map = unsafe { Mmap::map(&file)? };

        let block_size = map.len();

        let reader = WzReader::new(map);

        let ms_header = MsHeader::from_ms_file(path, &reader)?;

        Ok(MsFile {
            block_size,
            is_parsed: false,
            reader: Arc::new(reader),
            header: ms_header,
        })
    }
    pub fn parse(
        &mut self,
        parent: &WzNodeArc,
        patch_version: Option<i32>,
    ) -> Result<WzNodeArcVec, Error> {
        unimplemented!()
    }
}
