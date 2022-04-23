use std::{error::Error, result, sync::Arc};

use vulkano::shader::ShaderModule;

use super::Device;

type Result<T> = result::Result<T, Box<dyn Error + 'static>>;

pub type ShaderLoadable =
    fn(
        device: Arc<vulkano::device::Device>,
    ) -> result::Result<Arc<ShaderModule>, vulkano::shader::ShaderCreationError>;

#[derive(Debug, Clone, Copy)]
pub enum ShaderType {
    Fragment,
    Vertex,
}

#[derive(Debug, Clone)]
pub struct Shader {
    pub typ: ShaderType,
    pub shader: Arc<ShaderModule>,
}

impl Shader {
    pub fn create(device: &Device, typ: ShaderType, load: ShaderLoadable) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            typ,
            shader: load(device.device.clone())?,
        }))
    }
}
