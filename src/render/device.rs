use std::{collections::HashSet, error::Error, result, sync::Arc};

use log::debug;
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType, QueueFamily},
        DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo,
    },
    image::{view::ImageView, ImageUsage, ImageViewAbstract, SwapchainImage},
    instance::{
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger,
            DebugUtilsMessengerCreateInfo,
        },
        layers_list, Instance, InstanceCreateInfo, InstanceExtensions,
    },
    swapchain::{Surface, Swapchain, SwapchainCreateInfo, SwapchainCreationError},
};
use vulkano_win::create_surface_from_winit;
use winit::window::Window;

type Result<T> = result::Result<T, Box<dyn Error>>;

pub struct DeviceDefinition {
    window: Arc<Window>,
    enable_debug: bool,
}

impl DeviceDefinition {
    pub fn new(window: Arc<Window>) -> Self {
        Self {
            window,
            enable_debug: false,
        }
    }

    pub fn with_debug_enabled(mut self, b: bool) -> Self {
        self.enable_debug = b;
        self
    }
}

pub struct Device {
    pub instance: Arc<Instance>,
    pub surface: Arc<Surface<Arc<Window>>>,
    pub device: Arc<vulkano::device::Device>,
    pub queues: Vec<Arc<Queue>>,
    pub swapchain: Arc<Swapchain<Arc<Window>>>,
    pub image_views: Vec<Arc<ImageView<SwapchainImage<Arc<Window>>>>>,
    pub image_index: usize,

    // need to keep the Vulkan debug callback alive for the entier lifetime of the app
    #[allow(dead_code)]
    debug_callback: Option<DebugUtilsMessenger>,
}

impl Device {
    // the surface takes ownership of window from def
    pub fn new(def: DeviceDefinition) -> Result<Self> {
        // -----------------------------------------------------------------------------------
        // create instance (Vulkan context)
        // -----------------------------------------------------------------------------------

        let (instance, debug_callback) = create_instance(def.enable_debug)?;

        // -----------------------------------------------------------------------------------
        // create surface
        // -----------------------------------------------------------------------------------

        let surface = create_surface_from_winit(def.window, instance.clone())?;

        // -----------------------------------------------------------------------------------
        // pick physical device
        // -----------------------------------------------------------------------------------

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        let (physical_device, queue_family) =
            select_physical_device(&instance, surface.clone(), &device_extensions)?;

        // -----------------------------------------------------------------------------------
        // create logical device
        // -----------------------------------------------------------------------------------

        let (device, queues) = vulkano::device::Device::new(
            physical_device,
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
                enabled_extensions: physical_device
                    .required_extensions()
                    .union(&device_extensions), // new
                ..Default::default()
            },
        )?;
        let queues = queues.collect();

        // -----------------------------------------------------------------------------------
        // create swapchain and image views
        // -----------------------------------------------------------------------------------

        let (swapchain, image_views) =
            create_swapchain(&physical_device, &device, surface.clone())?;

        Ok(Self {
            instance,
            surface,
            device,
            queues,
            debug_callback,
            swapchain,
            image_views,
            image_index: 0,
        })
    }

    pub fn graphics_queue(&self) -> Arc<Queue> {
        self.queues[0].clone()
    }

    pub fn recreate_swapchain_and_views(&mut self) -> Result<()> {
        // recreate swapchain
        let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
            image_extent: self.surface.window().inner_size().into(),
            ..self.swapchain.create_info()
        }) {
            Ok(r) => r,
            Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return Ok(()),
            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
        };

        // this is duplicated from create_swapchain()
        let image_views = new_images
            .iter()
            .map(|img| ImageView::new_default(img.clone()).unwrap())
            .collect::<Vec<_>>();

        self.swapchain = new_swapchain;
        self.image_views = image_views;

        Ok(())
    }

    pub fn image_view(&self) -> Arc<dyn ImageViewAbstract> {
        self.image_views[self.image_index].clone()
    }
}

type InstanceResult = Result<(Arc<Instance>, Option<DebugUtilsMessenger>)>;

fn create_instance(enable_debug: bool) -> InstanceResult {
    debug!("List of Vulkan extensions supported by core:");
    for ext in format!("{:?}", InstanceExtensions::supported_by_core()?)
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_terminator(',')
        .map(|e| e.trim())
    {
        debug!("\t{}", ext);
    }

    debug!("List of Vulkan layers available to use:");
    let layer_map = layers_list()?
        .map(|l| l.name().to_owned())
        .collect::<HashSet<_>>();
    for l in layer_map.iter() {
        debug!("\t{}", l);
    }

    // extensions
    let window_extensions = vulkano_win::required_extensions();
    let extensions = InstanceExtensions {
        ext_debug_utils: enable_debug,
        ..window_extensions
    };
    debug!("List of Vulkan extensions to load:");
    for ext in format!("{:?}", extensions)
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_terminator(',')
        .map(|e| e.trim())
    {
        debug!("\t{}", ext);
    }

    // layers
    let mut layers = Vec::new();
    if enable_debug {
        let debug_validation_layer = "VK_LAYER_KHRONOS_validation".to_owned();
        if !layer_map.contains(&debug_validation_layer) {
            return Err(
                "debug validation layer requested but not supported (Did you install the Vulkan SDK?)"
                    .into(),
            );
        }
        // enable debug layer
        layers.push(debug_validation_layer);
    }

    // instance
    let instance = Instance::new(InstanceCreateInfo {
        enabled_extensions: extensions,
        enabled_layers: layers,
        ..Default::default()
    })?;

    // if debug enabled, register debug callback
    let mut callback = None;
    if enable_debug {
        callback = Some(create_debug_callback(instance.clone())?);
    }

    Ok((instance, callback))
}

type PhysicalDeviceResult<'a> = Result<(PhysicalDevice<'a>, QueueFamily<'a>)>;

fn select_physical_device<'a>(
    instance: &'a Arc<Instance>,
    surface: Arc<Surface<Arc<Window>>>,
    device_extensions: &DeviceExtensions,
) -> PhysicalDeviceResult<'a> {
    let (physical_device, queue_family) = PhysicalDevice::enumerate(instance)
        .filter(|&p| p.supported_extensions().is_superset_of(device_extensions))
        .filter_map(|p| {
            p.queue_families()
                .find(|&q| q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false))
                .map(|q| (p, q))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            PhysicalDeviceType::Other => 4,
        })
        .ok_or("no physical device found")?;

    Ok((physical_device, queue_family))
}

type SwapchainResult = Result<(
    Arc<Swapchain<Arc<Window>>>,
    Vec<Arc<ImageView<SwapchainImage<Arc<Window>>>>>,
)>;

fn create_swapchain<'a>(
    physical_device: &PhysicalDevice,
    device: &'a Arc<vulkano::device::Device>,
    surface: Arc<Surface<Arc<Window>>>,
) -> SwapchainResult {
    let device_caps = physical_device.surface_capabilities(&surface, Default::default())?;
    let dimensions = surface.window().inner_size();
    let composite_alpha = device_caps.supported_composite_alpha.iter().next().unwrap();
    let image_format = Some(physical_device.surface_formats(&surface, Default::default())?[0].0);
    let mut image_count = device_caps.min_image_count + 1;
    // cap image_count to the device max image count
    if let Some(max_image_count) = device_caps.max_image_count {
        if image_count > max_image_count {
            image_count = max_image_count;
        }
    }
    let (swapchain, images) = Swapchain::new(
        device.clone(),
        surface,
        SwapchainCreateInfo {
            // NOTE: It's good to have min_image_count be at least one more
            //       than the minimal, to give a bit more freedom to the image queue.
            min_image_count: image_count, // How many buffers to use in the swapchain
            image_format,
            image_extent: dimensions.into(),
            image_usage: ImageUsage::color_attachment(), // What the images are going to be used for
            composite_alpha,
            ..Default::default()
        },
    )?;

    let images = images
        .iter()
        .map(|img| ImageView::new_default(img.clone()).unwrap())
        .collect::<Vec<_>>();

    Ok((swapchain, images))
}

fn create_debug_callback(instance: Arc<Instance>) -> Result<DebugUtilsMessenger> {
    debug!("creating debug callback");
    let c = unsafe {
        DebugUtilsMessenger::new(
            instance,
            DebugUtilsMessengerCreateInfo {
                message_severity: DebugUtilsMessageSeverity::all(),
                message_type: DebugUtilsMessageType::all(),
                ..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
                    let ty = if msg.ty.general {
                        "general"
                    } else if msg.ty.validation {
                        "validation"
                    } else if msg.ty.performance {
                        "performance"
                    } else {
                        panic!("type no-impl");
                    };

                    let severity = if msg.severity.error {
                        "error"
                    } else if msg.severity.warning {
                        "warning"
                    } else if msg.severity.information {
                        "information"
                    } else if msg.severity.verbose {
                        "verbose"
                    } else {
                        panic!("severity no-impl");
                    };

                    debug!(
                        "[vulkan_debug][{}][{}][{}]: {}",
                        msg.layer_prefix.unwrap_or("unknown"),
                        ty,
                        severity,
                        msg.description
                    )
                }))
            },
        )?
    };

    Ok(c)
}
