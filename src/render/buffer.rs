use std::{error::Error, result, sync::Arc};

use vulkano::buffer::{BufferContents, BufferUsage, CpuAccessibleBuffer};

use super::Device;

type Result<T> = result::Result<T, Box<dyn Error>>;

pub enum BufferType {
    Vertex,
}

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
    pub fn vertex<I>(device: &Device, data: I) -> Result<Arc<Buffer<[T]>>>
    where
        I: IntoIterator<Item = T>,
        <I as std::iter::IntoIterator>::IntoIter: std::iter::ExactSizeIterator,
    {
        let cpu_buffer = CpuAccessibleBuffer::from_iter::<I>(
            device.device.clone(),
            BufferUsage::vertex_buffer(),
            false,
            data,
        )?;

        let b = Self {
            typ: BufferType::Vertex,
            buffer: cpu_buffer,
        };

        Ok(Arc::new(b))
    }
}
