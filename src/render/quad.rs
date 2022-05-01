use std::{error::Error, result, sync::Arc};

use bytemuck::{Pod, Zeroable};
use vulkano::{
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline,
    },
    render_pass::Subpass,
    shader::ShaderModule,
};

use super::{
    shader::{Shader, ShaderType},
    Device,
};

type Result<T> = result::Result<T, Box<dyn Error>>;

#[allow(clippy::needless_question_mark)]
pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450

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
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
];

const DEFAULT_MAX_QUADS: usize = 2000;

pub struct QuadData {
    max_quads: usize,
    max_vertices: usize,
    max_indices: usize,
    pub count: usize,
    pub vertices: Vec<QuadVertex>,
    pub indices: Vec<u32>,
}

impl QuadData {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, color: &[f32; 4]) -> Result<()> {
        if self.count >= self.max_quads {
            return Err(format!("quads limit exceeded: {}", self.max_quads).into());
        }

        // add indices
        let offset = self.vertices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.into_iter().map(|i| i + offset));

        // add vertices
        self.vertices
            .extend(QUAD_VERTICES.iter().map(|qv| QuadVertex::new(qv, color)));

        // bump quad count
        self.count += 1;

        Ok(())
    }
}

impl Default for QuadData {
    fn default() -> Self {
        let max_quads = DEFAULT_MAX_QUADS;
        let max_vertices = max_quads * 4;
        let max_indices = max_quads * 6;
        Self {
            count: 0,
            max_quads,
            max_vertices,
            max_indices,
            vertices: Vec::with_capacity(max_vertices),
            indices: Vec::with_capacity(max_indices),
        }
    }
}

pub struct BatchQuadRenderer {
    pub vertex_shader: Arc<Shader>,
    pub fragment_shader: Arc<Shader>,
    pub pipeline: Arc<GraphicsPipeline>,
    pub data: QuadData,
}

impl BatchQuadRenderer {
    pub fn new(device: &Device) -> Result<Self> {
        let vertex_shader = Shader::create(device, ShaderType::Vertex, vs::load)?;
        let fragment_shader = Shader::create(device, ShaderType::Fragment, fs::load)?;
        let pipeline = create_graphics_pipeline(
            device,
            vertex_shader.shader.clone(),
            fragment_shader.shader.clone(),
        )?;
        Ok(Self {
            vertex_shader,
            fragment_shader,
            pipeline,
            data: QuadData::new(),
        })
    }

    pub fn buffers_definition() -> BuffersDefinition {
        BuffersDefinition::vertex::<QuadVertex>(BuffersDefinition::new())
    }

    pub fn add_quad(&mut self, color: &[f32; 4]) -> Result<()> {
        self.data.add(color)
    }

    pub fn recreate_pipeline(&mut self, device: &Device) -> Result<()> {
        self.pipeline = create_graphics_pipeline(
            device,
            self.vertex_shader.shader.clone(),
            self.fragment_shader.shader.clone(),
        )?;
        Ok(())
    }
}

fn create_graphics_pipeline(
    device: &Device,
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
) -> Result<Arc<GraphicsPipeline>> {
    let dimensions = device.dimensions();
    let p = GraphicsPipeline::start()
        // defines how to handle vertex data provided by our vertex buffer (data layout)
        .vertex_input_state(BuffersDefinition::new().vertex::<QuadVertex>())
        // defines how the device should assemble primitives (vertices and instances)
        // default is TRIANGLE_LIST
        .input_assembly_state(InputAssemblyState::new())
        // defines the region of the framebuffer that the output will be rendered to
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
        .render_pass(Subpass::from(device.render_pass.clone(), 0).unwrap())
        .build(device.device.clone())?;

    Ok(p)
}
