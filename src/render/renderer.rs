use std::error::Error;
use std::ops::Mul;
use std::result;
use std::sync::Arc;

use cgmath::{Matrix4, SquareMatrix, Vector2, Vector4};
use log::error;
use vulkano::swapchain::AcquireError;
use vulkano::sync::{FlushError, GpuFuture};
use vulkano::{swapchain, sync};
use winit::event::{Event, WindowEvent};
use winit::window::Window;

use super::quad::QuadRenderPass;
use crate::render::{Device, DeviceDefinition};
use crate::TIME;

type Result<T> = result::Result<T, Box<dyn Error>>;

const DEFAULT_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

// Vulkan clip space has inverted Y and half Z.
#[rustfmt::skip]
const VULKAN_COORD_MAGIC_PROJ: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, -1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

// inspiration: https://github.com/vulkano-rs/vulkano/tree/master/examples/src/bin/interactive_fractal
pub struct Renderer2D {
    device: Device,

    background_color: [f32; 4],

    // window state
    should_recreate_swapchain: bool,

    render_pass: QuadRenderPass,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl Renderer2D {
    pub fn new(window: Arc<Window>, debug_enabled: bool) -> Result<Self> {
        let device = Device::new(DeviceDefinition::new(window).with_debug_enabled(debug_enabled))?;

        let render_pass =
            QuadRenderPass::new(device.graphics_queue(), device.swapchain.image_format());
        let previous_frame_end = Some(sync::now(device.device.clone()).boxed());

        let r = Renderer2D {
            device,
            background_color: DEFAULT_BACKGROUND_COLOR,
            should_recreate_swapchain: false,
            render_pass,
            previous_frame_end,
        };

        Ok(r)
    }

    pub fn on_event(&mut self, event: &Event<()>) {
        if let Event::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } = event
        {
            self.window_resized();
        }
    }

    pub fn set_background_color(&mut self, c: &[f32; 4]) {
        self.background_color = *c;
    }

    pub fn window_resized(&mut self) {
        self.should_recreate_swapchain = true;
    }

    pub fn begin(&mut self) -> Result<Box<dyn GpuFuture>> {
        TIME!("renderer.begin");
        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.should_recreate_swapchain {
            self.recreate_swapchain_and_views();
            self.should_recreate_swapchain = false;
        }

        // acquire next image from swapchain
        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.device.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.should_recreate_swapchain = true;
                    return Err(AcquireError::OutOfDate.into());
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.should_recreate_swapchain = true;
        }

        // set current swapchain image index
        self.device.image_index = image_i;

        // join previous frame future with acquire frame future
        let future = self.previous_frame_end.take().unwrap().join(acquire_future);
        Ok(future.boxed())
    }

    pub fn end(&mut self, after_future: Box<dyn GpuFuture>, vp: Matrix4<f32>) {
        TIME!("renderer.end");
        let model = Matrix4::identity();
        let mvp = model.mul(vp);
        // Pre-multiply mvp matrix with this magix matrix
        // to adapt to Vulkan coordinate system.
        //
        // It involves flipping Y to point downwards and moving
        // depth range from 0 <-> 1 to -1 <-> 1.
        //
        // This avoids doing it on the GPU with:
        //   account for vulkan Y pointing downwards
        //   gl_Position.y = -gl_Position.y;
        //   account for vulkan depth range being 0.0<->1.0
        //   gl_Position.z = (gl_Position.z + gl_Position.w) / 2.0;
        //
        // ref: https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
        let mvp = VULKAN_COORD_MAGIC_PROJ.mul(mvp);

        // submit graphics quads render pass (submit command buffer)
        let render_future = self.render_pass.render(
            after_future,
            self.device.image_view(),
            self.background_color,
            mvp,
        );

        // present swapchain image
        let future = render_future
            .then_swapchain_present(
                self.device.graphics_queue(),
                self.device.swapchain.clone(),
                self.device.image_index,
            )
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                // Prevent OutOfMemory error on Nvidia :(
                // https://github.com/vulkano-rs/vulkano/issues/627
                //
                // Adding the following line at the begining of begin() fixes it:
                //   self.previous_frame_end.as_mut().unwrap().cleanup_finished();
                //
                // match future.wait(None) {
                //     Ok(x) => x,
                //     Err(err) => error!("{:?}", err),
                // }
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.should_recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.device.clone()).boxed());
            }
            Err(e) => {
                error!("failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.device.clone()).boxed());
            }
        }
    }

    pub fn draw_quad(&mut self, position: Vector2<f32>, size: Vector2<f32>, color: Vector4<f32>) {
        self.render_pass.draw_quad(position, size, color)
    }

    fn recreate_swapchain_and_views(&mut self) {
        self.device.recreate_swapchain_and_views().unwrap();
    }
}
