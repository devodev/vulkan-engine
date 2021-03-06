use std::{error::Error, result, sync::Arc};

use cgmath::{Matrix4, Vector2, Vector4};
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents,
    },
    device::Queue,
    format::Format,
    image::{ImageAccess, ImageViewAbstract},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sync::{self, GpuFuture},
};

use super::pipeline::QuadPipeline;
use crate::TIME;

type Result<T> = result::Result<T, Box<dyn Error>>;

// QuadRenderPass is responsible for creating a render pass and a graphics
// pipeline.
pub struct QuadRenderPass {
    gfx_queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    pipeline: QuadPipeline,
}

impl QuadRenderPass {
    // TODO: output_format == swapchain.image_format()
    pub fn new(gfx_queue: Arc<Queue>, output_format: Format) -> Self {
        // create render pass
        let render_pass = vulkano::single_pass_renderpass!(
            gfx_queue.device().clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: output_format,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap();

        // create pipeline
        let pipeline = QuadPipeline::new(
            gfx_queue.clone(),
            Subpass::from(render_pass.clone(), 0).unwrap(),
        );

        Self {
            gfx_queue,
            render_pass,
            pipeline,
        }
    }

    pub fn render(
        &mut self,
        before_future: Box<dyn GpuFuture>,
        image_view: Arc<dyn ImageViewAbstract>,
        clear_value: [f32; 4],
        mvp: Matrix4<f32>,
    ) -> Box<dyn GpuFuture> {
        TIME!("renderpass.render");

        // create command buffer for copying uniform data
        let uniforms_cb = self
            .pipeline
            .copy_uniforms(mvp)
            .expect("create uniform command buffer");

        // record render commands into command buffer
        let (renderpass_cb, renderpass_future) =
            self.record_command_buffer(image_view, clear_value).unwrap();

        // Execute command buffers
        let after_future = before_future
            .join(renderpass_future)
            .then_execute(self.gfx_queue.clone(), uniforms_cb)
            .unwrap()
            .then_execute(self.gfx_queue.clone(), renderpass_cb)
            .unwrap();

        after_future.boxed()
    }

    pub fn draw_quad(&mut self, position: Vector2<f32>, size: Vector2<f32>, color: Vector4<f32>) {
        self.pipeline.add_quad(position, size, color)
    }

    fn record_command_buffer(
        &mut self,
        image_view: Arc<dyn ImageViewAbstract>,
        clear_value: [f32; 4],
    ) -> Result<(PrimaryAutoCommandBuffer, Box<dyn GpuFuture>)> {
        let dimensions = image_view.clone().image().dimensions();
        let framebuffer = Framebuffer::new(
            self.render_pass.clone(),
            FramebufferCreateInfo {
                attachments: vec![image_view],
                ..Default::default()
            },
        )
        .unwrap();
        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            self.gfx_queue.device().clone(),
            self.gfx_queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Begin render pass
        command_buffer_builder
            .begin_render_pass(
                framebuffer,
                SubpassContents::SecondaryCommandBuffers,
                vec![clear_value.into()],
            )
            .unwrap();

        // Create secondary command buffer from texture pipeline & send draw
        // commands
        let mut future = sync::now(self.gfx_queue.device().clone()).boxed();
        if let Some((draw_cb, buffers_future)) = self.pipeline.draw(dimensions.width_height()) {
            future = Box::new(future.join(buffers_future));
            // Execute above commands (subpass)
            command_buffer_builder.execute_commands(draw_cb).unwrap();
        }
        // End render pass
        command_buffer_builder.end_render_pass().unwrap();
        // Build command buffer
        let command_buffer = command_buffer_builder.build()?;

        Ok((command_buffer, future))
    }
}
