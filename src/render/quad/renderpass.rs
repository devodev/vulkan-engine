use std::sync::Arc;

use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SubpassContents},
    device::Queue,
    format::Format,
    image::{ImageAccess, ImageViewAbstract},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sync::GpuFuture,
};

use super::pipeline::QuadPipeline;
use crate::render::renderer::ModelViewProjection;

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
        mvp: &ModelViewProjection,
    ) -> Box<dyn GpuFuture> {
        let dimensions = image_view.clone().image().dimensions();
        let framebuffer = Framebuffer::new(
            self.render_pass.clone(),
            FramebufferCreateInfo {
                attachments: vec![image_view],
                ..Default::default()
            },
        )
        .unwrap();

        // Create primary command buffer builder
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

        let mut after_future = before_future;

        // Create secondary command buffer from texture pipeline & send draw
        // commands
        if let Some((draw_cb, buffers_future)) = self.pipeline.draw(dimensions.width_height(), mvp)
        {
            after_future = Box::new(after_future.join(buffers_future));
            // Execute above commands (subpass)
            command_buffer_builder.execute_commands(draw_cb).unwrap();
        }
        // End render pass
        command_buffer_builder.end_render_pass().unwrap();
        // Build command buffer
        let command_buffer = command_buffer_builder.build().unwrap();
        // Execute primary command buffer
        let after_future = after_future
            .then_execute(self.gfx_queue.clone(), command_buffer)
            .unwrap();

        after_future.boxed()
    }

    pub fn draw_quad(&mut self, color: &[f32; 4]) {
        self.pipeline.add_quad(color)
    }
}
