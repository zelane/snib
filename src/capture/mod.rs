//! Enumerate every capturable toplevel and output over one Wayland connection,
//! then grab and downscale a thumbnail of each.

mod image;
mod protocol;

use std::os::fd::AsFd;

// `::image` is the crate; `self::image` is our pixel-math module below.
use ::image::{imageops, RgbaImage};
use rustix::fs::{ftruncate, memfd_create, MemfdFlags};
use rustix::mm::{mmap, munmap, MapFlags, ProtFlags};
use wayland_client::{
    protocol::{
        wl_output,
        wl_shm::{self, WlShm},
    },
    Connection, EventQueue,
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};

use self::image::{apply_transform, bgra_to_rgba, scaled_dims};
use self::protocol::{debug_on, Capture, State};
use crate::cli::Kind;
use crate::source::{Extra, Source};

/// Pump the queue until `done` observes what it's waiting for, or we give up.
fn deblock(state: &mut State, queue: &mut EventQueue<State>, done: fn(&Capture) -> bool) -> bool {
    for _ in 0..512 {
        if done(&state.cap) {
            return true;
        }
        if queue.blocking_dispatch(state).is_err() {
            return false;
        }
    }
    false
}

fn capture_source(
    queue: &mut EventQueue<State>,
    state: &mut State,
    copy_mgr: &ExtImageCopyCaptureManagerV1,
    shm: &WlShm,
    source: &ExtImageCaptureSourceV1,
    max: u32,
    fit_height: bool,
) -> Option<(u32, u32, Vec<u8>)> {
    let qh = queue.handle();
    state.cap = Capture::default();

    let session = copy_mgr.create_session(source, Options::empty(), &qh, ());
    let got = deblock(state, queue, |c| c.constraints_done || c.failed);
    if debug_on() {
        eprintln!(
            "[snib]  session done={} failed={} size={:?} formats={:?}",
            state.cap.constraints_done, state.cap.failed, state.cap.size, state.cap.formats
        );
    }
    if !got || state.cap.failed {
        session.destroy();
        return None;
    }

    let (w, h) = state.cap.size?;
    // Prefer opaque XRGB, else ARGB. Both are BGRA byte order.
    // XRGB/ARGB8888 come through as either wl_shm's special values (1/0) or
    // the equivalent DRM fourccs ('XR24'/'AR24'), depending on the compositor.
    const XR24: u32 = 0x3432_5258;
    const AR24: u32 = 0x3432_5241;
    let has = |a: u32, b: u32| state.cap.formats.iter().any(|&f| f == a || f == b);
    let (format, opaque) = if has(u32::from(wl_shm::Format::Xrgb8888), XR24) {
        (wl_shm::Format::Xrgb8888, true)
    } else if has(u32::from(wl_shm::Format::Argb8888), AR24) {
        (wl_shm::Format::Argb8888, false)
    } else {
        (wl_shm::Format::Xrgb8888, true)
    };

    let stride = (w * 4) as usize;
    let size = stride * h as usize;

    let fd = memfd_create("snib", MemfdFlags::CLOEXEC).ok()?;
    ftruncate(&fd, size as u64).ok()?;
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::SHARED,
            &fd,
            0,
        )
    }
    .ok()? as *mut u8;

    let pool = shm.create_pool(fd.as_fd(), size as i32, &qh, ());
    let buffer = pool.create_buffer(0, w as i32, h as i32, stride as i32, format, &qh, ());

    let frame = session.create_frame(&qh, ());
    frame.attach_buffer(&buffer);
    frame.damage_buffer(0, 0, w as i32, h as i32);
    frame.capture();

    let ok = deblock(state, queue, |c| c.ready || c.failed) && state.cap.ready;
    if debug_on() {
        eprintln!(
            "[snib]  frame ready={} failed={} reason={:?}",
            state.cap.ready, state.cap.failed, state.cap.fail_reason
        );
    }

    let result = if ok {
        let raw = unsafe { std::slice::from_raw_parts(ptr, size) };
        let rgba = bgra_to_rgba(raw, opaque);
        let mut img = RgbaImage::from_raw(w, h, rgba)?;
        img = apply_transform(img, state.cap.transform.unwrap_or(wl_output::Transform::Normal));
        let (iw, ih) = img.dimensions();
        let (tw, th) = scaled_dims(iw, ih, fit_height, max);
        let small = imageops::thumbnail(&img, tw.max(1), th.max(1));
        Some((tw.max(1), th.max(1), small.into_raw()))
    } else {
        None
    };

    frame.destroy();
    buffer.destroy();
    pool.destroy();
    session.destroy();
    unsafe {
        let _ = munmap(ptr as *mut _, size);
    }
    result
}

// ------------------------------------------------------------------ api -----

/// A source we've decided to capture, before its pixels exist.
struct Job {
    kind: Kind,
    identifier: String,
    app_id: String,
    caption: String,
    haystack: String,
    source: ExtImageCaptureSourceV1,
}

pub fn capture_thumbnails(max: u32, fit_height: bool) -> Vec<Source> {
    let Ok(conn) = Connection::connect_to_env() else {
        return Vec::new();
    };
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = State::default();

    // #1 globals + binds, #2 toplevel handles + output props.
    if queue.roundtrip(&mut state).is_err() || queue.roundtrip(&mut state).is_err() {
        return Vec::new();
    }

    if debug_on() {
        eprintln!(
            "[snib] globals: shm={} copy_mgr={} out_mgr={} tl_mgr={}",
            state.shm.is_some(), state.copy_mgr.is_some(),
            state.output_src_mgr.is_some(), state.toplevel_src_mgr.is_some(),
        );
        let tl = state.toplevels.values().filter(|t| !t.closed && t.identifier.is_some()).count();
        let out = state.outputs.values().filter(|o| o.name.is_some()).count();
        eprintln!("[snib] enumerated {tl} toplevels, {out} outputs");
    }

    let (Some(copy_mgr), Some(shm)) = (state.copy_mgr.clone(), state.shm.clone()) else {
        return Vec::new();
    };

    let mut jobs: Vec<Job> = Vec::new();

    if let Some(mgr) = state.toplevel_src_mgr.clone() {
        for id in &state.tl_order {
            let info = &state.toplevels[id];
            if info.closed {
                continue;
            }
            let Some(identifier) = info.identifier.clone() else {
                continue;
            };
            let caption = info
                .title
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| info.app_id.clone())
                .unwrap_or_else(|| "(untitled)".to_string());
            let haystack = format!(
                "{} {}",
                info.title.as_deref().unwrap_or_default(),
                info.app_id.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            jobs.push(Job {
                kind: Kind::Window,
                identifier,
                app_id: info.app_id.clone().unwrap_or_default(),
                caption,
                haystack,
                source: mgr.create_source(&info.handle, &qh, ()),
            });
        }
    }

    if let Some(mgr) = state.output_src_mgr.clone() {
        for id in &state.out_order {
            let info = &state.outputs[id];
            let Some(name) = info.name.clone() else {
                continue;
            };
            let caption = match &info.model {
                Some(m) => format!("{name} — {m}"),
                None => name.clone(),
            };
            jobs.push(Job {
                kind: Kind::Display,
                identifier: name,
                app_id: String::new(),
                haystack: caption.to_lowercase(),
                caption,
                source: mgr.create_source(&info.output, &qh, ()),
            });
        }
    }

    if debug_on() {
        eprintln!("[snib] built {} capture jobs", jobs.len());
    }

    let mut thumbs = Vec::new();
    for job in jobs {
        if let Some((w, h, rgba)) =
            capture_source(&mut queue, &mut state, &copy_mgr, &shm, &job.source, max, fit_height)
        {
            thumbs.push(Source {
                kind: job.kind,
                identifier: job.identifier,
                app_id: job.app_id,
                caption: job.caption,
                haystack: job.haystack,
                size: [w, h],
                rgba,
                extra: Extra::new(),
            });
        }
        job.source.destroy();
    }
    thumbs
}
