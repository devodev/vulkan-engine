use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::{
    buffer::{BufferUsage, ImmutableBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SecondaryAutoCommandBuffer},
    device::Queue,
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline,
    },
    render_pass::Subpass,
    sync::{self, GpuFuture},
};

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct QuadVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}
vulkano::impl_vertex!(QuadVertex, position, color);

impl QuadVertex {
    pub fn new(pos: &[f32; 3], col: &[f32; 4]) -> Self {
        QuadVertex {
            position: *pos,
            color: *col,
        }
    }
}

// NOTE: Vulkan 0.0 is top-left corner
//
// 0 +--------------+ 1
//   |              |
//   |              |
//   |              |
// 3 +--------------+ 2
//
const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 3, 0];
const QUAD_VERTICES: [[f32; 3]; 4] = [
    [-0.5, 0.5, 0.0],
    [0.5, 0.5, 0.0],
    [0.5, -0.5, 0.0],
    [-0.5, -0.5, 0.0],
];

const DEFAULT_MAX_QUADS: usize = 2000;

struct QuadBufferData {
    quads_count: usize,
    vertex_buffer: Arc<ImmutableBuffer<[QuadVertex]>>,
    index_buffer: Arc<ImmutableBuffer<[u32]>>,
    future: Box<dyn GpuFuture>,
}

pub struct QuadPipeline {
    // graphics pipeline
    gfx_queue: Arc<Queue>,
    pipeline: Arc<GraphicsPipeline>,
    // batch rendering
    max_quads: usize,
    quads_count: usize,
    vertices: Vec<QuadVertex>,
    indices: Vec<u32>,
    buffer_data: Vec<QuadBufferData>,
}

impl QuadPipeline {
    // TODO: subpass == Subpass::from(render_pass.clone(), 0).unwrap()
    pub fn new(gfx_queue: Arc<Queue>, subpass: Subpass) -> Self {
        let pipeline = {
            // compile shaders
            let vs = vs::load(gfx_queue.device().clone())
                .expect("failed to create vertex shader module");
            let fs = fs::load(gfx_queue.device().clone())
                .expect("failed to create fragment shader module");
            // create graphics pipeline
            GraphicsPipeline::start()
                .vertex_input_state(BuffersDefinition::new().vertex::<QuadVertex>())
                .vertex_shader(vs.entry_point("main").unwrap(), ())
                .input_assembly_state(InputAssemblyState::new())
                .fragment_shader(fs.entry_point("main").unwrap(), ())
                .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
                .render_pass(subpass)
                .build(gfx_queue.device().clone())
                .unwrap()
        };
        let max_quads = DEFAULT_MAX_QUADS;

        Self {
            gfx_queue,
            pipeline,
            max_quads,
            quads_count: 0,
            vertices: Vec::with_capacity(max_quads * 4),
            indices: Vec::with_capacity(max_quads * 6),
            buffer_data: Vec::new(),
        }
    }

    pub fn add_quad(&mut self, color: &[f32; 4]) {
        if self.quads_count >= self.max_quads {
            // we must flush our current vertices/indices
            self.flush_batch();
        }

        // add indices
        let offset = self.vertices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.into_iter().map(|i| i + offset));

        // add vertices
        self.vertices
            .extend(QUAD_VERTICES.iter().map(|qv| QuadVertex::new(qv, color)));

        // bump quad count
        self.quads_count += 1;
    }

    pub fn draw(
        &mut self,
        viewport_dimensions: [u32; 2],
    ) -> Option<(SecondaryAutoCommandBuffer, Box<dyn GpuFuture>)> {
        // flush remaining quads
        self.flush_batch();

        // bail out if nothing to draw
        if self.buffer_data.is_empty() {
            return None;
        }

        // create secondary command buffer
        let mut builder = AutoCommandBufferBuilder::secondary_graphics(
            self.gfx_queue.device().clone(),
            self.gfx_queue.family(),
            CommandBufferUsage::MultipleSubmit,
            self.pipeline.subpass().clone(),
        )
        .unwrap();
        builder.set_viewport(
            0,
            [Viewport {
                origin: [0.0, 0.0],
                dimensions: [viewport_dimensions[0] as f32, viewport_dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }],
        );

        let mut future = sync::now(self.gfx_queue.device().clone()).boxed();

        // record draw commands on the secondary command buffer
        if !self.buffer_data.is_empty() {
            builder.bind_pipeline_graphics(self.pipeline.clone());
        }
        for data in self.buffer_data.drain(..) {
            future = Box::new(future.join(data.future));
            builder
                .bind_vertex_buffers(0, data.vertex_buffer)
                .bind_index_buffer(data.index_buffer)
                .draw_indexed((data.quads_count * 6) as u32, 1, 0, 0, 0)
                .unwrap();
        }

        let command_buffer = builder.build().unwrap();
        let future = Box::new(future);

        Some((command_buffer, future))
    }

    fn flush_batch(&mut self) {
        if self.quads_count == 0 {
            return;
        }

        let quads_count = self.quads_count;

        // create vertex and index buffer from quad renderer
        let (vertex_buffer, vb_future) = ImmutableBuffer::from_iter(
            self.vertices.clone(),
            BufferUsage::vertex_buffer_transfer_dst(),
            self.gfx_queue.clone(),
        )
        .unwrap();
        let (index_buffer, ib_future) = ImmutableBuffer::from_iter(
            self.indices.clone(),
            BufferUsage::index_buffer_transfer_dst(),
            self.gfx_queue.clone(),
        )
        .unwrap();

        // join both futures
        let future = Box::new(vb_future.join(ib_future));

        // reset batcher values
        self.reset_batch();

        // push buffer data onto vec
        self.buffer_data.push(QuadBufferData {
            quads_count,
            vertex_buffer,
            index_buffer,
            future,
        })
    }

    fn reset_batch(&mut self) {
        self.quads_count = 0;
        self.vertices.clear();
        self.indices.clear();
    }
}

#[allow(clippy::needless_question_mark)]
pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450

// uniforms
layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} mvp;

// inputs
layout(location = 0) in vec3 position;
layout(location = 1) in vec4 color;

// outputs
layout(location = 0) out vec4 frag_Color;

void main() {
    frag_Color = color;
    gl_Position = vec4(position, 1.0);
}"
    }
}

#[allow(clippy::needless_question_mark)]
pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450

// inputs
layout(location = 0) in vec4 frag_Color;

// outputs
layout(location = 0) out vec4 f_color;

void main() {
    f_color = frag_Color;
}"
    }
}
