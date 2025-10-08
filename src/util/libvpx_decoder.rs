use vpx_sys::{
    vpx_codec_ctx, vpx_codec_dec_init_ver, vpx_codec_decode, vpx_codec_destroy, vpx_codec_err_t,
    vpx_codec_get_frame, vpx_codec_iter_t, vpx_codec_vp8_dx, vpx_codec_vp9_dx, vpx_image_t,
    vpx_img_fmt_t, VPX_CODEC_ABI_VERSION,
};

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Arc;

use av_data::frame::{Frame, FrameBufferCopy, FrameType, VideoInfo};
use av_data::pixel::formats::YUV420;
use thiserror::Error;
use yuvutils_rs::{BufferStoreMut, YuvPlanarImage, YuvPlanarImageMut};

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Failed to create decoder {0:?}")]
    CreateFailed(vpx_codec_err_t),
}

pub struct Decoder<T> {
    pub ctx: vpx_codec_ctx,
    pub iter: vpx_codec_iter_t,
    private_data: PhantomData<T>,
}

pub enum VpVersion {
    Vp8,
    Vp9,
}

fn frame_from_img(img: vpx_image_t) -> Frame {
    let format = match img.fmt {
        vpx_img_fmt_t::VPX_IMG_FMT_I420 => YUV420,
        _ => panic!("TODO: support more pixel formats"),
    };
    let video = VideoInfo::new(
        img.d_w as usize,
        img.d_h as usize,
        false,
        FrameType::OTHER,
        Arc::new(*format),
    );

    let mut frame = Frame::new_default_frame(video, None);

    let mut src = img
        .planes
        .iter()
        .zip(img.stride.iter())
        .zip(format.iter())
        .map(|((plane, line), chromaton)| unsafe {
            (
                std::slice::from_raw_parts(
                    *plane as *const u8,
                    *line as usize * chromaton.map(|c| c.get_height(img.h as usize)).unwrap_or(0),
                ),
                *line as u32,
            )
        });
    let Some((y_plane, y_stride)) = src.next() else {
        panic!("No Y plane found");
    };
    let Some((u_plane, u_stride)) = src.next() else {
        panic!("No U plane found");
    };
    let Some((v_plane, v_stride)) = src.next() else {
        panic!("No V plane found");
    };

    let image_plane = YuvPlanarImage {
        y_plane,
        u_plane,
        v_plane,
        y_stride,
        u_stride,
        v_stride,
        width: img.d_w as u32,
        height: img.d_h as u32,
    };

    frame
}

// the part of implementation and comment is from https://github.com/rust-av/vpx-rs/blob/master/src/decoder.rs
impl<T> Decoder<T> {
    /// Create a new decoder
    ///
    /// # Errors
    ///
    /// The function may fail if provide wrong version or the underlying library has no respective vp8/vp9.
    pub fn new(version: VpVersion) -> Result<Self, DecoderError> {
        let mut ctx: MaybeUninit<vpx_codec_ctx> = MaybeUninit::uninit();
        let cfg = MaybeUninit::zeroed();
        let ret = unsafe {
            let dx = match version {
                VpVersion::Vp8 => vpx_codec_vp8_dx(),
                VpVersion::Vp9 => vpx_codec_vp9_dx(),
            };
            vpx_codec_dec_init_ver(
                ctx.as_mut_ptr(),
                dx,
                cfg.as_ptr(),
                0,
                VPX_CODEC_ABI_VERSION as i32,
            )
        };
        if ret != vpx_codec_err_t::VPX_CODEC_OK {
            return Err(DecoderError::CreateFailed(ret));
        }
        Ok(Self {
            ctx: unsafe { ctx.assume_init() },
            iter: ptr::null(),
            private_data: PhantomData,
        })
    }
    /// Feed some compressed data to the encoder
    ///
    /// The `data` slice is sent to the decoder alongside the optional
    /// `private` struct.
    ///
    /// The [`get_frame`] method must be called to retrieve the decompressed
    /// frame, do not call this method again before calling [`get_frame`].
    ///
    /// It matches a call to `vpx_codec_decode`.
    ///
    /// [`get_frame`]: #method.get_frame
    pub fn decode(&mut self, data: &[u8]) -> Result<(), vpx_codec_err_t> {
        let ret = unsafe {
            vpx_codec_decode(
                &mut self.ctx,
                data.as_ptr(),
                data.len() as u32,
                std::ptr::null_mut(),
                0,
            )
        };
        // Safety measure to not call get_frame on an invalid iterator
        self.iter = ptr::null();

        match ret {
            vpx_codec_err_t::VPX_CODEC_OK => Ok(()),
            _ => Err(ret),
        }
    }

    /// Notify the decoder to return any pending frame
    ///
    /// The [`get_frame`] method must be called to retrieve the decompressed
    /// frame.
    ///
    /// It matches a call to `vpx_codec_decode` with NULL arguments.
    ///
    /// [`get_frame`]: #method.get_frame
    pub fn flush(&mut self) -> Result<(), vpx_codec_err_t> {
        let ret = unsafe { vpx_codec_decode(&mut self.ctx, ptr::null(), 0, ptr::null_mut(), 0) };

        self.iter = ptr::null();

        match ret {
            vpx_codec_err_t::VPX_CODEC_OK => Ok(()),
            _ => Err(ret),
        }
    }

    /// Retrieve decoded frames
    ///
    /// Should be called repeatedly until it returns `None`.
    ///
    /// It matches a call to `vpx_codec_get_frame`.
    pub fn get_frame(&mut self) -> Option<(Frame, Option<Box<T>>)> {
        let img = unsafe { vpx_codec_get_frame(&mut self.ctx, &mut self.iter) };
        if img.is_null() {
            None
        } else {
            let im = unsafe { *img };
            let priv_data = if im.user_priv.is_null() {
                None
            } else {
                let p = im.user_priv as *mut T;
                Some(unsafe { Box::from_raw(p) })
            };
            let frame = frame_from_img(im);
            Some((frame, priv_data))
        }
    }
}

impl<T> Drop for Decoder<T> {
    fn drop(&mut self) {
        unsafe {
            vpx_codec_destroy(&mut self.ctx);
        }
    }
}
