use anyhow::anyhow;
use rand::Rng;
use vulkano::format::{Format, Pixel};
use vulkano::image::view::ImageView;
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount, StorageImage};
use vulkano::swapchain::{acquire_next_image, present};
use vulkano::sync;

use std::sync::Arc;
use std::time::Duration;
use vulkano::sync::GpuFuture;
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

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/shaders/main.comp"
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
    let compute_shader = cs::Shader::load(context.device().clone()).unwrap();

    let renderer = gpu::Renderer::new(
        &context,
        vertex_shader.main_entry_point(),
        fragment_shader.main_entry_point(),
    )?;

    let dims = context.swapchain().dimensions();

    let storage = StorageImage::new(
        context.device().clone(),
        ImageDimensions::Dim2d {
            width: dims[0] as u32,
            height: dims[1] as u32,
            array_layers: 9,
        },
        Format::R32_SFLOAT,
        [context.graphics_queue().family().clone()],
    )?;

    let compute_program = gpu::ComputeProgram::new(&context, &compute_shader.main_entry_point())?;

    let view = ImageView::new(storage.clone())?;

    let future = compute_program.compute(&[view.clone()], [2, 2, 1])?;

    let mut last_frame = Some(future);

    event_loop.run(move |event, _, flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *flow = ControlFlow::Exit,
        Event::RedrawRequested(_) => {
            last_frame = Some(Box::new(
                renderer
                    .draw(&[view.clone()], last_frame.take())
                    .unwrap()
                    .then_signal_fence_and_flush()
                    .unwrap(),
            ));
        }
        _ => {}
    });
}
