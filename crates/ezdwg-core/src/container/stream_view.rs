use crate::bit::BitReader;
use crate::container::section_directory::SectionLocatorRecord;
use crate::container::section_loader::SectionSlice;
use crate::io::ByteReader;

#[derive(Debug, Clone)]
pub struct StreamView<'a> {
    section: SectionSlice<'a>,
}

impl<'a> StreamView<'a> {
    pub fn new(section: SectionSlice<'a>) -> Self {
        Self { section }
    }

    pub fn record(&self) -> SectionLocatorRecord {
        self.section.record.clone()
    }

    pub fn offset(&self) -> u32 {
        self.section.record.offset
    }

    pub fn size(&self) -> u32 {
        self.section.record.size
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.section.data.as_ref()
    }

    pub fn byte_reader(&self) -> ByteReader<'_> {
        ByteReader::new(self.section.data.as_ref())
    }

    pub fn bit_reader(&self) -> BitReader<'_> {
        BitReader::new(self.section.data.as_ref())
    }
}
