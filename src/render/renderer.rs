use std::error::Error;
use std::result;
use std::sync::Arc;

use vulkano::buffer::{BufferUsage, ImmutableBuffer, TypedBufferAccess};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage,
    PrimaryAutoCommandBuffer, SubpassContents,
};
use vulkano::swapchain::{AcquireError, PresentFuture, SwapchainAcquireFuture};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture, JoinFuture};
use vulkano::{swapchain, sync};
use winit::window::Window;

use super::quad::BatchQuadRenderer;
use crate::render::{Device, DeviceDefinition};

type Result<T> = result::Result<T, Box<dyn Error>>;

const DEFAULT_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.4, 1.0, 1.0];

type Fences = Vec<
    Option<
        Arc<
            FenceSignalFuture<
                PresentFuture<
                    CommandBufferExecFuture<
                        JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture<Arc<Window>>>,
                        Arc<PrimaryAutoCommandBuffer>,
                    >,
                    Arc<Window>,
                >,
            >,
        >,
    >,
>;

pub struct Renderer2D {
    device: Device,

    quad_renderer: BatchQuadRenderer,

    background_color: [f32; 4],

    // window state
    should_recreate_swapchain: bool,

    // event_loop state
    fences: Fences,
    previous_fence_i: usize,
}

impl Renderer2D {
    pub fn new(window: Arc<Window>, debug_enabled: bool) -> Result<Self> {
        let device = Device::new(DeviceDefinition::new(window).with_debug_enabled(debug_enabled))?;

        let frames_in_flight = device.image_views.len();
        let quad_renderer = BatchQuadRenderer::new(&device)?;

        let r = Renderer2D {
            device,
            quad_renderer,
            background_color: DEFAULT_BACKGROUND_COLOR,
            should_recreate_swapchain: false,
            fences: vec![None; frames_in_flight],
            previous_fence_i: 0,
        };

        Ok(r)
    }

    #[allow(dead_code)]
    pub fn set_background_color(&mut self, c: [f32; 4]) {
        self.background_color = c;
    }

    pub fn window_resized(&mut self) {
        self.should_recreate_swapchain = true;
    }

    pub fn begin(&self) {}

    pub fn draw_quad(&mut self, color: &[f32; 4]) {
        self.quad_renderer.add_quad(color).unwrap()
    }

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

        let previous_future = self.previous_future();

        // create command buffer
        let gfx_queue = self.device.graphics_queue();
        let mut cb_builder = AutoCommandBufferBuilder::primary(
            self.device.device.clone(),
            gfx_queue.family(),
            // since we are creating a command buffer each frame, set to one-time submit
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        if self.quad_renderer.data.count > 0 {
            // create vertex and index buffer from quad renderer
            let (vertex_buffer, vb_future) = ImmutableBuffer::from_iter(
                self.quad_renderer.data.vertices.clone(),
                BufferUsage::vertex_buffer_transfer_dst(),
                gfx_queue.clone(),
            )
            .unwrap();
            let (index_buffer, ib_future) = ImmutableBuffer::from_iter(
                self.quad_renderer.data.indices.clone(),
                BufferUsage::index_buffer_transfer_dst(),
                gfx_queue.clone(),
            )
            .unwrap();

            vb_future.join(ib_future).flush().unwrap();

            cb_builder
                .begin_render_pass(
                    self.device.framebuffers[image_i].clone(),
                    SubpassContents::Inline,
                    vec![self.background_color.into()],
                )
                .unwrap()
                .bind_pipeline_graphics(self.quad_renderer.pipeline.clone())
                .bind_vertex_buffers(0, vertex_buffer)
                .bind_index_buffer(index_buffer.clone())
                // first vertex index == firstIndex × indexSize + offset
                .draw_indexed(index_buffer.len() as u32, 1, 0, 0, 0)
                .unwrap()
                .end_render_pass()
                .unwrap();
        }

        // put command_buffer in Arc to be able to store the future in self.fences
        let command_buffer = Arc::new(cb_builder.build().unwrap());

        // synchronize previous submission with current which will execute our command
        // buffer and present the swapchain. It returns a fence future that will be
        // signaled when this has completed
        let future = previous_future
            .join(acquire_future)
            .then_execute(gfx_queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(gfx_queue, self.device.swapchain.clone(), image_i)
            .then_signal_fence_and_flush();

        // store fence future for next frame
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
        self.quad_renderer.recreate_pipeline(&self.device).unwrap();
    }

    fn previous_future(&self) -> Box<dyn GpuFuture> {
        match self.fences[self.previous_fence_i].clone() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.device.device.clone());
                now.cleanup_finished();
                now.boxed()
            }
            // Use the existing FenceSignalFuture
            Some(fence) => fence.boxed(),
        }
    }
}
