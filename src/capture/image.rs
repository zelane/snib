//! Pure pixel math: what we do to a captured buffer before handing it to GTK.

use image::imageops;
use image::RgbaImage;
use wayland_client::protocol::wl_output;

/// Shrink to fit `max` on the constrained axis, preserving aspect ratio.
/// Never upscales.
pub(super) fn scaled_dims(w: u32, h: u32, fit_height: bool, max: u32) -> (u32, u32) {
    let (wf, hf, maxf) = (w as f32, h as f32, max as f32);
    if fit_height && h > max {
        ((wf * maxf / hf).round() as u32, max)
    } else if !fit_height && w > max {
        (max, (hf * maxf / wf).round() as u32)
    } else {
        (w, h)
    }
}

/// wl_shm XRGB/ARGB8888 are little-endian B,G,R,A in memory. Swizzle to RGBA.
pub(super) fn bgra_to_rgba(src: &[u8], opaque: bool) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for (o, i) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        o[0] = i[2];
        o[1] = i[1];
        o[2] = i[0];
        o[3] = if opaque { 255 } else { i[3] };
    }
    out
}

pub(super) fn apply_transform(img: RgbaImage, t: wl_output::Transform) -> RgbaImage {
    use wl_output::Transform::*;
    match t {
        Normal => img,
        _90 | Flipped90 => imageops::rotate90(&img),
        _180 | Flipped180 => imageops::rotate180(&img),
        _270 | Flipped270 => imageops::rotate270(&img),
        Flipped => imageops::flip_horizontal(&img),
        _ => img,
    }
}
