enum BufferType {
    Index,
    Vertex,
}

pub struct Buffer {
    typ: BufferType,
}

impl Buffer {
    pub fn vertex() -> Self {
        Self {
            typ: BufferType::Vertex,
        }
    }
}
