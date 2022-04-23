use std::{error::Error, result, sync::Arc};

use vulkano::buffer::{BufferContents, BufferUsage, CpuAccessibleBuffer};

use super::Device;

#[allow(dead_code)]
type Result<T> = result::Result<T, Box<dyn Error>>;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum BufferType {
    Index,
    Vertex,
}

#[derive(Debug, Clone)]
pub struct Buffer<T>
where
    T: BufferContents + ?Sized,
{
    pub typ: BufferType,
    pub buffer: Arc<CpuAccessibleBuffer<T>>,
}

impl<T> Buffer<[T]>
where
    [T]: BufferContents,
{
    #[allow(dead_code)]
    pub fn create<I>(device: &Device, typ: BufferType, data: I) -> Result<Arc<Buffer<[T]>>>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let usage = match typ {
            BufferType::Index => BufferUsage::index_buffer(),
            BufferType::Vertex => BufferUsage::vertex_buffer(),
        };
        let cpu_buffer =
            CpuAccessibleBuffer::from_iter::<I>(device.device.clone(), usage, false, data)?;

        Ok(Arc::new(Self {
            typ,
            buffer: cpu_buffer,
        }))
    }
}
