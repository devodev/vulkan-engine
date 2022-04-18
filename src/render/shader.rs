enum ShaderType {
    Fragment,
    Vertex,
}

pub struct Shader {
    typ: ShaderType,
}

impl Shader {
    pub fn vertex() -> Self {
        Self {
            typ: ShaderType::Vertex,
        }
    }

    pub fn fragment() -> Self {
        Self {
            typ: ShaderType::Fragment,
        }
    }
}
