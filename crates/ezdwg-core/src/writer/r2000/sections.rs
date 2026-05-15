use crate::container::SectionLocatorRecord;

#[derive(Debug, Clone, Default)]
pub struct SectionAssembly {
    pub records: Vec<SectionLocatorRecord>,
    pub data: Vec<u8>,
}
