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
use winit::monitor::{self, VideoMode};

use std::sync::Arc;
use std::time::{Duration, Instant};
use vulkano::sync::GpuFuture;
use winit::event::{ElementState, Event, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

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

    let mode = event_loop
        .available_monitors()
        .next()
        .unwrap()
        .video_modes()
        .max()
        .unwrap();

    let size = mode.size();

    let window = Arc::new(
        WindowBuilder::new()
            .with_inner_size(size)
            .with_max_inner_size(size)
            .with_min_inner_size(size)
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .build(&event_loop)?,
    );

    let context = gpu::Context::new(&window)?;

    println!(
        "GPU in use: {}\n Resolution: {}x{}",
        context.physical_device().properties().device_name,
        size.width,
        size.height
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

    let simulation_size = [dims[0] / 2, dims[1] / 2];

    let storage_images = vec![
        StorageImage::with_usage(
            context.device().clone(),
            ImageDimensions::Dim2d {
                width: simulation_size[0] as u32,
                height: simulation_size[1] as u32,
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
                width: simulation_size[0] as u32,
                height: simulation_size[1] as u32,
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

    let mut layer = 0;

    let mut init = 1;

    let views = storage_images
        .iter()
        .cloned()
        .map(ImageView::new)
        .collect::<Result<Vec<_>, _>>()?;

    let mut input_view = views[0].clone();
    let mut output_view = views[1].clone();

    let mut last_frame = Instant::now();

    let mut last_frame_end = Some(sync::now(context.device()).boxed());

    let mut brightness = 1.0;

    let mut mouse_pos = [0.0, 0.0];
    let mut cursor_pos = [0.0, 0.0];
    let mut mouse_pressed = false;
    let mut mouse_delta = [0.0, 0.0];

    event_loop.run(move |event, _, flow| {
        match event {
            Event::WindowEvent {
                event: window_event,
                ..
            } => match window_event {
                WindowEvent::CloseRequested => *flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. } => match input.virtual_keycode {
                    Some(VirtualKeyCode::Period) if input.state == ElementState::Pressed => {
                        brightness *= 1.1;
                    }
                    Some(VirtualKeyCode::Comma) if input.state == ElementState::Pressed => {
                        brightness *= 0.9;
                    }
                    Some(VirtualKeyCode::Escape) => *flow = ControlFlow::Exit,
                    _ => {}
                },
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state,
                    ..
                } => {
                    if state == ElementState::Pressed {
                        if !mouse_pressed {
                            cursor_pos = mouse_pos;
                        }

                        mouse_pressed = true;
                    }

                    if state == ElementState::Released {
                        mouse_pressed = false;
                    }
                }

                WindowEvent::CursorMoved { position, .. } => {
                    mouse_pos = [
                        position.x as f32 / dims[1] as f32,
                        position.y as f32 / dims[1] as f32,
                    ];
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                let mut compute_future = compute_program
                    .compute(
                        &[input_view.clone(), output_view.clone()],
                        [simulation_size[0] / 8 + 1, simulation_size[1] / 8 + 1, 1],
                        cs::ty::PushConstants {
                            mouse_pos: cursor_pos,
                            mouse_delta,
                            init,
                        },
                        sync::now(context.device()).boxed(),
                    )
                    .unwrap()
                    .boxed();

                let render_future = renderer
                    .draw(
                        &[output_view.clone()],
                        compute_future,
                        fs::ty::PushConstants { brightness },
                    )
                    .unwrap()
                    .boxed();

                drop(render_future);

                std::mem::swap(&mut input_view, &mut &mut output_view);

                //last_frame_end = Some(render_future);

                init = 0;

                if mouse_pressed {
                    mouse_delta = [
                        0.5 * (mouse_pos[0] - cursor_pos[0]),
                        0.5 * (mouse_pos[1] - cursor_pos[1]),
                    ];
                } else {
                    mouse_delta = [0., 0.];
                }

                //cursor_pos[0] += mouse_delta[0];
                //cursor_pos[1] += mouse_delta[1];
            }
            _ => {}
        }

        window.request_redraw();

        /*
        if (Instant::now() - last_frame) > Duration::from_secs_f32(1. / 240.) {
            last_frame = Instant::now();
        }
        */
    });
}
