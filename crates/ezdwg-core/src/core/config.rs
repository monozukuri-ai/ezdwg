#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub strict: bool,
    pub max_recursion: u32,
    pub max_objects: u32,
    pub max_section_bytes: u64,
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self {
            strict: false,
            max_recursion: 64,
            max_objects: 1_000_000,
            max_section_bytes: 256 * 1024 * 1024,
        }
    }
}
