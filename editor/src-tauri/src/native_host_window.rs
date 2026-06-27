//! Platform adapters for native host-window presentation.
//!
//! The target model is that native code owns the editor root window and embeds
//! Web UI as panels/overlays.

use std::num::NonZeroIsize;
use std::sync::mpsc;
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::editor_compositor;
use crate::scene_window;

pub struct NativeHostSceneTarget {
    pub surface: scene_window::SceneRawSurface,
    pub layout_mode: NativeHostLayoutMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct NativeHostSceneRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct NativeHostPanelState {
    pub hierarchy_open: bool,
    pub inspector_open: bool,
    pub ai_panel_open: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct NativeHostLayoutState {
    pub scene_rect: Option<NativeHostSceneRect>,
    pub panels: NativeHostPanelState,
    pub host_root_active: bool,
}

impl From<scene_window::SceneViewportRect> for NativeHostSceneRect {
    fn from(rect: scene_window::SceneViewportRect) -> Self {
        let rect = rect.sanitized();
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum NativeHostLayoutMode {
    /// Target model: native host window owns root geometry and embeds Web UI views.
    HostOwnedRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostBackend {
    X11,
    Wayland,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostWindowBackend {
    LinuxX11,
    LinuxWayland,
    Win32,
    AppKit,
    MobileRootView,
    UnsupportedDesktop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostRoute {
    LinuxHostRoot(NativeHostBackend),
    WindowsDirectComposition,
    MacosCoreAnimation,
    RootWindowSurface,
    UnsupportedDesktop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowsDirectCompositionHostPlan {
    platform: editor_compositor::NativeHostPlatformPlan,
    native_host_root: &'static str,
    scene_surface_route: &'static str,
    web_ui_route: &'static str,
}

impl WindowsDirectCompositionHostPlan {
    fn unavailable_error(self) -> String {
        platform_plan_error(
            self.platform,
            &[
                self.native_host_root,
                self.scene_surface_route,
                self.web_ui_route,
            ],
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MacosCoreAnimationHostPlan {
    platform: editor_compositor::NativeHostPlatformPlan,
    native_host_root: &'static str,
    scene_surface_route: &'static str,
    web_ui_route: &'static str,
}

impl MacosCoreAnimationHostPlan {
    fn unavailable_error(self) -> String {
        platform_plan_error(
            self.platform,
            &[
                self.native_host_root,
                self.scene_surface_route,
                self.web_ui_route,
            ],
        )
    }
}

const WINDOWS_DIRECTCOMPOSITION_HOST_PLAN: WindowsDirectCompositionHostPlan =
    WindowsDirectCompositionHostPlan {
        platform: editor_compositor::WINDOWS_NATIVE_HOST_PLAN,
        native_host_root: "native host root: HWND-owned DirectComposition visual tree",
        scene_surface_route: "scene route: WGPU output presented through a DXGI/DirectComposition surface",
        web_ui_route: "web UI route: WebView2 CompositionController attached as hosted visuals",
    };

const MACOS_CORE_ANIMATION_HOST_PLAN: MacosCoreAnimationHostPlan = MacosCoreAnimationHostPlan {
    platform: editor_compositor::MACOS_NATIVE_HOST_PLAN,
    native_host_root: "native host root: NSWindow/NSView-owned Core Animation layer tree",
    scene_surface_route: "scene route: WGPU output presented through CAMetalLayer",
    web_ui_route: "web UI route: WKWebView/AppKit panels embedded in the native view tree",
};

pub fn install_host_root_on_main_thread(
    app: &tauri::AppHandle,
) -> Result<NativeHostLayoutMode, String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let handle = window
        .window_handle()
        .map_err(|error| format!("main window handle: {error}"))?
        .as_raw();
    match native_host_route(native_host_window_backend(handle)) {
        NativeHostRoute::LinuxHostRoot(NativeHostBackend::X11) => {
            install_linux_x11_host_root_on_main_thread(app)?;
            Ok(NativeHostLayoutMode::HostOwnedRoot)
        }
        NativeHostRoute::LinuxHostRoot(NativeHostBackend::Wayland) => Err(native_wayland_error()),
        NativeHostRoute::WindowsDirectComposition => {
            Err(WINDOWS_DIRECTCOMPOSITION_HOST_PLAN.unavailable_error())
        }
        NativeHostRoute::MacosCoreAnimation => {
            Err(MACOS_CORE_ANIMATION_HOST_PLAN.unavailable_error())
        }
        NativeHostRoute::RootWindowSurface => Ok(NativeHostLayoutMode::HostOwnedRoot),
        NativeHostRoute::UnsupportedDesktop => Err(format!(
            "native host window Scene View does not support this desktop backend yet: {handle:?}"
        )),
    }
}

#[cfg(target_os = "linux")]
fn install_linux_x11_host_root_on_main_thread(
    app: &tauri::AppHandle,
) -> Result<scene_window::SceneRawSurface, String> {
    ensure_x11_host_surface_on_main_thread(app)
}

#[cfg(not(target_os = "linux"))]
fn install_linux_x11_host_root_on_main_thread(
    _app: &tauri::AppHandle,
) -> Result<scene_window::SceneRawSurface, String> {
    Err("Linux X11 native host adapter can only run on Linux.".to_owned())
}

fn native_host_window_backend(handle: RawWindowHandle) -> NativeHostWindowBackend {
    match handle {
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => NativeHostWindowBackend::LinuxX11,
        RawWindowHandle::Wayland(_) => NativeHostWindowBackend::LinuxWayland,
        RawWindowHandle::Win32(_) => NativeHostWindowBackend::Win32,
        RawWindowHandle::AppKit(_) => NativeHostWindowBackend::AppKit,
        RawWindowHandle::UiKit(_) | RawWindowHandle::AndroidNdk(_) => {
            NativeHostWindowBackend::MobileRootView
        }
        _ => NativeHostWindowBackend::UnsupportedDesktop,
    }
}

fn native_host_route(backend: NativeHostWindowBackend) -> NativeHostRoute {
    match backend {
        NativeHostWindowBackend::LinuxX11 => NativeHostRoute::LinuxHostRoot(NativeHostBackend::X11),
        NativeHostWindowBackend::LinuxWayland => {
            NativeHostRoute::LinuxHostRoot(NativeHostBackend::Wayland)
        }
        NativeHostWindowBackend::Win32 => NativeHostRoute::WindowsDirectComposition,
        NativeHostWindowBackend::AppKit => NativeHostRoute::MacosCoreAnimation,
        NativeHostWindowBackend::MobileRootView => NativeHostRoute::RootWindowSurface,
        NativeHostWindowBackend::UnsupportedDesktop => NativeHostRoute::UnsupportedDesktop,
    }
}

pub fn main_window_scene_target(app: &tauri::AppHandle) -> Result<NativeHostSceneTarget, String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let handle = window
        .window_handle()
        .map_err(|error| format!("main window handle: {error}"))?
        .as_raw();
    match native_host_route(native_host_window_backend(handle)) {
        NativeHostRoute::LinuxHostRoot(NativeHostBackend::X11) => {
            create_linux_host_surface(app.clone(), NativeHostBackend::X11)
        }
        NativeHostRoute::LinuxHostRoot(NativeHostBackend::Wayland) => Err(native_wayland_error()),
        NativeHostRoute::WindowsDirectComposition => create_windows_host_scene_target(),
        NativeHostRoute::MacosCoreAnimation => create_macos_host_scene_target(),
        NativeHostRoute::RootWindowSurface => create_root_window_scene_surface(app),
        NativeHostRoute::UnsupportedDesktop => Err(format!(
            "native host window Scene View does not support this desktop backend yet: {handle:?}"
        )),
    }
}

fn native_wayland_error() -> String {
    "native Wayland Scene View embedding is disabled; start the editor under X11/Xwayland so GTK exposes X11 handles".to_owned()
}

pub fn resize_main_window_scene_surface(
    app: tauri::AppHandle,
    rect: NativeHostSceneRect,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = mpsc::channel();
        app.clone()
            .run_on_main_thread(move || {
                let result = resize_linux_host_surface_on_main_thread(&app, rect);
                let _ = tx.send(result);
            })
            .map_err(|error| format!("schedule native host surface resize: {error}"))?;
        return rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("native host surface resize timed out: {error}"))?;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        let _ = rect;
        Ok(())
    }
}

fn platform_plan_error(
    plan: editor_compositor::NativeHostPlatformPlan,
    host_boundaries: &[&str],
) -> String {
    let mut blocking_work = Vec::with_capacity(host_boundaries.len() + plan.blocking_work.len());
    blocking_work.extend_from_slice(host_boundaries);
    blocking_work.extend_from_slice(plan.blocking_work);
    format!(
        "{} Blocking work: {}",
        plan.status,
        blocking_work.join("; ")
    )
}

#[cfg(target_os = "windows")]
fn create_windows_host_scene_target() -> Result<NativeHostSceneTarget, String> {
    Err(WINDOWS_DIRECTCOMPOSITION_HOST_PLAN.unavailable_error())
}

#[cfg(not(target_os = "windows"))]
fn create_windows_host_scene_target() -> Result<NativeHostSceneTarget, String> {
    Err("Windows native host adapter can only run on Windows.".to_owned())
}

#[cfg(target_os = "macos")]
fn create_macos_host_scene_target() -> Result<NativeHostSceneTarget, String> {
    Err(MACOS_CORE_ANIMATION_HOST_PLAN.unavailable_error())
}

#[cfg(not(target_os = "macos"))]
fn create_macos_host_scene_target() -> Result<NativeHostSceneTarget, String> {
    Err("macOS native host adapter can only run on macOS.".to_owned())
}

fn create_root_window_scene_surface(
    app: &tauri::AppHandle,
) -> Result<NativeHostSceneTarget, String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let handle = window
        .window_handle()
        .map_err(|error| format!("main window handle: {error}"))?
        .as_raw();
    let surface = match handle {
        RawWindowHandle::Win32(handle) => scene_window::SceneRawSurface::Win32 {
            hwnd: handle.hwnd.get(),
            hinstance: handle.hinstance.map(NonZeroIsize::get),
        },
        RawWindowHandle::AppKit(handle) => scene_window::SceneRawSurface::AppKit {
            ns_view: handle.ns_view.as_ptr() as usize,
        },
        RawWindowHandle::UiKit(handle) => scene_window::SceneRawSurface::UiKit {
            ui_view: handle.ui_view.as_ptr() as usize,
            ui_view_controller: handle.ui_view_controller.map(|ptr| ptr.as_ptr() as usize),
        },
        RawWindowHandle::AndroidNdk(handle) => scene_window::SceneRawSurface::AndroidNdk {
            a_native_window: handle.a_native_window.as_ptr() as usize,
        },
        other => {
            return Err(format!(
                "native host root surface does not support this desktop backend yet: {other:?}"
            ));
        }
    };
    Ok(NativeHostSceneTarget {
        surface,
        layout_mode: NativeHostLayoutMode::HostOwnedRoot,
    })
}

#[cfg(target_os = "linux")]
fn create_linux_host_surface(
    app: tauri::AppHandle,
    backend: NativeHostBackend,
) -> Result<NativeHostSceneTarget, String> {
    let (tx, rx) = mpsc::channel();
    app.clone()
        .run_on_main_thread(move || {
            let result = create_linux_host_surface_on_main_thread(&app, backend);
            let _ = tx.send(result);
        })
        .map_err(|error| format!("schedule native host surface creation: {error}"))?;
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|error| format!("native host surface creation timed out: {error}"))?
}

#[cfg(not(target_os = "linux"))]
fn create_linux_host_surface(
    _app: tauri::AppHandle,
    _backend: NativeHostBackend,
) -> Result<NativeHostSceneTarget, String> {
    Err("native host window adapter is not implemented on this platform yet".to_owned())
}

#[cfg(target_os = "linux")]
fn create_linux_host_surface_on_main_thread(
    app: &tauri::AppHandle,
    backend: NativeHostBackend,
) -> Result<NativeHostSceneTarget, String> {
    use gtk::prelude::*;

    if backend == NativeHostBackend::X11 {
        let surface = ensure_x11_host_surface_on_main_thread(app)?;
        return Ok(NativeHostSceneTarget {
            surface,
            layout_mode: NativeHostLayoutMode::HostOwnedRoot,
        });
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let vbox = window
        .default_vbox()
        .map_err(|error| format!("main GTK vbox: {error}"))?;
    let vbox_widget: gtk::Widget = vbox.upcast();
    let _host_root = ensure_native_host_root(&vbox_widget)?;

    let drawing = find_named_widget(&vbox_widget, HOST_DRAWING_NAME)
        .and_then(|widget| widget.downcast::<gtk::DrawingArea>().ok())
        .ok_or_else(|| "native host drawing surface is missing".to_owned())?;
    drawing.show_all();
    drawing.realize();
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }

    let surface = gtk_drawing_area_raw_surface(&drawing, backend)?;
    Ok(NativeHostSceneTarget {
        surface,
        layout_mode: NativeHostLayoutMode::HostOwnedRoot,
    })
}

#[cfg(target_os = "linux")]
fn resize_linux_host_surface_on_main_thread(
    app: &tauri::AppHandle,
    rect: NativeHostSceneRect,
) -> Result<(), String> {
    use gtk::prelude::*;

    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let handle = window
        .window_handle()
        .map_err(|error| format!("main window handle: {error}"))?
        .as_raw();
    if native_host_window_backend(handle) == NativeHostWindowBackend::LinuxX11 {
        ensure_x11_host_surface_on_main_thread(app)?;
        return resize_x11_host_surface_on_main_thread(rect);
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let vbox = window
        .default_vbox()
        .map_err(|error| format!("main GTK vbox: {error}"))?;
    let vbox_widget: gtk::Widget = vbox.upcast();
    let host_root = ensure_native_host_root(&vbox_widget)?;
    let drawing = find_named_widget(&vbox_widget, HOST_DRAWING_NAME)
        .and_then(|widget| widget.downcast::<gtk::DrawingArea>().ok())
        .ok_or_else(|| "native host drawing surface is missing".to_owned())?;

    drawing.set_size_request(rect.width.max(1) as i32, rect.height.max(1) as i32);
    host_root.move_(&drawing, rect.x.max(0), rect.y.max(0));
    drawing.queue_resize();
    drawing.show_all();
    configure_native_scene_window(&drawing, Some(rect));
    Ok(())
}

#[cfg(target_os = "linux")]
const HOST_ROOT_NAME: &str = "varg-native-host-root";
#[cfg(target_os = "linux")]
const HOST_DRAWING_NAME: &str = "varg-native-host-scene-surface";
#[cfg(target_os = "linux")]
thread_local! {
    static X11_HOST_SURFACE: std::cell::RefCell<Option<X11HostSurface>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "linux")]
struct X11HostSurface {
    display: *mut x11::xlib::Display,
    parent: x11::xlib::Window,
    window: x11::xlib::Window,
    mapped: bool,
}

#[cfg(target_os = "linux")]
impl Drop for X11HostSurface {
    fn drop(&mut self) {
        if !self.display.is_null() && self.window != 0 {
            unsafe {
                x11::xlib::XDestroyWindow(self.display, self.window);
                x11::xlib::XFlush(self.display);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn ensure_x11_host_surface_on_main_thread(
    app: &tauri::AppHandle,
) -> Result<scene_window::SceneRawSurface, String> {
    use gtk::prelude::*;

    let window = app
        .get_window("main")
        .ok_or_else(|| "main editor window is not available".to_owned())?;
    let parent = match window
        .window_handle()
        .map_err(|error| format!("main window handle: {error}"))?
        .as_raw()
    {
        RawWindowHandle::Xlib(handle) => handle.window,
        RawWindowHandle::Xcb(handle) => handle.window.get() as x11::xlib::Window,
        other => {
            return Err(format!(
                "X11 native scene surface requires Xlib/Xcb main window, got {other:?}"
            ));
        }
    };
    let vbox = window
        .default_vbox()
        .map_err(|error| format!("main GTK vbox: {error}"))?;
    let vbox_widget: gtk::Widget = vbox.upcast();
    let gdk_window = vbox_widget.toplevel().and_then(|widget| widget.window());
    let gdk_window = gdk_window
        .ok_or_else(|| "main GTK toplevel did not realize a native GDK window".to_owned())?;
    if !gdk_window.ensure_native() {
        return Err("main GTK toplevel could not expose a native X11 window".to_owned());
    }
    let display = gdk_window.display();
    let xdisplay = unsafe {
        gdk_x11_sys::gdk_x11_display_get_xdisplay(
            display.as_ptr() as *mut gdk_x11_sys::GdkX11Display
        )
    };
    if xdisplay.is_null() || parent == 0 {
        return Err("GTK did not expose main X11 parent handles".to_owned());
    }

    X11_HOST_SURFACE.with(|cell| {
        let mut surface = cell.borrow_mut();
        let recreate = surface
            .as_ref()
            .is_none_or(|surface| surface.display != xdisplay || surface.parent != parent);
        if recreate {
            *surface = Some(create_x11_child_surface(xdisplay, parent)?);
        }
        let surface = surface
            .as_ref()
            .ok_or_else(|| "X11 native scene surface is missing".to_owned())?;
        Ok(scene_window::SceneRawSurface::Xlib {
            display: surface.display as usize,
            window: surface.window as u64,
        })
    })
}

#[cfg(target_os = "linux")]
fn create_x11_child_surface(
    display: *mut x11::xlib::Display,
    parent: x11::xlib::Window,
) -> Result<X11HostSurface, String> {
    let window = unsafe { x11::xlib::XCreateSimpleWindow(display, parent, 0, 0, 1, 1, 0, 0, 0) };
    if window == 0 {
        return Err("X11 could not create native scene child window".to_owned());
    }
    unsafe {
        x11::xlib::XFlush(display);
    }
    Ok(X11HostSurface {
        display,
        parent,
        window,
        mapped: false,
    })
}

#[cfg(target_os = "linux")]
fn resize_x11_host_surface_on_main_thread(rect: NativeHostSceneRect) -> Result<(), String> {
    X11_HOST_SURFACE.with(|cell| {
        let mut surface = cell.borrow_mut();
        let surface = surface
            .as_mut()
            .ok_or_else(|| "X11 native scene surface is missing".to_owned())?;
        unsafe {
            x11::xlib::XMoveResizeWindow(
                surface.display,
                surface.window,
                rect.x.max(0),
                rect.y.max(0),
                rect.width.max(1),
                rect.height.max(1),
            );
            x11::xlib::XRaiseWindow(surface.display, surface.window);
            if !surface.mapped {
                x11::xlib::XMapWindow(surface.display, surface.window);
                surface.mapped = true;
            }
            x11::xlib::XFlush(surface.display);
        }
        Ok(())
    })
}

#[cfg(target_os = "linux")]
pub fn hide_main_window_scene_surface(app: tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let result = X11_HOST_SURFACE.with(|cell| {
            let mut surface = cell.borrow_mut();
            if let Some(surface) = surface.as_mut() {
                unsafe {
                    x11::xlib::XUnmapWindow(surface.display, surface.window);
                    x11::xlib::XFlush(surface.display);
                }
                surface.mapped = false;
            }
            Ok(())
        });
        let _ = tx.send(result);
    })
    .map_err(|error| format!("schedule native host surface hide: {error}"))?;
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|error| format!("native host surface hide timed out: {error}"))?
}

#[cfg(not(target_os = "linux"))]
pub fn hide_main_window_scene_surface(_app: tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_native_host_root(vbox_widget: &gtk::Widget) -> Result<gtk::Fixed, String> {
    use gtk::prelude::*;

    if let Some(widget) = find_named_widget(vbox_widget, HOST_ROOT_NAME) {
        return widget
            .downcast::<gtk::Fixed>()
            .map_err(|_| "native host root has unexpected GTK type".to_owned());
    }

    let vbox = vbox_widget
        .clone()
        .downcast::<gtk::Box>()
        .map_err(|_| "main GTK root has unexpected type".to_owned())?;
    let children = vbox.children();
    for child in &children {
        vbox.remove(child);
    }

    let drawing = gtk::DrawingArea::new();
    drawing.set_widget_name(HOST_DRAWING_NAME);
    drawing.set_has_window(true);
    drawing.set_size_request(1, 1);
    drawing.set_hexpand(false);
    drawing.set_vexpand(false);
    drawing.set_halign(gtk::Align::Start);
    drawing.set_valign(gtk::Align::Start);
    drawing.set_no_show_all(true);

    let host_root = gtk::Fixed::new();
    host_root.set_widget_name(HOST_ROOT_NAME);
    host_root.set_hexpand(true);
    host_root.set_vexpand(true);
    host_root.set_halign(gtk::Align::Fill);
    host_root.set_valign(gtk::Align::Fill);
    host_root.put(&drawing, 0, 0);

    for child in children {
        child.set_hexpand(true);
        child.set_vexpand(true);
        child.set_halign(gtk::Align::Fill);
        child.set_valign(gtk::Align::Fill);
        host_root.put(&child, 0, 0);
    }

    let host_root_resize = host_root.clone();
    host_root.connect_size_allocate(move |root, allocation| {
        for child in root.children() {
            if child.widget_name().as_str() == HOST_DRAWING_NAME {
                continue;
            }
            child.set_size_request(allocation.width().max(1), allocation.height().max(1));
            host_root_resize.move_(&child, 0, 0);
        }
    });

    vbox.pack_start(&host_root, true, true, 0);
    host_root.show_all();
    drawing.hide();
    Ok(host_root)
}

#[cfg(target_os = "linux")]
fn configure_native_scene_window(drawing: &gtk::DrawingArea, rect: Option<NativeHostSceneRect>) {
    use gtk::prelude::*;

    if let (Some(window), Some(rect)) = (drawing.window(), rect) {
        let _ = window.ensure_native();
        window.move_resize(
            rect.x.max(0),
            rect.y.max(0),
            rect.width.max(1) as i32,
            rect.height.max(1) as i32,
        );
        let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
            0,
            0,
            rect.width.max(1) as i32,
            rect.height.max(1) as i32,
        ));
        window.set_opaque_region(Some(&region));
        window.show();
        window.restack(None, true);
        window.raise();
    }
}

#[cfg(target_os = "linux")]
fn find_named_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    use gtk::prelude::*;

    if root.widget_name().as_str() == name {
        return Some(root.clone());
    }
    let container = root.clone().downcast::<gtk::Container>().ok()?;
    for child in container.children() {
        if let Some(found) = find_named_widget(&child, name) {
            return Some(found);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn gtk_drawing_area_raw_surface(
    drawing: &gtk::DrawingArea,
    backend: NativeHostBackend,
) -> Result<scene_window::SceneRawSurface, String> {
    use gtk::prelude::*;

    let gdk_window = drawing
        .window()
        .ok_or_else(|| "GTK drawing area did not realize a native GDK window".to_owned())?;
    if !gdk_window.ensure_native() {
        return Err("GTK drawing area could not create a native surface".to_owned());
    }
    let display = gdk_window.display();

    match backend {
        NativeHostBackend::Wayland => {
            let wl_display = unsafe {
                gdk_wayland_sys::gdk_wayland_display_get_wl_display(
                    display.as_ptr() as *mut gdk_wayland_sys::GdkWaylandDisplay
                )
            };
            let wl_surface = unsafe {
                gdk_wayland_sys::gdk_wayland_window_get_wl_surface(
                    gdk_window.as_ptr() as *mut gdk_wayland_sys::GdkWaylandWindow
                )
            };
            if wl_display.is_null() || wl_surface.is_null() {
                return Err("GTK did not expose Wayland native surface handles".to_owned());
            }
            Ok(scene_window::SceneRawSurface::Wayland {
                display: wl_display as usize,
                surface: wl_surface as usize,
            })
        }
        NativeHostBackend::X11 => {
            let xdisplay = unsafe {
                gdk_x11_sys::gdk_x11_display_get_xdisplay(
                    display.as_ptr() as *mut gdk_x11_sys::GdkX11Display
                )
            };
            let xid = unsafe {
                gdk_x11_sys::gdk_x11_window_get_xid(
                    gdk_window.as_ptr() as *mut gdk_x11_sys::GdkX11Window
                )
            };
            if xdisplay.is_null() || xid == 0 {
                return Err("GTK did not expose X11 native surface handles".to_owned());
            }
            Ok(scene_window::SceneRawSurface::Xlib {
                display: xdisplay as usize,
                window: xid as u64,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_host_route_classifies_desktop_backends() {
        assert_eq!(
            native_host_route(NativeHostWindowBackend::LinuxX11),
            NativeHostRoute::LinuxHostRoot(NativeHostBackend::X11)
        );
        assert_eq!(
            native_host_route(NativeHostWindowBackend::LinuxWayland),
            NativeHostRoute::LinuxHostRoot(NativeHostBackend::Wayland)
        );
        assert_eq!(
            native_host_route(NativeHostWindowBackend::Win32),
            NativeHostRoute::WindowsDirectComposition
        );
        assert_eq!(
            native_host_route(NativeHostWindowBackend::AppKit),
            NativeHostRoute::MacosCoreAnimation
        );
    }

    #[test]
    fn native_host_route_keeps_mobile_root_surface_separate() {
        assert_eq!(
            native_host_route(NativeHostWindowBackend::MobileRootView),
            NativeHostRoute::RootWindowSurface
        );
        assert_eq!(
            native_host_route(NativeHostWindowBackend::UnsupportedDesktop),
            NativeHostRoute::UnsupportedDesktop
        );
    }

    #[test]
    fn native_host_scene_rect_sanitizes_scene_viewport_size() {
        let rect = NativeHostSceneRect::from(scene_window::SceneViewportRect {
            x: -12,
            y: 24,
            width: 0,
            height: 0,
        });

        assert_eq!(
            rect,
            NativeHostSceneRect {
                x: -12,
                y: 24,
                width: 1,
                height: 1,
            }
        );
    }

    #[test]
    fn windows_directcomposition_plan_formats_unavailable_boundaries() {
        let error = WINDOWS_DIRECTCOMPOSITION_HOST_PLAN.unavailable_error();

        assert!(error.contains("planned but not implemented"));
        assert!(error.contains("native host root: HWND-owned DirectComposition visual tree"));
        assert!(error.contains("DXGI/DirectComposition surface"));
        assert!(error.contains("WebView2 CompositionController"));
        assert!(error.contains("Blocking work:"));
        assert!(
            !WINDOWS_DIRECTCOMPOSITION_HOST_PLAN
                .platform
                .support()
                .available
        );
    }

    #[test]
    fn macos_core_animation_plan_formats_unavailable_boundaries() {
        let error = MACOS_CORE_ANIMATION_HOST_PLAN.unavailable_error();

        assert!(error.contains("planned but not implemented"));
        assert!(
            error.contains("native host root: NSWindow/NSView-owned Core Animation layer tree")
        );
        assert!(error.contains("CAMetalLayer"));
        assert!(error.contains("WKWebView/AppKit panels"));
        assert!(error.contains("Blocking work:"));
        assert!(!MACOS_CORE_ANIMATION_HOST_PLAN.platform.support().available);
    }
}
