#![allow(unused)]
use std::mem::swap;
use std::sync::Arc;

use derive_getters::Getters;
use thiserror::Error;
use vulkano::device::physical::{PhysicalDevice, QueueFamily};
use vulkano::device::{Device, DeviceCreationError, DeviceExtensions, Features, Queue};
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreationError};
use vulkano::swapchain::{Surface, SurfaceCreationError, Swapchain, SwapchainCreationError};
use vulkano::Version;
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
    #[error("Failed to create vulkan surface")]
    SurfaceCreationError(#[from] SurfaceCreationError),
    #[error("Failed to create vulkan device")]
    DeviceCreationError(#[from] DeviceCreationError),
    #[error("Failed to create swapchain")]
    SwapchainCreationError(#[from] SwapchainCreationError),
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
}

impl Context {
    pub fn new(window: &Arc<Window>) -> Result<Self, ContextCreationError> {
        window.set_resizable(false);
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));

        let instance = {
            let instance_extensions = vulkano_win::required_extensions();

            Instance::new(None, Version::V1_2, &instance_extensions, [])
                .map_err(ContextCreationError::InstanceCreationError)?
        };

        let (phys_index, physical_device) = PhysicalDevice::enumerate(&instance)
            .enumerate()
            .next()
            .ok_or(ContextCreationError::NoGPUAvailable)?;

        let surface = vulkano_win::create_vk_surface(window.clone(), instance.clone())
            .map_err(ContextCreationError::SurfaceCreationError)?;

        let queue_families = QueueFamilies::find(&physical_device, &surface)?;

        let (device, graphics_queue, compute_queue) = {
            let features = Features::none();
            let device_extensions = DeviceExtensions::none();

            let (device, mut queues) = Device::new(
                physical_device,
                &features,
                &device_extensions,
                queue_families.as_vec(),
            )
            .map_err(ContextCreationError::DeviceCreationError)?;

            (device, queues.next().unwrap(), queues.next().unwrap())
        };

        let (swapchain, swapchain_images) = Swapchain::start(device.clone(), surface.clone())
            .usage(ImageUsage::color_attachment())
            .build()
            .map_err(ContextCreationError::SwapchainCreationError)?;

        Ok(Self {
            window: window.clone(),
            instance,
            phys_index,
            device,
            graphics_queue,
            compute_queue,
            swapchain,
            swapchain_images: Arc::new(swapchain_images),
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
        let graphics_family: QueueFamily = phys_device
            .queue_families()
            .find(|qf| qf.supports_graphics() && surface.is_supported(*qf).is_ok())
            .ok_or(ContextCreationError::NoQueueFamilyFound)?;

        let compute_family = if graphics_family.supports_compute() {
            graphics_family
        } else {
            phys_device
                .queue_families()
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
