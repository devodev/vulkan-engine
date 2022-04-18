pub enum PipelineType {
    Graphics,
}

pub struct Pipeline {
    typ: PipelineType,
}

impl Pipeline {
    pub fn graphics() -> Self {
        Self {
            typ: PipelineType::Graphics,
        }
    }
}
