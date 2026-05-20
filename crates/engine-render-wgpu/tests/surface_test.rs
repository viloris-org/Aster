//! Integration test: surface creation from a real winit window.

use engine_render_wgpu::WgpuRenderDevice;
use winit::window::WindowAttributes;

#[allow(deprecated)]
#[test]
fn surface_creation_succeeds_with_winit_window() {
    let mut builder = winit::event_loop::EventLoop::builder();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_any_thread(true);
    }
    let event_loop = builder
        .build()
        .expect("failed to create event loop (no display?)");
    let window = event_loop
        .create_window(
            WindowAttributes::default()
                .with_title("wgpu surface test")
                .with_inner_size(winit::dpi::PhysicalSize::new(256, 256)),
        )
        .expect("failed to create window");
    let device =
        WgpuRenderDevice::new(&window).expect("failed to create WgpuRenderDevice with surface");
    assert_eq!(device.submitted_worlds(), 0);
}
