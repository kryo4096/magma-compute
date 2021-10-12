use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageCreateFlags, ImageDimensions, ImageUsage, StorageImage};

use vulkano::sync;

use std::sync::Arc;
use std::time::Instant;
use vulkano::sync::GpuFuture;
use winit::event::{ElementState, Event, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, WindowBuilder};

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
        .max_by(|mode1, mode2| mode1.size().width.cmp(&mode2.size().width))
        .unwrap();

    let window = Arc::new(
        WindowBuilder::new()
            .with_inner_size(mode.size())
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .build(&event_loop)?,
    );

    let context = gpu::Context::new(&window)?;

    let dims = context.swapchain().dimensions();

    println!(
        "GPU in use: {}\n Resolution: {}x{}, Swapchainsize: {}x{}",
        context.physical_device().properties().device_name,
        mode.size().width,
        mode.size().height,
        dims[0],
        dims[1]
    );

    let vertex_shader = vs::Shader::load(context.device()).unwrap();
    let fragment_shader = fs::Shader::load(context.device()).unwrap();
    let compute_shader = cs::Shader::load(context.device()).unwrap();

    let renderer = gpu::Renderer::new(
        &context,
        vertex_shader.main_entry_point(),
        fragment_shader.main_entry_point(),
    )?;

    let simulation_size = [dims[0], dims[1]];

    let storage_images = vec![
        StorageImage::with_usage(
            context.device(),
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
            [context.queue().family()],
        )?,
        StorageImage::with_usage(
            context.device(),
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
            [context.queue().family()],
        )?,
    ];

    let compute_program = gpu::ComputeProgram::new(&context, &compute_shader.main_entry_point())?;

    let views = storage_images
        .iter()
        .cloned()
        .map(ImageView::new)
        .collect::<Result<Vec<_>, _>>()?;

    let mut input_view = views[0].clone();
    let mut output_view = views[1].clone();

    let mut last_frame = Instant::now();

    let mut _last_frame_end = Some(sync::now(context.device()).boxed());

    let mut brightness = 1.0;

    let mut mouse_pos = [0.0, 0.0];
    let mut cursor_pos = [0.0, 0.0];
    let mut mouse_pressed = false;

    let mut compute_uniforms = cs::ty::PushConstants {
        init: 1,
        mouse_pos: [0.0, 0.0],
        mouse_delta: [0.0, 0.0],
        dissipation: 0.0,
    };

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
                    Some(VirtualKeyCode::R) => compute_uniforms.init = 1,
                    Some(VirtualKeyCode::Space) => match input.state {
                        ElementState::Pressed => compute_uniforms.dissipation = 1.0,
                        ElementState::Released => compute_uniforms.dissipation = 0.0,
                    },
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
                    let new_mouse_pos = [
                        position.x as f32 / dims[1] as f32,
                        position.y as f32 / dims[1] as f32,
                    ];

                    if mouse_pressed {
                        compute_uniforms.mouse_delta[0] = (new_mouse_pos[0] - mouse_pos[0]) * 60.;
                        compute_uniforms.mouse_delta[1] = (new_mouse_pos[1] - mouse_pos[1]) * 60.;
                    }

                    compute_uniforms.mouse_pos = new_mouse_pos;
                    mouse_pos = new_mouse_pos;
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                let compute_future = compute_program
                    .compute(
                        &[input_view.clone(), output_view.clone()],
                        [simulation_size[0] / 8 + 1, simulation_size[1] / 8 + 1, 1],
                        compute_uniforms,
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

                compute_uniforms.init = 0;

                /*if mouse_pressed {
                    compute_uniforms.mouse_delta = [
                        0.5 * (mouse_pos[0] - cursor_pos[0]),
                        0.5 * (mouse_pos[1] - cursor_pos[1]),
                    ];
                } else {
                    compute_uniforms.mouse_delta = [0., 0.];
                }*/

                compute_uniforms.mouse_delta = [0.0, 0.0];
            }
            _ => {}
        }

        window.request_redraw();
    });
}
