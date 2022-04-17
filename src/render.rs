use std::error::Error;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::buffer::TypedBufferAccess;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage,
    PrimaryAutoCommandBuffer, SubpassContents,
};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType, QueueFamily};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo};
use vulkano::image::view::ImageView;
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::shader::ShaderModule;
use vulkano::swapchain::{
    AcquireError, PresentFuture, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
    SwapchainCreationError,
};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture, JoinFuture};
use vulkano::{swapchain, sync};
use vulkano_win::create_surface_from_winit;
use winit::window::Window;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
    pub position: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position);

impl Vertex {
    pub fn new(pos: [f32; 2]) -> Self {
        Vertex { position: pos }
    }
}

#[allow(clippy::needless_question_mark)]
pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450

layout(location = 0) in vec2 position;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
}"
    }
}

#[allow(clippy::needless_question_mark)]
pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4(1.0, 0.0, 0.0, 1.0);
}"
    }
}

type Fences = Vec<
    Option<
        Arc<
            FenceSignalFuture<
                PresentFuture<
                    CommandBufferExecFuture<
                        JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture<Window>>,
                        Arc<PrimaryAutoCommandBuffer>,
                    >,
                    Window,
                >,
            >,
        >,
    >,
>;

#[allow(dead_code)]
pub struct Renderer {
    instance: Arc<Instance>,
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    gfx_queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    swapchain_images: Vec<Arc<SwapchainImage<Window>>>,
    render_pass: Arc<RenderPass>,
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    vertex_shader: Arc<ShaderModule>,
    fragment_shader: Arc<ShaderModule>,
    pipeline: Arc<GraphicsPipeline>,
    framebuffers: Vec<Arc<Framebuffer>>,
    viewport: Viewport,
    command_buffers: Vec<Arc<PrimaryAutoCommandBuffer>>,

    // event_loop state
    frames_in_flight: usize,
    fences: Fences,
    previous_fence_i: usize,
}

impl Renderer {
    pub fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        // -----------------------------------------------------------------------------------
        // create instance (Vulkan context)
        // -----------------------------------------------------------------------------------

        let window_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(InstanceCreateInfo {
            enabled_extensions: window_extensions,
            ..Default::default()
        })?;

        // -----------------------------------------------------------------------------------
        // create surface
        // -----------------------------------------------------------------------------------

        let surface = create_surface_from_winit(window, instance.clone())?;

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

        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo::family(queue_family)],
                enabled_extensions: physical_device
                    .required_extensions()
                    .union(&device_extensions), // new
                ..Default::default()
            },
        )?;
        let queue = queues.next().ok_or("no queue found in queue_family")?;

        // -----------------------------------------------------------------------------------
        // create swapchain
        // -----------------------------------------------------------------------------------

        let (swapchain, swapchain_images) =
            create_swapchain(&physical_device, &device, surface.clone())?;

        // -----------------------------------------------------------------------------------
        // create image views
        // -----------------------------------------------------------------------------------

        // TODO: currently done inline when creating framebuffers using swapchain_images
        //       should use iterator and create right away image views as we will never
        //       need raw images.

        // -----------------------------------------------------------------------------------
        // create render pass
        // -----------------------------------------------------------------------------------

        let render_pass = get_render_pass(device.clone(), swapchain.clone())?;

        // -----------------------------------------------------------------------------------
        // create graphics pipeline
        // -----------------------------------------------------------------------------------

        // create vertex buffer (triangle)
        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage::vertex_buffer(),
            false,
            vec![
                Vertex::new([-0.5, -0.5]),
                Vertex::new([0.0, 0.5]),
                Vertex::new([0.5, -0.25]),
            ]
            .into_iter(),
        )?;

        // load shaders
        let vertex_shader = vs::load(device.clone())?;
        let fragment_shader = fs::load(device.clone())?;

        // create actual pipeline
        let pipeline = create_graphics_pipeline(
            device.clone(),
            vertex_shader.clone(),
            fragment_shader.clone(),
            render_pass.clone(),
        )?;

        // -----------------------------------------------------------------------------------
        // create framebuffers
        // -----------------------------------------------------------------------------------

        let framebuffers = create_framebuffers(&swapchain_images, render_pass.clone())?;

        // -----------------------------------------------------------------------------------
        // create command pool
        // -----------------------------------------------------------------------------------

        // TODO: this is currently handled automatically by Vulkano when creating
        //       command buffers. It will request the default command pool from
        //       the provided device and queue family.
        //       Ref: AutoCommandBufferBuilder::primary().

        // -----------------------------------------------------------------------------------
        // create command buffers
        // -----------------------------------------------------------------------------------

        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: surface.window().inner_size().into(),
            depth_range: 0.0..1.0,
        };
        let command_buffers = create_command_buffers(
            device.clone(),
            queue.clone(),
            pipeline.clone(),
            &framebuffers,
            vertex_buffer.clone(),
            viewport.clone(),
        )?;

        // Frames in flight: executing instructions parallel to the GPU
        let frames_in_flight = swapchain_images.len();
        let fences: Fences = vec![None; frames_in_flight];
        let previous_fence_i = 0;

        let r = Renderer {
            instance,
            surface,
            device,
            gfx_queue: queue,
            swapchain,
            swapchain_images,
            render_pass,
            vertex_buffer,
            vertex_shader,
            fragment_shader,
            pipeline,
            framebuffers,
            viewport,
            command_buffers,
            frames_in_flight,
            fences,
            previous_fence_i,
        };

        Ok(r)
    }

    pub fn render(&mut self, window_resized: &mut bool, recreate_swapchain: &mut bool) {
        if *window_resized || *recreate_swapchain {
            *recreate_swapchain = false;

            // acquire new dimensions
            let new_dimensions = self.surface.window().inner_size();

            // recreate swapchain
            let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                image_extent: new_dimensions.into(),
                ..self.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
            };
            self.swapchain = new_swapchain;

            // recreate framebuffers
            let new_framebuffers =
                create_framebuffers(&new_images, self.render_pass.clone()).unwrap();

            self.framebuffers = new_framebuffers;

            if *window_resized {
                *window_resized = false;

                self.viewport.dimensions = new_dimensions.into();

                let new_pipeline = create_graphics_pipeline(
                    self.device.clone(),
                    self.vertex_shader.clone(),
                    self.fragment_shader.clone(),
                    self.render_pass.clone(),
                )
                .unwrap();

                self.pipeline = new_pipeline;

                self.command_buffers = create_command_buffers(
                    self.device.clone(),
                    self.gfx_queue.clone(),
                    self.pipeline.clone(),
                    &self.framebuffers,
                    self.vertex_buffer.clone(),
                    self.viewport.clone(),
                )
                .unwrap();
            }
        }

        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    *recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            *recreate_swapchain = true;
        }

        // wait for the fence related to this image to finish
        // normally this would be the oldest fence, that most likely have already
        // finished
        if let Some(image_fence) = &self.fences[image_i] {
            image_fence.wait(None).unwrap();
        }

        let previous_future = match self.fences[self.previous_fence_i].clone() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.device.clone());
                now.cleanup_finished();

                now.boxed()
            }
            // Use the existing FenceSignalFuture
            Some(fence) => fence.boxed(),
        };

        let future = previous_future
            .join(acquire_future)
            .then_execute(
                self.gfx_queue.clone(),
                self.command_buffers[image_i].clone(),
            )
            .unwrap()
            .then_swapchain_present(self.gfx_queue.clone(), self.swapchain.clone(), image_i)
            .then_signal_fence_and_flush();

        self.fences[image_i] = match future {
            Ok(value) => Some(Arc::new(value)),
            Err(FlushError::OutOfDate) => {
                *recreate_swapchain = true;
                None
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                None
            }
        };

        self.previous_fence_i = image_i;
    }
}

pub fn select_physical_device<'a>(
    instance: &'a Arc<Instance>,
    surface: Arc<Surface<Window>>,
    device_extensions: &DeviceExtensions,
) -> Result<(PhysicalDevice<'a>, QueueFamily<'a>), Box<dyn Error>> {
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

type SwapchainResult =
    Result<(Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>), Box<dyn Error>>;

pub fn create_swapchain<'a>(
    physical_device: &PhysicalDevice,
    device: &'a Arc<Device>,
    surface: Arc<Surface<Window>>,
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

    Ok((swapchain, images))
}

pub fn get_render_pass(
    device: Arc<Device>,
    swapchain: Arc<Swapchain<Window>>,
) -> Result<Arc<RenderPass>, Box<dyn Error>> {
    let rp = vulkano::single_pass_renderpass!(
        device,
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.image_format(),  // set the format the same as the swapchain
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {}
        }
    )?;

    Ok(rp)
}

pub fn create_graphics_pipeline(
    device: Arc<Device>,
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    render_pass: Arc<RenderPass>,
) -> Result<Arc<GraphicsPipeline>, Box<dyn Error>> {
    let p = GraphicsPipeline::start()
        .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .input_assembly_state(InputAssemblyState::new())
        .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .render_pass(Subpass::from(render_pass, 0).unwrap())
        .build(device)?;

    Ok(p)
}

pub fn create_framebuffers(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPass>,
) -> Result<Vec<Arc<Framebuffer>>, Box<dyn Error>> {
    let fbs = images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    Ok(fbs)
}

pub fn create_command_buffers(
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipeline: Arc<GraphicsPipeline>,
    framebuffers: &[Arc<Framebuffer>],
    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    viewport: Viewport,
) -> Result<Vec<Arc<PrimaryAutoCommandBuffer>>, Box<dyn Error>> {
    let fbs = framebuffers
        .iter()
        // cant get rid of unwraps when using map...
        .map(|framebuffer| {
            let mut cbbuilder = AutoCommandBufferBuilder::primary(
                device.clone(),
                queue.family(),
                // don't forget to write the correct buffer usage
                CommandBufferUsage::MultipleSubmit,
            )
            .unwrap();

            cbbuilder
                .begin_render_pass(
                    framebuffer.clone(),
                    SubpassContents::Inline,
                    vec![[0.0, 0.0, 1.0, 1.0].into()],
                )
                .unwrap()
                .bind_pipeline_graphics(pipeline.clone())
                .bind_vertex_buffers(0, vertex_buffer.clone())
                .set_viewport(0, [viewport.clone()])
                .draw(vertex_buffer.clone().len() as u32, 1, 0, 0)
                .unwrap()
                .end_render_pass()
                .unwrap();

            Arc::new(cbbuilder.build().unwrap())
        })
        .collect();

    Ok(fbs)
}
