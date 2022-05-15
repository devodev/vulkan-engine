use std::{error::Error, fmt::Debug, ops::Mul, result, sync::Arc};

use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, Vector2, Vector3, Vector4};
use vulkano::{
    buffer::{BufferUsage, CpuBufferPool, DeviceLocalBuffer, ImmutableBuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryAutoCommandBuffer,
        SecondaryAutoCommandBuffer,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::Queue,
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline,
    },
    render_pass::Subpass,
    sync::{self, GpuFuture},
};

use crate::TIME;

type Result<T> = result::Result<T, Box<dyn Error>>;

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
const QUAD_VERTICES: [Vector3<f32>; 4] = [
    Vector3::new(-0.5, 0.5, 0.0),
    Vector3::new(0.5, 0.5, 0.0),
    Vector3::new(0.5, -0.5, 0.0),
    Vector3::new(-0.5, -0.5, 0.0),
];

const DEFAULT_MAX_QUADS: usize = 2000;

struct QuadBufferData {
    quads_count: usize,
    vertex_buffer: Arc<ImmutableBuffer<[QuadVertex]>>,
    index_buffer: Arc<ImmutableBuffer<[u32]>>,
    future: Box<dyn GpuFuture>,
}

impl Debug for QuadBufferData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuadBufferData{{quads_count: {}}}", self.quads_count)
    }
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
    uniform_buffer: Arc<CpuBufferPool<vs::ty::UniformBufferObject>>,
    uniform_buffer_dev: Arc<DeviceLocalBuffer<vs::ty::UniformBufferObject>>,
    uniform_mvp_ds: Arc<PersistentDescriptorSet>,
}

impl QuadPipeline {
    // TODO: subpass == Subpass::from(render_pass.clone(), 0).unwrap()
    pub fn new(gfx_queue: Arc<Queue>, subpass: Subpass) -> Self {
        let max_quads = DEFAULT_MAX_QUADS;
        // graphics pipeline
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

        // create cpu and gpu buffers (we will copy data between them each frame)
        let uniform_mvp_buffer = Arc::new(CpuBufferPool::<vs::ty::UniformBufferObject>::new(
            gfx_queue.device().clone(),
            BufferUsage::transfer_src(),
        ));
        let uniform_mvp_buffer_dev = DeviceLocalBuffer::<vs::ty::UniformBufferObject>::new(
            gfx_queue.device().clone(),
            BufferUsage::uniform_buffer_transfer_dst(),
            [gfx_queue.family()],
        )
        .expect("create device local buffer for storing mvp uniform");

        // create descriptor set
        let layout = pipeline.layout().set_layouts().get(0).unwrap();
        let uniform_mvp_ds = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(
                0,
                uniform_mvp_buffer_dev.clone(),
            )],
        )
        .expect("create descriptor set for mvp uniform buffer");

        Self {
            gfx_queue,
            pipeline,
            max_quads,
            quads_count: 0,
            vertices: Vec::with_capacity(max_quads * 4),
            indices: Vec::with_capacity(max_quads * 6),
            buffer_data: Vec::new(),
            uniform_buffer: uniform_mvp_buffer,
            uniform_buffer_dev: uniform_mvp_buffer_dev,
            uniform_mvp_ds,
        }
    }

    pub fn add_quad(&mut self, position: Vector2<f32>, size: Vector2<f32>, color: Vector4<f32>) {
        if self.quads_count >= self.max_quads {
            // we must flush our current vertices/indices
            self.flush_batch();
        }

        // add indices
        let offset = self.vertices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.into_iter().map(|i| i + offset));

        // add vertices
        let translation = Matrix4::from_translation(Vector3::new(position.x, position.y, 1.0));
        let scale = Matrix4::from_nonuniform_scale(size.x, size.y, 1.0);
        self.vertices.extend(QUAD_VERTICES.iter().map(|qv| {
            QuadVertex::new(
                &scale
                    .mul(translation.mul(Vector4::new(qv.x, qv.y, qv.z, 1.0)))
                    .truncate()
                    .into(),
                &color.into(),
            )
        }));

        // bump quad count
        self.quads_count += 1;
    }

    pub fn draw(
        &mut self,
        viewport_dimensions: [u32; 2],
    ) -> Option<(SecondaryAutoCommandBuffer, Box<dyn GpuFuture>)> {
        TIME!("pipeline.draw");
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
            CommandBufferUsage::OneTimeSubmit,
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
            // bind pipeline
            builder.bind_pipeline_graphics(self.pipeline.clone());

            // bind uniform buffer descriptor set
            builder.bind_descriptor_sets(
                vulkano::pipeline::PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                self.uniform_mvp_ds.clone(),
            );

            // record draw commands
            for data in self.buffer_data.drain(..) {
                future = Box::new(future.join(data.future));

                builder
                    .bind_vertex_buffers(0, data.vertex_buffer)
                    .bind_index_buffer(data.index_buffer)
                    .draw_indexed((data.quads_count * QUAD_INDICES.len()) as u32, 1, 0, 0, 0)
                    .unwrap();
            }
        }

        let command_buffer = builder.build().unwrap();
        let future = Box::new(future);

        Some((command_buffer, future))
    }

    pub fn copy_uniforms(&mut self, mvp: Matrix4<f32>) -> Result<PrimaryAutoCommandBuffer> {
        let subbuffer = self
            .uniform_buffer
            .next(vs::ty::UniformBufferObject { mvp: mvp.into() })?;

        let mut cbb = AutoCommandBufferBuilder::primary(
            self.gfx_queue.device().clone(),
            self.gfx_queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        cbb.copy_buffer(CopyBufferInfo::buffers(
            subbuffer,
            self.uniform_buffer_dev.clone(),
        ))?;
        let cb = cbb.build()?;

        Ok(cb)
    }

    fn flush_batch(&mut self) {
        TIME!("pipeline.flush_batch");
        if self.quads_count == 0 {
            return;
        }

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

        // save quads_count before reset
        let quads_count = self.quads_count;

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
        types_meta: {
            use bytemuck::{Pod, Zeroable};

            #[derive(Clone, Copy, Zeroable, Pod)]
        },
        src: "
#version 450

// uniforms
layout(binding = 0) uniform UniformBufferObject  {
    mat4 mvp;
} ubo;

// inputs
layout(location = 0) in vec3 position;
layout(location = 1) in vec4 color;

// outputs
layout(location = 0) out vec4 frag_Color;

void main() {
    frag_Color = color;
    gl_Position = ubo.mvp * vec4(position, 1.0);
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
