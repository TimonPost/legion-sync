pub struct BufferResource {
    pub(crate) recv_buffer: Vec<u8>,
}

impl BufferResource {
    pub fn from_capacity(size: usize) -> BufferResource {
        BufferResource {
            recv_buffer: vec![0; size],
        }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.recv_buffer
    }
}
