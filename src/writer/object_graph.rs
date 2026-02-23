#[derive(Debug, Clone)]
pub struct WriterObject {
    pub handle: u64,
    pub type_code: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct WriterObjectGraph {
    objects: Vec<WriterObject>,
}

impl WriterObjectGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_object(&mut self, object: WriterObject) {
        self.objects.push(object);
    }

    pub fn objects(&self) -> &[WriterObject] {
        &self.objects
    }

    pub fn into_sorted_by_handle(mut self) -> Vec<WriterObject> {
        self.objects.sort_by_key(|obj| obj.handle);
        self.objects
    }
}
