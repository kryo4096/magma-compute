use anyhow::anyhow;
use vulkano::swapchain::{acquire_next_image, present};
use vulkano::sync;

use std::sync::Arc;
use std::time::Duration;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

use crate::gpu::RendererCreationError;

mod gpu;

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/shaders/main.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/shaders/main.frag"
    }
}

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new();

    let window = Arc::new(WindowBuilder::new().build(&event_loop)?);

    let context = gpu::Context::new(&window)?;

    println!(
        "GPU in use: {}",
        context.physical_device().properties().device_name
    );

    let vertex_shader = vs::Shader::load(context.device().clone()).unwrap();
    let fragment_shader = fs::Shader::load(context.device().clone()).unwrap();

    let renderer = gpu::Renderer::new(
        &context,
        vertex_shader.main_entry_point(),
        fragment_shader.main_entry_point(),
    )?;

    event_loop.run(move |event, looop, flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *flow = ControlFlow::Exit,
        Event::RedrawRequested(id) => {
            renderer.draw().unwrap();
        }
        _ => {}
    });

    Ok(())
}
