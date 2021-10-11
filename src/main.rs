use anyhow::anyhow;
use rand::Rng;
use vulkano::buffer::view;
use vulkano::format::{Format, Pixel};
use vulkano::image::view::{ImageView, ImageViewType};
use vulkano::image::{
    AttachmentImage, ImageCreateFlags, ImageDimensions, ImageUsage, ImmutableImage, MipmapsCount,
    StorageImage,
};
use vulkano::swapchain::{acquire_next_image, present};
use vulkano::sync;
use winit::dpi::PhysicalSize;

use std::sync::Arc;
use std::time::{Duration, Instant};
use vulkano::sync::GpuFuture;
use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
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

    let window = Arc::new(
        WindowBuilder::new()
            .with_inner_size(PhysicalSize {
                width: 1024,
                height: 1024,
            })
            .build(&event_loop)?,
    );

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

    let storage_images = vec![
        StorageImage::with_usage(
            context.device().clone(),
            ImageDimensions::Dim2d {
                width: dims[0] as u32 * 4,
                height: dims[1] as u32 * 4,
                array_layers: 9,
            },
            Format::R32_SFLOAT,
            ImageUsage {
                sampled: true,
                storage: true,
                ..ImageUsage::none()
            },
            ImageCreateFlags::none(),
            [context.queue().family().clone()],
        )?,
        StorageImage::with_usage(
            context.device().clone(),
            ImageDimensions::Dim2d {
                width: dims[0] as u32 * 4,
                height: dims[1] as u32 * 4,
                array_layers: 9,
            },
            Format::R32_SFLOAT,
            ImageUsage {
                sampled: true,
                storage: true,
                ..ImageUsage::none()
            },
            ImageCreateFlags::none(),
            [context.queue().family().clone()],
        )?,
    ];

    let compute_program = gpu::ComputeProgram::new(&context, &compute_shader.main_entry_point())?;

    // let mut last_frame = Some(sync::now(context.device()).boxed());

    let mut layer = 0;

    let mut init = 1;

    let views = storage_images
        .iter()
        .cloned()
        .map(ImageView::new)
        .collect::<Result<Vec<_>, _>>()?;

    let mut compute_index = 0;
    let mut render_index = 1;

    let mut last_frame = Instant::now();

    event_loop.run(move |event, _, flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => match input.virtual_keycode {
                Some(VirtualKeyCode::P) if input.state == ElementState::Pressed => {
                    layer = (layer + 1) % 9
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                let compute_future = compute_program
                    .compute(
                        &[views[compute_index].clone(), views[render_index].clone()],
                        [dims[0] / 2, dims[1] / 2, 1],
                        cs::ty::PushConstants { init },
                        sync::now(context.device()).boxed(),
                    )
                    .unwrap()
                    .boxed();

                renderer
                    .draw(
                        &[views[render_index].clone()],
                        compute_future,
                        fs::ty::PushConstants { layer },
                    )
                    .unwrap()
                    .boxed();

                init = 0;

                std::mem::swap(&mut compute_index, &mut render_index);
            }
            _ => {}
        }
        if (Instant::now() - last_frame) > Duration::from_secs_f32(1. / 144.) {
            window.request_redraw();
            last_frame = Instant::now();
        }
    });
}
