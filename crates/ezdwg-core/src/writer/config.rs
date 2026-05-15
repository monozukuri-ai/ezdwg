#[derive(Debug, Clone)]
pub struct WriterConfig {
    pub strict: bool,
    pub preserve_input_handles: bool,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            strict: false,
            preserve_input_handles: true,
        }
    }
}
