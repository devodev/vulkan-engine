use std::{error::Error, fmt::Debug, result, sync::Arc};

use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, Vector2, Vector4};
use vulkano::{
    buffer::{BufferUsage, CpuBufferPool, DeviceLocalBuffer, ImmutableBuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage, CopyBufferInfo,
        PrimaryAutoCommandBuffer, SecondaryAutoCommandBuffer,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::Queue,
    format::Format,
    image::{view::ImageView, ImageDimensions, ImmutableImage, MipmapsCount},
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline, Pipeline,
    },
    render_pass::Subpass,
    sampler::{Sampler, SamplerCreateInfo},
    sync::{self, GpuFuture, NowFuture},
};

use crate::TIME;

type Result<T> = result::Result<T, Box<dyn Error>>;

const WHITE: [u8; 4] = [255, 255, 255, 255];

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
struct QuadVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(QuadVertex, position, tex_coords);

impl QuadVertex {
    fn new(pos: &[f32; 2], tex_coords: &[f32; 2]) -> Self {
        QuadVertex {
            position: *pos,
            tex_coords: *tex_coords,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Zeroable, Pod)]
struct QuadVertexInstance {
    offset: [f32; 2],
    scale: [f32; 2],
    color: [f32; 4],
}
vulkano::impl_vertex!(QuadVertexInstance, offset, scale, color);

impl QuadVertexInstance {
    fn new(off: &[f32; 2], scale: &[f32; 2], col: &[f32; 4]) -> Self {
        QuadVertexInstance {
            offset: *off,
            scale: *scale,
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
const QUAD_VERTICES: [Vector2<f32>; 4] = [
    Vector2::new(-0.5, 0.5),
    Vector2::new(0.5, 0.5),
    Vector2::new(0.5, -0.5),
    Vector2::new(-0.5, -0.5),
];
const QUAD_TEX_COORDS: [Vector2<f32>; 4] = [
    Vector2::new(0.0, 0.0),
    Vector2::new(1.0, 0.0),
    Vector2::new(0.0, 1.0),
    Vector2::new(1.0, 1.0),
];

const DEFAULT_MAX_QUADS: usize = 10000;

/// contains a batch of quads data that can be used in a single draw call.
struct QuadBufferData {
    quads_count: usize,
    vertex_buffer: Arc<ImmutableBuffer<[QuadVertex]>>,
    instance_buffer: Arc<ImmutableBuffer<[QuadVertexInstance]>>,
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
    instances: Vec<QuadVertexInstance>,
    buffer_data: Vec<QuadBufferData>,
    uniform_buffer: Arc<CpuBufferPool<vs::ty::UniformBufferObject>>,
    uniform_buffer_dev: Arc<DeviceLocalBuffer<vs::ty::UniformBufferObject>>,
    uniform_descriptor_set: Arc<PersistentDescriptorSet>,
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
                .vertex_input_state(
                    BuffersDefinition::new()
                        .vertex::<QuadVertex>()
                        .instance::<QuadVertexInstance>(),
                )
                .vertex_shader(vs.entry_point("main").unwrap(), ())
                .input_assembly_state(InputAssemblyState::new())
                .fragment_shader(fs.entry_point("main").unwrap(), ())
                .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
                .render_pass(subpass)
                .build(gfx_queue.device().clone())
                .unwrap()
        };

        // TODO: textures
        //
        // 1. load image from file
        // 2. upload image data to GPU
        // 3. create sampler
        // 4. write sampler and texture to descriptor set

        // create white texture
        let (white_texture, white_texture_future) = {
            let dimensions = ImageDimensions::Dim2d {
                width: 1,
                height: 1,
                // set `array_layers: 2` even if we only have one so that we can
                // use sampler2DArray in the shader.
                //
                // TODO: using a sampler2DArray requires that all textures
                //       be of the SAME SIZE!
                //
                // Explanation: ImageView::new_default(image) calls
                // ImageViewCreateInfo::from_image(&image) and from_image() chooses
                // a viewType Dim2dArray only if array-laters > 1
                array_layers: 2,
            };
            let image_data = (0..dimensions.width() * dimensions.height() * 4).map(|_| WHITE);
            let (image, future) = ImmutableImage::from_iter(
                image_data,
                dimensions,
                MipmapsCount::One,
                Format::R8G8B8A8_SRGB,
                gfx_queue.clone(),
            )
            .expect("create white texture immutable buffer");

            let image_view = ImageView::new_default(image).unwrap();

            (image_view, future)
        };
        white_texture_future.flush().expect("white texture flush");

        let white_sampler = Sampler::new(
            gfx_queue.device().clone(),
            SamplerCreateInfo::simple_repeat_linear(),
        )
        .expect("create sampler");

        // create cpu and gpu buffers (we will copy data between them each frame)
        let uniform_buffer = Arc::new(CpuBufferPool::<vs::ty::UniformBufferObject>::new(
            gfx_queue.device().clone(),
            BufferUsage::transfer_src(),
        ));
        let uniform_buffer_dev = DeviceLocalBuffer::<vs::ty::UniformBufferObject>::new(
            gfx_queue.device().clone(),
            BufferUsage::uniform_buffer_transfer_dst(),
            [gfx_queue.family()],
        )
        .expect("create device local buffer for storing mvp uniform");

        // create descriptor set
        let layout = pipeline.layout().set_layouts().get(0).unwrap();
        let uniform_descriptor_set = PersistentDescriptorSet::new(
            layout.clone(),
            [
                WriteDescriptorSet::buffer(0, uniform_buffer_dev.clone()),
                WriteDescriptorSet::image_view_sampler(1, white_texture, white_sampler),
            ],
        )
        .expect("create descriptor set for mvp uniform buffer");

        Self {
            gfx_queue,
            pipeline,
            max_quads,
            quads_count: 0,
            vertices: Vec::with_capacity(QUAD_INDICES.len()),
            instances: Vec::with_capacity(max_quads),
            buffer_data: Vec::new(),
            uniform_buffer,
            uniform_buffer_dev,
            uniform_descriptor_set,
        }
    }

    pub fn add_quad(&mut self, position: Vector2<f32>, size: Vector2<f32>, color: Vector4<f32>) {
        if self.quads_count >= self.max_quads {
            // we must flush our current vertices/indices
            self.flush_batch();
        }

        // add instance data
        self.instances.push(QuadVertexInstance::new(
            &position.into(),
            &size.into(),
            &color.into(),
        ));

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

        // create initial future used to chain buffer data futures
        //
        // TODO: maybe we could store a `previous_future` on the pipeline
        //       used to store a future of all the init data. It could
        //       then be retrieved so that we dont need to call
        //       future.flush() in new()?
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
                self.uniform_descriptor_set.clone(),
            );

            // record draw commands
            for data in self.buffer_data.drain(..) {
                TIME!("commandbuffer record buffer_data");
                future = Box::new(future.join(data.future));

                builder
                    .bind_vertex_buffers(0, (data.vertex_buffer, data.instance_buffer))
                    .draw(QUAD_INDICES.len() as u32, data.quads_count as u32, 0, 0)
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
            QUAD_INDICES.into_iter().map(|idx| {
                QuadVertex::new(
                    &QUAD_VERTICES[idx as usize].into(),
                    &QUAD_TEX_COORDS[idx as usize].into(),
                )
            }),
            BufferUsage::vertex_buffer_transfer_dst(),
            self.gfx_queue.clone(),
        )
        .unwrap();
        let (instance_buffer, ib_future) = ImmutableBuffer::from_iter(
            self.instances.clone(),
            BufferUsage::vertex_buffer_transfer_dst(),
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
            instance_buffer,
            future,
        })
    }

    fn reset_batch(&mut self) {
        self.quads_count = 0;
        self.vertices.clear();
        self.instances.clear();
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
layout(binding = 0) uniform UniformBufferObject {
    mat4 mvp;
} ubo;

// inputs (vertex positions)
layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_coords;

// inputs (per-instance data)
layout(location = 2) in vec2 offset;
layout(location = 3) in vec2 scale;
layout(location = 4) in vec4 color;

// outputs
layout(location = 0) out vec2 f_tex_coords;
layout(location = 1) out vec4 f_frag_color;

void main() {
    f_tex_coords = tex_coords;
    f_frag_color = color;
    gl_Position = ubo.mvp * vec4(position * scale + offset, 0.0, 1.0);
}"
    }
}

#[allow(clippy::needless_question_mark)]
pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450

// uniforms
layout(binding = 1) uniform sampler2DArray tex;

// inputs
layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec4 frag_color;

// outputs
layout(location = 0) out vec4 color;

void main() {
    // color = frag_color;
    // color = vec4(tex_coords, 0.0, 1.0);
    // color = texture(white_tex[0], tex_coords) * frag_color;
    color = texture(tex, vec3(tex_coords, 0)) * frag_color;
}"
    }
}
