use std::{error::Error, result, sync::Arc};

use vulkano::shader::ShaderModule;

use super::Device;

type Result<T> = result::Result<T, Box<dyn Error + 'static>>;

pub type ShaderLoadable =
    fn(
        device: Arc<vulkano::device::Device>,
    ) -> result::Result<Arc<ShaderModule>, vulkano::shader::ShaderCreationError>;

pub enum ShaderType {
    Fragment,
    Vertex,
}

pub struct Shader {
    pub typ: ShaderType,
    pub shader: Arc<ShaderModule>,
}

impl Shader {
    pub fn vertex(device: &Device, load: ShaderLoadable) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            typ: ShaderType::Vertex,
            shader: load(device.device.clone())?,
        }))
    }

    pub fn fragment(device: &Device, load: ShaderLoadable) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            typ: ShaderType::Fragment,
            shader: load(device.device.clone())?,
        }))
    }
}
