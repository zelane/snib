//! Wayland plumbing: the globals we bind, the per-object state we accumulate,
//! and the `Dispatch` impls that fill it in. Nothing here decides *what* to
//! capture — see the parent module for that.

use std::collections::HashMap;

use wayland_client::{
    backend::ObjectId,
    event_created_child,
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::{self, WlOutput},
        wl_registry::{self, WlRegistry},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
    },
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};

pub(super) fn debug_on() -> bool {
    std::env::var_os("SNIB_DEBUG").is_some()
}

pub(super) struct ToplevelInfo {
    pub handle: ExtForeignToplevelHandleV1,
    pub identifier: Option<String>,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub closed: bool,
}

pub(super) struct OutputInfo {
    pub output: WlOutput,
    pub name: Option<String>,
    pub model: Option<String>,
}

/// Scratch state for the source currently being captured.
#[derive(Default)]
pub(super) struct Capture {
    pub size: Option<(u32, u32)>,
    pub formats: Vec<u32>,
    pub constraints_done: bool,
    pub transform: Option<wl_output::Transform>,
    pub ready: bool,
    pub failed: bool,
    pub fail_reason: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    pub shm: Option<WlShm>,
    pub copy_mgr: Option<ExtImageCopyCaptureManagerV1>,
    pub output_src_mgr: Option<ExtOutputImageCaptureSourceManagerV1>,
    pub toplevel_src_mgr: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,

    pub tl_order: Vec<ObjectId>,
    pub toplevels: HashMap<ObjectId, ToplevelInfo>,
    pub out_order: Vec<ObjectId>,
    pub outputs: HashMap<ObjectId, OutputInfo>,

    pub cap: Capture,
}

// ------------------------------------------------------------- registry -----

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else {
            return;
        };
        let bind_at = |iface_ver: u32, cap: u32| version.min(iface_ver).min(cap);

        if interface == ExtForeignToplevelListV1::interface().name {
            let v = bind_at(ExtForeignToplevelListV1::interface().version, u32::MAX);
            registry.bind::<ExtForeignToplevelListV1, _, _>(name, v, qh, ());
        } else if interface == WlOutput::interface().name {
            let v = bind_at(WlOutput::interface().version, 4);
            let output = registry.bind::<WlOutput, _, _>(name, v, qh, ());
            let id = output.id();
            state.out_order.push(id.clone());
            state.outputs.insert(id, OutputInfo { output, name: None, model: None });
        } else if interface == WlShm::interface().name {
            let v = bind_at(WlShm::interface().version, u32::MAX);
            state.shm = Some(registry.bind::<WlShm, _, _>(name, v, qh, ()));
        } else if interface == ExtImageCopyCaptureManagerV1::interface().name {
            let v = bind_at(ExtImageCopyCaptureManagerV1::interface().version, u32::MAX);
            state.copy_mgr = Some(registry.bind::<ExtImageCopyCaptureManagerV1, _, _>(name, v, qh, ()));
        } else if interface == ExtOutputImageCaptureSourceManagerV1::interface().name {
            let v = bind_at(ExtOutputImageCaptureSourceManagerV1::interface().version, u32::MAX);
            state.output_src_mgr =
                Some(registry.bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(name, v, qh, ()));
        } else if interface == ExtForeignToplevelImageCaptureSourceManagerV1::interface().name {
            let v = bind_at(ExtForeignToplevelImageCaptureSourceManagerV1::interface().version, u32::MAX);
            state.toplevel_src_mgr = Some(
                registry.bind::<ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(name, v, qh, ()),
            );
        }
    }
}

// --------------------------------------------------------- enumeration -----

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            let id = toplevel.id();
            state.tl_order.push(id.clone());
            state.toplevels.insert(
                id,
                ToplevelInfo {
                    handle: toplevel,
                    identifier: None,
                    title: None,
                    app_id: None,
                    closed: false,
                },
            );
        }
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(info) = state.toplevels.get_mut(&handle.id()) else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                info.identifier = Some(identifier);
            }
            ext_foreign_toplevel_handle_v1::Event::Title { title } => info.title = Some(title),
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => info.app_id = Some(app_id),
            ext_foreign_toplevel_handle_v1::Event::Closed => info.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(info) = state.outputs.get_mut(&output.id()) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => info.name = Some(name),
            wl_output::Event::Geometry { model, .. } => {
                let m = model.trim();
                if !m.is_empty() && !m.eq_ignore_ascii_case("unknown") {
                    info.model = Some(m.to_string());
                }
            }
            _ => {}
        }
    }
}

// -------------------------------------------------------- capture events -----

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                state.cap.size = Some((width, height));
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat { format } => {
                // wlroots advertises shm formats as DRM fourccs, which don't
                // map onto wl_shm's special XRGB/ARGB values, so decoding to
                // the enum loses them. Keep the raw number.
                let raw = match format {
                    WEnum::Value(v) => v.into(),
                    WEnum::Unknown(u) => u,
                };
                state.cap.formats.push(raw);
            }
            ext_image_copy_capture_session_v1::Event::Done => state.cap.constraints_done = true,
            ext_image_copy_capture_session_v1::Event::Stopped => state.cap.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_frame_v1::Event::Transform { transform } => {
                if let WEnum::Value(t) = transform {
                    state.cap.transform = Some(t);
                }
            }
            ext_image_copy_capture_frame_v1::Event::Ready => state.cap.ready = true,
            ext_image_copy_capture_frame_v1::Event::Failed { reason } => {
                state.cap.failed = true;
                state.cap.fail_reason = Some(format!("{reason:?}"));
            }
            _ => {}
        }
    }
}

macro_rules! ignore_events {
    ($($t:ty),+ $(,)?) => {$(
        impl Dispatch<$t, ()> for State {
            fn event(_: &mut Self, _: &$t, _: <$t as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )+};
}
ignore_events!(
    WlShm,
    WlShmPool,
    WlBuffer,
    ExtImageCaptureSourceV1,
    ExtImageCopyCaptureManagerV1,
    ExtOutputImageCaptureSourceManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
);
