#![allow(unused)]
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::mem::swap;
use std::sync::Arc;

use derive_getters::Getters;
use thiserror::Error;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BeginRenderPassError, BuildError, CommandBufferExecError,
    CommandBufferUsage, DrawError, ExecuteCommandsError, SecondaryAutoCommandBuffer,
    SubpassContents,
};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::persistent::PersistentDescriptorSetBuilder;
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::device::physical::{PhysicalDevice, QueueFamily};
use vulkano::device::{Device, DeviceCreationError, DeviceExtensions, Features, Queue};
use vulkano::image::view::{ImageView, ImageViewCreationError};
use vulkano::image::{
    ImageAccess, ImageLayout, ImageUsage, ImageViewAbstract, SampleCount, SwapchainImage,
};
use vulkano::instance::debug::{
    DebugCallback, DebugCallbackCreationError, MessageSeverity, MessageType,
};
use vulkano::instance::{Instance, InstanceCreationError, InstanceExtensions};
use vulkano::pipeline::layout::PipelineLayout;
use vulkano::pipeline::shader::{GraphicsEntryPoint, ShaderModule};
use vulkano::pipeline::vertex::VertexDefinition;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineCreationError};
use vulkano::render_pass::{
    self, AttachmentDesc, Framebuffer, FramebufferCreationError, LoadOp, RenderPass,
    RenderPassCreationError, RenderPassDesc, StoreOp, Subpass, SubpassDependencyDesc, SubpassDesc,
};
use vulkano::swapchain::{
    acquire_next_image, present, Surface, SurfaceCreationError, Swapchain, SwapchainCreationError,
};
use vulkano::sync::GpuFuture;
use vulkano::{OomError, Version};
use winit::dpi::PhysicalSize;
use winit::window::{Fullscreen, Window};

#[derive(Error, Debug)]
pub enum ContextCreationError {
    #[error("No device supporting vulkan found.")]
    NoGPUAvailable,
    #[error("No suitable queue families found")]
    NoQueueFamilyFound,
    #[error("Failed to create vulkan instance")]
    InstanceCreationError(#[from] InstanceCreationError),
    #[error("Failed to create debug callback")]
    DebugCallbackCreationError(#[from] DebugCallbackCreationError),
    #[error("Failed to create vulkan surface")]
    SurfaceCreationError(#[from] SurfaceCreationError),
    #[error("Failed to create vulkan device")]
    DeviceCreationError(#[from] DeviceCreationError),
    #[error("Failed to create swapchain")]
    SwapchainCreationError(#[from] SwapchainCreationError),
    #[error("Failed to create image views")]
    ImageViewCreationError(#[from] ImageViewCreationError),
}

#[derive(Clone, Getters)]
pub struct Context {
    window: Arc<Window>,
    instance: Arc<Instance>,
    #[getter(skip)]
    phys_index: usize,
    device: Arc<Device>,
    graphics_queue: Arc<Queue>,
    compute_queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Arc<Window>>>,
    swapchain_images: Arc<Vec<Arc<SwapchainImage<Arc<Window>>>>>,
    swapchain_image_views: Arc<Vec<Arc<ImageView<Arc<SwapchainImage<Arc<Window>>>>>>>,
}

impl Context {
    pub fn new(window: &Arc<Window>) -> Result<Self, ContextCreationError> {
        window.set_resizable(false);

        let instance = {
            let instance_extensions = InstanceExtensions {
                ..vulkano_win::required_extensions()
            };

            Instance::new(None, Version::V1_2, &instance_extensions, [])?
        };

        let (phys_index, physical_device) = PhysicalDevice::enumerate(&instance)
            .enumerate()
            .next()
            .ok_or(ContextCreationError::NoGPUAvailable)?;

        let surface = vulkano_win::create_vk_surface(window.clone(), instance.clone())?;

        let queue_families = QueueFamilies::find(&physical_device, &surface)?;

        let (device, graphics_queue, compute_queue) = {
            let features = Features::none();
            let device_extensions = DeviceExtensions {
                khr_swapchain: true,
                ..DeviceExtensions::none()
            };

            let (device, mut queues) = Device::new(
                physical_device,
                &features,
                &device_extensions,
                queue_families.as_vec(),
            )?;

            (device, queues.next().unwrap(), queues.next().unwrap())
        };

        let (swapchain, swapchain_images) = Swapchain::start(device.clone(), surface.clone())
            .usage(ImageUsage::color_attachment())
            .num_images(3)
            .build()?;

        let image_views = swapchain_images
            .iter()
            .map(|image| ImageView::new(image.clone()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            window: window.clone(),
            instance,
            phys_index,
            device,
            graphics_queue,
            compute_queue,
            swapchain,
            swapchain_images: Arc::new(swapchain_images),
            swapchain_image_views: Arc::new(image_views),
        })
    }

    pub fn physical_device(&self) -> PhysicalDevice {
        PhysicalDevice::from_index(&self.instance, self.phys_index).unwrap()
    }
}

struct QueueFamilies<'a> {
    graphics: QueueFamily<'a>,
    compute: QueueFamily<'a>,
}

impl<'a> QueueFamilies<'a> {
    fn find<W>(
        phys_device: &PhysicalDevice<'a>,
        surface: &Arc<Surface<W>>,
    ) -> Result<Self, ContextCreationError> {
        let graphics_family = phys_device
            .queue_families()
            .find(|qf| qf.supports_graphics() && surface.is_supported(*qf).is_ok())
            .ok_or(ContextCreationError::NoQueueFamilyFound)?;

        let compute_family =
            if graphics_family.supports_compute() && graphics_family.queues_count() > 1 {
                graphics_family
            } else {
                phys_device
                    .queue_families()
                    .filter(|qf| *qf != graphics_family)
                    .find(QueueFamily::supports_compute)
                    .ok_or(ContextCreationError::NoQueueFamilyFound)?
            };

        Ok(Self {
            graphics: graphics_family,
            compute: compute_family,
        })
    }

    fn as_vec(&self) -> Vec<(QueueFamily, f32)> {
        vec![(self.graphics, 1.0), (self.compute, 1.0)]
    }
}

#[derive(Error, Debug)]
pub enum RendererCreationError {
    #[error("Failed to create render pass.")]
    RenderPassCreationError(#[from] RenderPassCreationError),
    #[error("Failed to create graphics pipeline.")]
    PipelineCreationError(#[from] GraphicsPipelineCreationError),
}

#[derive(Error, Debug)]
pub enum RendererDrawError {
    #[error("Ran out of memory.")]
    OomError(#[from] OomError),
    #[error("Failed to create draw command.")]
    DrawError(#[from] DrawError),
    #[error("Failed to build renderer command buffer.")]
    BuildError(#[from] BuildError),
    #[error("Failed to create framebuffer.")]
    FramebufferCreationError(#[from] FramebufferCreationError),
    #[error("Failed to begin render pass.")]
    BeginRenderPassError(#[from] BeginRenderPassError),
    #[error("Failed to create secondary buffer.")]
    ExecuteCommandsError(#[from] ExecuteCommandsError),
    #[error("Failed to execute command buffer.")]
    CommandBufferExecError(#[from] CommandBufferExecError),
}

pub struct Renderer {
    context: Context,
    render_pass: Arc<RenderPass>,
    pipeline: Arc<GraphicsPipeline>,
}

impl Renderer {
    pub fn new(
        context: &Context,
        vertex_shader: GraphicsEntryPoint,
        fragment_shader: GraphicsEntryPoint,
    ) -> Result<Self, RendererCreationError> {
        let context = context.clone();

        let render_pass = Arc::new(RenderPass::new(
            context.device.clone(),
            RenderPassDesc::new(
                vec![AttachmentDesc {
                    format: context.swapchain().format(),
                    samples: SampleCount::Sample1,
                    load: LoadOp::Clear,
                    store: StoreOp::Store,
                    stencil_load: LoadOp::DontCare,
                    stencil_store: StoreOp::DontCare,
                    initial_layout: ImageLayout::Undefined,
                    final_layout: ImageLayout::PresentSrc,
                }],
                vec![SubpassDesc {
                    color_attachments: vec![(0, ImageLayout::ColorAttachmentOptimal)],
                    depth_stencil: None,
                    input_attachments: vec![],
                    resolve_attachments: vec![],
                    preserve_attachments: vec![],
                }],
                vec![],
            ),
        )?);

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_shader(vertex_shader, ())
                .triangle_list()
                .fragment_shader(fragment_shader, ())
                .viewports_dynamic_scissors_irrelevant(1)
                .depth_stencil_disabled()
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(context.device().clone())?,
        );

        Ok(Self {
            context,
            render_pass,
            pipeline,
        })
    }

    pub fn draw(&self) -> Result<(), RendererDrawError> {
        let (image_index, _, image_future) =
            acquire_next_image(self.context.swapchain().clone(), None).unwrap();

        let framebuffer = Arc::new(
            Framebuffer::start(self.render_pass.clone())
                .add(self.context.swapchain_image_views()[image_index].clone())?
                .build()?,
        );

        let dimensions = self.context.swapchain().dimensions();

        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            self.context.device.clone(),
            self.context.graphics_queue().family(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        command_buffer_builder
            .begin_render_pass(framebuffer, SubpassContents::Inline, [[1.0; 4].into()])?
            .set_viewport(
                0,
                [Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                }],
            )
            .bind_pipeline_graphics(self.pipeline.clone())
            .draw(6, 1, 0, 0)?
            .end_render_pass();

        let command_buffer = command_buffer_builder.build()?;

        let drawing_done_future =
            image_future.then_execute(self.context.graphics_queue().clone(), command_buffer)?;

        present(
            self.context.swapchain().clone(),
            drawing_done_future,
            self.context.graphics_queue().clone(),
            image_index,
        );

        Ok(())
    }
}
