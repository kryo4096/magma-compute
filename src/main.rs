use anyhow::anyhow;

use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

mod gpu;

struct Application {
    event_loop: EventLoop<()>,
    window: Window,
    context: gpu::Context,
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new();

    let window = Arc::new(WindowBuilder::new().build(&event_loop)?);

    let context = gpu::Context::new(&window)?;

    println!(
        "GPU in use: {}",
        context.physical_device().properties().device_name
    );

    context.swapchain();

    event_loop.run(|event, looop, flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *flow = ControlFlow::Exit,
        Event::RedrawRequested(id) => {}
        _ => {}
    });

    Ok(())
}
