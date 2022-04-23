use std::error::Error;
use std::result;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::buffer::TypedBufferAccess;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage,
    PrimaryAutoCommandBuffer, SubpassContents,
};
use vulkano::device::Queue;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{Framebuffer, RenderPass, Subpass};
use vulkano::shader::ShaderModule;
use vulkano::swapchain::{AcquireError, PresentFuture, SwapchainAcquireFuture};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture, JoinFuture};
use vulkano::{swapchain, sync};
use winit::window::Window;

use super::buffer::{Buffer, BufferType};
use super::shader::{Shader, ShaderType};
use crate::render::{Device, DeviceDefinition};

type Result<T> = result::Result<T, Box<dyn Error>>;

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
    device: Device,

    // triangle graphics context
    vertex_buffer: Arc<Buffer<[Vertex]>>,
    vertex_shader: Arc<Shader>,
    fragment_shader: Arc<Shader>,
    pipeline: Arc<GraphicsPipeline>,

    // window state
    should_recreate_swapchain: bool,

    // event_loop state
    frames_in_flight: usize,
    fences: Fences,
    previous_fence_i: usize,
}

impl Renderer {
    pub fn new(window: Window, debug_enabled: bool) -> Result<Self> {
        let device = Device::new(DeviceDefinition::new(window).with_debug_enabled(debug_enabled))?;

        // -----------------------------------------------------------------------------------
        // create graphics pipeline
        // -----------------------------------------------------------------------------------

        // create vertex buffer (triangle)
        let vertex_buffer = Buffer::create(
            &device,
            BufferType::Vertex,
            vec![
                Vertex::new([-0.5, -0.5]),
                Vertex::new([0.0, 0.5]),
                Vertex::new([0.5, -0.25]),
            ]
            .into_iter(),
        )?;

        // load shaders
        let vertex_shader = Shader::create(&device, ShaderType::Vertex, vs::load)?;
        let fragment_shader = Shader::create(&device, ShaderType::Fragment, fs::load)?;

        // create actual pipeline
        let pipeline = create_graphics_pipeline(
            device.device.clone(),
            vertex_shader.shader.clone(),
            fragment_shader.shader.clone(),
            device.render_pass.clone(),
            device.dimensions(),
        )
        .unwrap();

        // -----------------------------------------------------------------------------------
        // create command pool
        // -----------------------------------------------------------------------------------

        // NOTE: this is currently handled automatically by Vulkano when creating
        //       command buffers. It will request the default command pool from
        //       the provided device and queue family.
        //       Ref: AutoCommandBufferBuilder::primary().

        // -----------------------------------------------------------------------------------
        // create command buffers
        // -----------------------------------------------------------------------------------

        // NOTE: Created every frame in the event loop
        //

        let frames_in_flight = device.image_views.len();
        let r = Renderer {
            device,
            vertex_buffer,
            vertex_shader,
            fragment_shader,
            pipeline,
            should_recreate_swapchain: false,
            frames_in_flight,
            fences: vec![None; frames_in_flight],
            previous_fence_i: 0,
        };

        Ok(r)
    }

    pub fn window_resized(&mut self) {
        self.should_recreate_swapchain = true;
    }

    pub fn begin(&self) {}

    pub fn end(&mut self) {
        if self.should_recreate_swapchain {
            self.recreate_swapchain();
            self.should_recreate_swapchain = false;
        }

        // acquire next image from swapchain
        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.device.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.should_recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.should_recreate_swapchain = true;
        }

        // wait for the fence related to the acquired image to finish
        // normally this would be the oldest fence, that most likely have already
        // finished
        if let Some(image_fence) = &self.fences[image_i] {
            image_fence.wait(None).unwrap();
        }

        let previous_future = match self.fences[self.previous_fence_i].clone() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.device.device.clone());
                now.cleanup_finished();

                now.boxed()
            }
            // Use the existing FenceSignalFuture
            Some(fence) => fence.boxed(),
        };

        let gfx_queue = self.device.queues[0].clone();

        let command_buffer = create_command_buffer(
            self.device.device.clone(),
            gfx_queue.clone(),
            self.pipeline.clone(),
            self.device.framebuffers[image_i].clone(),
            self.vertex_buffer.clone(),
        )
        .unwrap();

        let future = previous_future
            .join(acquire_future)
            .then_execute(gfx_queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(gfx_queue, self.device.swapchain.clone(), image_i)
            .then_signal_fence_and_flush();

        self.fences[image_i] = match future {
            Ok(value) => Some(Arc::new(value)),
            Err(FlushError::OutOfDate) => {
                self.should_recreate_swapchain = true;
                None
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                None
            }
        };

        self.previous_fence_i = image_i;
    }

    fn recreate_swapchain(&mut self) {
        self.device.recreate_swapchain().unwrap();
        self.pipeline = create_graphics_pipeline(
            self.device.device.clone(),
            self.vertex_shader.shader.clone(),
            self.fragment_shader.shader.clone(),
            self.device.render_pass.clone(),
            self.device.dimensions(),
        )
        .unwrap();
    }
}

type GraphicsPipelineResult = Result<Arc<GraphicsPipeline>>;

fn create_graphics_pipeline(
    device: Arc<vulkano::device::Device>,
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    render_pass: Arc<RenderPass>,
    dimensions: [u32; 2],
) -> GraphicsPipelineResult {
    let p = GraphicsPipeline::start()
        // define states
        .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
        .input_assembly_state(InputAssemblyState::new())
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([
            Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0..1.0,
            },
        ]))
        // define shaders
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        // define render pass
        .render_pass(Subpass::from(render_pass, 0).unwrap())
        .build(device)?;

    Ok(p)
}

type CommandbufferResult = Result<Arc<PrimaryAutoCommandBuffer>>;

// create a command buffer for each framebuffer
fn create_command_buffer(
    device: Arc<vulkano::device::Device>,
    queue: Arc<Queue>,
    pipeline: Arc<GraphicsPipeline>,
    framebuffer: Arc<Framebuffer>,
    vertex_buffer: Arc<Buffer<[Vertex]>>,
) -> CommandbufferResult {
    let clear_value = [0.0, 0.0, 1.0, 1.0];
    let mut cbbuilder = AutoCommandBufferBuilder::primary(
        device,
        queue.family(),
        // don't forget to write the correct buffer usage
        CommandBufferUsage::OneTimeSubmit,
    )?;

    cbbuilder
        .begin_render_pass(
            framebuffer,
            SubpassContents::Inline,
            vec![clear_value.into()],
        )
        .unwrap()
        .bind_pipeline_graphics(pipeline)
        .bind_vertex_buffers(0, vertex_buffer.buffer.clone())
        .draw(vertex_buffer.buffer.clone().len() as u32, 1, 0, 0)?
        .end_render_pass()?;

    let fb = cbbuilder.build()?;
    Ok(Arc::new(fb))
}
