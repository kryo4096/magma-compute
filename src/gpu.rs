#![allow(unused)]

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::mem::swap;
use std::sync::Arc;

use derive_getters::Getters;
use thiserror::Error;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BeginRenderPassError, BuildError, CommandBufferExecError,
    CommandBufferUsage, DispatchError, DrawError, ExecuteCommandsError, SecondaryAutoCommandBuffer,
    SubpassContents,
};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::persistent::PersistentDescriptorSetBuilder;
use vulkano::descriptor_set::{DescriptorSetError, PersistentDescriptorSet};
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
use vulkano::pipeline::shader::{
    ComputeEntryPoint, EntryPointAbstract, GraphicsEntryPoint, ShaderModule,
    SpecializationConstants,
};
use vulkano::pipeline::vertex::VertexDefinition;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{
    ComputePipeline, ComputePipelineCreationError, GraphicsPipeline, GraphicsPipelineCreationError,
    PipelineBindPoint,
};
use vulkano::render_pass::{
    self, AttachmentDesc, Framebuffer, FramebufferAbstract, FramebufferCreationError, LoadOp,
    RenderPass, RenderPassCreationError, RenderPassDesc, StoreOp, Subpass, SubpassDependencyDesc,
    SubpassDesc,
};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode, SamplerCreationError};
use vulkano::swapchain::{
    acquire_next_image, present, PresentMode, Surface, SurfaceCreationError, Swapchain,
    SwapchainCreationError,
};
use vulkano::sync::{self, GpuFuture};
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

#[derive(Clone)]
pub struct Context {
    window: Arc<Window>,
    instance: Arc<Instance>,
    phys_index: usize,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Arc<Window>>>,
    swapchain_images: Arc<Vec<Arc<SwapchainImage<Arc<Window>>>>>,
    swapchain_image_views: Arc<Vec<Arc<ImageView<Arc<SwapchainImage<Arc<Window>>>>>>>,
}

impl Context {
    pub fn new(window: &Arc<Window>) -> Result<Self, ContextCreationError> {
        window.set_resizable(false);

        let instance = {
            let instance_extensions = InstanceExtensions {
                khr_get_display_properties2: false,
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

        let (device, queue) = {
            let features = Features {
                shader_storage_image_extended_formats: true,
                ..Features::none()
            };

            let device_extensions = DeviceExtensions {
                khr_swapchain: true,
                khr_storage_buffer_storage_class: true,
                ..DeviceExtensions::none()
            };

            let (device, mut queues) = Device::new(
                physical_device,
                &features,
                &device_extensions,
                queue_families.as_vec(),
            )?;

            let queue = queues.next().unwrap();

            (device, queue.clone())
        };

        let (swapchain, swapchain_images) = Swapchain::start(device.clone(), surface.clone())
            .usage(ImageUsage::color_attachment())
            .num_images(3)
            .present_mode(PresentMode::Relaxed)
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
            queue,
            swapchain,
            swapchain_images: Arc::new(swapchain_images),
            swapchain_image_views: Arc::new(image_views),
        })
    }

    pub fn window(&self) -> Arc<Window> {
        self.window.clone()
    }

    pub fn instance(&self) -> Arc<Instance> {
        self.instance.clone()
    }

    pub fn physical_device(&self) -> PhysicalDevice {
        PhysicalDevice::from_index(&self.instance, self.phys_index).unwrap()
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }

    pub fn swapchain(&self) -> Arc<Swapchain<Arc<Window>>> {
        self.swapchain.clone()
    }

    pub fn swapchain_images(&self) -> Arc<Vec<Arc<SwapchainImage<Arc<Window>>>>> {
        self.swapchain_images.clone()
    }

    pub fn swapchain_image_views(
        &self,
    ) -> Arc<Vec<Arc<ImageView<Arc<SwapchainImage<Arc<Window>>>>>>> {
        self.swapchain_image_views.clone()
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

        let compute_family = graphics_family;

        Ok(Self {
            graphics: graphics_family,
            compute: compute_family,
        })
    }

    fn as_vec(&self) -> Vec<(QueueFamily, f32)> {
        vec![(self.graphics, 1.0)]
    }
}

#[derive(Error, Debug)]
pub enum RendererCreationError {
    #[error("Failed to create render pass.")]
    RenderPassCreationError(#[from] RenderPassCreationError),
    #[error("Failed to create graphics pipeline.")]
    PipelineCreationError(#[from] GraphicsPipelineCreationError),
    #[error("Failed to create sampler.")]
    SamplerCreationError(#[from] SamplerCreationError),
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
    #[error("Failed to execute command buffer.")]
    CommandBufferExecError(#[from] CommandBufferExecError),
    #[error("Failed to create descriptor set.")]
    DescriptorSetError(#[from] DescriptorSetError),
}

pub struct Renderer {
    context: Context,
    render_pass: Arc<RenderPass>,
    pipeline: Arc<GraphicsPipeline>,
    sampler: Arc<Sampler>,
}

impl Renderer {
    pub fn new(
        context: &Context,
        vertex_shader: GraphicsEntryPoint,
        fragment_shader: GraphicsEntryPoint,
    ) -> Result<Self, RendererCreationError> {
        let context = context.clone();

        let render_pass = Arc::new(RenderPass::new(
            context.device(),
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
                .build(context.device())?,
        );

        let sampler = Sampler::new(
            context.device(),
            Filter::Nearest,
            Filter::Nearest,
            MipmapMode::Nearest,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            0.0,
            1.0,
            0.0,
            1.0,
        )?;

        Ok(Self {
            context,
            render_pass,
            pipeline,
            sampler,
        })
    }

    pub fn draw<Pc>(
        &self,
        input_images: &[Arc<dyn ImageViewAbstract>],
        before: Box<dyn GpuFuture>,
        push_constants: Pc,
    ) -> Result<Box<dyn GpuFuture>, RendererDrawError> {
        let (image_index, _, image_future) =
            acquire_next_image(self.context.swapchain(), None).unwrap();

        let framebuffer = Arc::new(
            Framebuffer::start(self.render_pass.clone())
                .add(self.context.swapchain_image_views()[image_index].clone())?
                .build()?,
        );

        let dimensions = self.context.swapchain().dimensions();

        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            self.context.device(),
            self.context.queue().family(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let mut descriptor_set_builder = PersistentDescriptorSet::start(
            self.pipeline.layout().descriptor_set_layouts()[0].clone(),
        );

        for image in input_images {
            descriptor_set_builder.add_sampled_image(image.clone(), self.sampler.clone())?;
        }

        let set = Arc::new(descriptor_set_builder.build()?);

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
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                set.clone(),
            )
            .push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .draw(6, 1, 0, 0)?
            .end_render_pass();

        let command_buffer = command_buffer_builder.build()?;

        let future = image_future
            .join(before)
            .then_execute(self.context.queue(), command_buffer)?
            .then_swapchain_present(self.context.queue(), self.context.swapchain(), image_index)
            .then_signal_fence_and_flush()
            .unwrap();

        Ok(future.boxed())
    }
}

#[derive(Error, Debug)]
pub enum ComputeProgramCreationError {
    #[error("Failed to create compute pipeline.")]
    ComputePipelineCreationError(#[from] ComputePipelineCreationError),
}

#[derive(Error, Debug)]
pub enum ComputeError {
    #[error("Shader doesn't have an image descriptor.")]
    NoImageDescriptor,
    #[error("Failed to execute command buffer.")]
    CommandBufferExecError(#[from] CommandBufferExecError),
    #[error("Failed to create descriptor set.")]
    DescriptorSetError(#[from] DescriptorSetError),
    #[error("Ran out of memory.")]
    OomError(#[from] OomError),
    #[error("Failed to dispatch compute shader.")]
    DispatchError(#[from] DispatchError),
    #[error("Failed to build renderer command buffer.")]
    BuildError(#[from] BuildError),
}

pub struct ComputeProgram {
    context: Context,
    pipeline: Arc<ComputePipeline>,
}

impl ComputeProgram {
    pub fn new(
        context: &Context,
        shader: &ComputeEntryPoint,
    ) -> Result<Self, ComputeProgramCreationError> {
        let pipeline = Arc::new(ComputePipeline::new(
            context.device(),
            shader,
            &(),
            None,
            |_| {},
        )?);

        Ok(Self {
            context: context.clone(),
            pipeline,
        })
    }

    pub fn compute<Pc>(
        &self,
        images: &[Arc<dyn ImageViewAbstract>],
        dispatch_dimensions: [u32; 3],
        push_constants: Pc,
        before: Box<dyn GpuFuture>,
    ) -> Result<Box<dyn GpuFuture>, ComputeError> {
        let mut set_builder = PersistentDescriptorSet::start(
            self.pipeline
                .layout()
                .descriptor_set_layouts()
                .get(0)
                .ok_or(ComputeError::NoImageDescriptor)?
                .clone(),
        );

        for image in images {
            set_builder.add_image(image.clone());
        }

        let set = set_builder.build()?;

        let mut builder = AutoCommandBufferBuilder::primary(
            self.context.device(),
            self.context.queue().family(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        builder
            .bind_pipeline_compute(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipeline.layout().clone(),
                0,
                set,
            )
            .push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .dispatch(dispatch_dimensions)?;

        let commands = builder.build()?;

        let future = before.then_execute(self.context.queue(), commands)?;

        Ok(future.boxed())
    }
}
