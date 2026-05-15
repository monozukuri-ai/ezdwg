use crate::objects::Handle;

#[derive(Debug, Clone, Copy)]
pub struct ObjectRef {
    pub handle: Handle,
    pub offset: u32,
}
