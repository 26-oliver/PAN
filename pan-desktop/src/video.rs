use eframe::egui::{Color32, ColorImage};
use std::{
    cell::UnsafeCell,
    ffi::{c_char, c_uint, c_void},
    sync::{Arc, Mutex},
};

pub type Setup = unsafe extern "C" fn(
    *mut *mut c_void,
    *mut c_char,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
) -> c_uint;
pub type Cleanup = unsafe extern "C" fn(*mut c_void);
pub type Lock = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void;
pub type Unlock = unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void);
pub type Display = unsafe extern "C" fn(*mut c_void, *mut c_void);

pub type Frames = Arc<Mutex<Option<ColorImage>>>;

#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct Block([u8; 32]);

struct Output {
    latest: Frames,
    width: usize,
    height: usize,
    pitch: usize,
    // VLC's vmem prepare/display callbacks run serially for each video output.
    // One aligned plane stays writable for the lifetime of that output.
    pixels: Box<[UnsafeCell<Block>]>,
}

pub unsafe extern "C" fn setup(
    opaque: *mut *mut c_void,
    chroma: *mut c_char,
    width: *mut c_uint,
    height: *mut c_uint,
    pitches: *mut c_uint,
    lines: *mut c_uint,
) -> c_uint {
    let latest = &*opaque.read().cast::<Frames>();
    let (mut w, mut h) = (width.read() as usize, height.read() as usize);
    if w == 0 || h == 0 {
        return 0;
    }
    let longest = w.max(h);
    if longest > 3840 {
        w = (w.saturating_mul(3840) / longest).max(1);
        h = (h.saturating_mul(3840) / longest).max(1);
    }
    let pitch = (w * 4).next_multiple_of(32);
    let padded_height = h.next_multiple_of(32);
    let mut pixels = Vec::new();
    if pixels
        .try_reserve_exact(pitch * padded_height / 32)
        .is_err()
    {
        return 0;
    }
    pixels.resize_with(pitch * padded_height / 32, || {
        UnsafeCell::new(Block([0; 32]))
    });
    let output = Box::new(Output {
        latest: latest.clone(),
        width: w,
        height: h,
        pitch,
        pixels: pixels.into_boxed_slice(),
    });

    std::ptr::copy_nonoverlapping(b"RV32".as_ptr(), chroma.cast(), 4);
    width.write(w as u32);
    height.write(h as u32);
    pitches.write(pitch as u32);
    lines.write(padded_height as u32);
    opaque.write(Box::into_raw(output).cast());
    1
}

pub unsafe extern "C" fn cleanup(opaque: *mut c_void) {
    drop(Box::from_raw(opaque.cast::<Output>()));
}

pub unsafe extern "C" fn lock(opaque: *mut c_void, planes: *mut *mut c_void) -> *mut c_void {
    let output = &*opaque.cast::<Output>();
    planes.write(output.pixels[0].get().cast());
    opaque
}

pub unsafe extern "C" fn display(opaque: *mut c_void, _picture: *mut c_void) {
    let output = &*opaque.cast::<Output>();
    let bytes = std::slice::from_raw_parts(
        output.pixels[0].get().cast::<u8>(),
        output.pitch * output.height,
    );
    let mut pixels = Vec::with_capacity(output.width * output.height);
    for row in bytes.chunks_exact(output.pitch) {
        pixels.extend(
            row[..output.width * 4]
                .chunks_exact(4)
                .map(|pixel| Color32::from_rgb(pixel[2], pixel[1], pixel[0])),
        );
    }
    if let Ok(mut latest) = output.latest.lock() {
        *latest = Some(ColorImage::new([output.width, output.height], pixels));
    }
}
