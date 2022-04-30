use std::{error::Error, result, sync::Arc};

use bytemuck::{Pod, Zeroable};
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;

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
    pub fn new(pos: [f32; 3], col: [f32; 4]) -> Self {
        QuadVertex {
            position: pos,
            color: col,
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

struct QuadData<T> {
    max_quads: usize,
    max_vertices: usize,
    max_indices: usize,
    vertices: Vec<T>,
    indices: Vec<u32>,
}

impl<T> QuadData<T> {
    fn new(max_quads: usize) -> Self {
        let max_vertices = max_quads * 4;
        let max_indices = max_quads * 6;
        Self {
            max_quads,
            max_vertices,
            max_indices,
            vertices: Vec::with_capacity(max_vertices),
            indices: Vec::with_capacity(max_indices),
        }
    }
}

struct BatchQuadRenderer<T> {
    vertex_shader: Arc<Shader>,
    fragment_shader: Arc<Shader>,
    data: QuadData<T>,
}

impl<T> BatchQuadRenderer<T>
where
    T: vulkano::pipeline::graphics::vertex_input::Vertex,
{
    fn new(device: Arc<Device>) -> Result<Self> {
        Ok(Self {
            vertex_shader: Shader::create(&device, ShaderType::Vertex, vs::load)?,
            fragment_shader: Shader::create(&device, ShaderType::Fragment, fs::load)?,
            data: QuadData::new(DEFAULT_MAX_QUADS),
        })
    }

    fn buffers_definition() -> BuffersDefinition {
        BuffersDefinition::vertex::<T>(BuffersDefinition::new())
    }
}
