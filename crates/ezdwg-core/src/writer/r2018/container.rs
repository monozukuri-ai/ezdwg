//! R2004-family page container, with uncompressed data pages and literal-only
//! LZ streams for the two system maps. No template or external writer is used.
use crate::core::{
    error::{DwgError, ErrorKind},
    result::Result,
};

const PAGE: usize = 0x7400;
fn u32_at(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn u64_at(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn aligned(n: usize) -> usize {
    (n + 31) & !31
}

fn checksum(seed: u32, bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (seed & 0xffff, seed >> 16);
    for chunk in bytes.chunks(0x15b0) {
        for &v in chunk {
            a += u32::from(v);
            b += a;
        }
        a %= 0xfff1;
        b %= 0xfff1;
    }
    (b << 16) | a
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &v in bytes {
        crc ^= u32::from(v);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn literal(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if bytes.len() <= 18 {
        out.push((bytes.len() - 3) as u8);
    } else {
        out.push(0);
        let mut left = bytes.len() - 18;
        while left > 255 {
            out.push(0);
            left -= 255;
        }
        out.push(left as u8);
    }
    out.extend_from_slice(bytes);
    out.extend_from_slice(&[0x11, 0, 0]);
    out
}

fn system(magic: u32, bytes: &[u8]) -> Vec<u8> {
    let compressed = literal(bytes);
    let mut out = vec![0; aligned(20 + compressed.len() + 20)];
    for (i, v) in [magic, bytes.len() as u32, compressed.len() as u32, 2, 0]
        .into_iter()
        .enumerate()
    {
        u32_at(&mut out, i * 4, v);
    }
    let sum = checksum(checksum(0, &out[..20]), &compressed);
    u32_at(&mut out, 16, sum);
    out[20..20 + compressed.len()].copy_from_slice(&compressed);
    u32_at(&mut out, 20 + compressed.len(), magic);
    u32_at(&mut out, 32 + compressed.len(), 2);
    out
}

pub(crate) fn write_container(sections: Vec<(&str, Vec<u8>)>) -> Result<Vec<u8>> {
    let mut file = vec![0u8; 0x100];
    let mut pages = Vec::new();
    // Include the empty logical section (id 0), as required by the format.
    let mut map = vec![0u8; 20 + 96];
    u32_at(&mut map, 0, (sections.len() + 1) as u32);
    u32_at(&mut map, 4, 2);
    u32_at(&mut map, 8, PAGE as u32);
    u32_at(&mut map, 16, (sections.len() + 1) as u32);
    u32_at(&mut map, 20 + 12, PAGE as u32);
    u32_at(&mut map, 20 + 20, 2);
    for (section_index, (name, data)) in sections.iter().enumerate() {
        let section_id = (sections.len() - section_index) as u32;
        let capacity = PAGE.min(aligned(data.len()).max(32));
        let chunks: Vec<_> = data.chunks(capacity).collect();
        let at = map.len();
        map.resize(at + 96 + chunks.len() * 16, 0);
        u64_at(&mut map, at, data.len() as u64);
        u32_at(&mut map, at + 8, chunks.len() as u32);
        u32_at(&mut map, at + 12, capacity as u32);
        u32_at(&mut map, at + 16, 1);
        u32_at(&mut map, at + 20, 1);
        u32_at(&mut map, at + 24, section_id);
        map[at + 32..at + 32 + name.len()].copy_from_slice(name.as_bytes());
        for (i, chunk) in chunks.iter().enumerate() {
            let id = pages.len() as u32 + 1;
            let offset = u32::try_from(file.len())
                .map_err(|_| DwgError::new(ErrorKind::Unsupported, "AC1032 file exceeds 4 GiB"))?;
            let mut page = vec![0u8; 32 + capacity];
            page[32..32 + chunk.len()].copy_from_slice(chunk);
            let data_checksum = checksum(0, &page[32..]);
            for (j, v) in [
                0x4163_043b,
                section_id,
                capacity as u32,
                (32 + capacity) as u32,
                (i * capacity) as u32,
                0,
                0,
                data_checksum,
            ]
            .into_iter()
            .enumerate()
            {
                u32_at(&mut page, j * 4, v);
            }
            let sum = checksum(data_checksum, &page[..32]);
            u32_at(&mut page, 24, sum);
            let mask = 0x4164_536b ^ offset;
            for j in 0..8 {
                let v = u32::from_le_bytes(page[j * 4..j * 4 + 4].try_into().unwrap()) ^ mask;
                u32_at(&mut page, j * 4, v);
            }
            page[32..32 + chunk.len()].copy_from_slice(chunk);
            let info = at + 96 + i * 16;
            u32_at(&mut map, info, id);
            u32_at(&mut map, info + 4, capacity as u32);
            u64_at(&mut map, info + 8, (i * capacity) as u64);
            pages.push((id, page.len() as u32));
            file.extend_from_slice(&page);
        }
    }
    let section_map_id = pages.len() as u32 + 1;
    let section_map = system(0x4163_003b, &map);
    pages.push((section_map_id, section_map.len() as u32));
    file.extend_from_slice(&section_map);
    let page_map_id = section_map_id + 2;
    let page_map_offset = file.len();
    let mut page_map_data = Vec::new();
    for &(id, size) in &pages {
        page_map_data.extend_from_slice(&id.to_le_bytes());
        page_map_data.extend_from_slice(&size.to_le_bytes());
    }
    page_map_data.extend_from_slice(&page_map_id.to_le_bytes());
    page_map_data.extend_from_slice(&0u32.to_le_bytes());
    let provisional = system(0x4163_0e3b, &page_map_data);
    let n = page_map_data.len();
    u32_at(&mut page_map_data, n - 4, provisional.len() as u32);
    let page_map = system(0x4163_0e3b, &page_map_data);
    file.extend_from_slice(&page_map);
    let mut header = vec![0u8; 0x6c];
    header[..11].copy_from_slice(b"AcFssFcAJMB");
    for (at, v) in [
        (0x10, 0x6c),
        (0x14, 4),
        (0x24, 1),
        (0x28, page_map_id),
        (0x40, pages.len() as u32 + 1),
        (0x44, 0x20),
        (0x48, 0x80),
        (0x4c, 0x40),
        (0x50, page_map_id),
        (0x5c, section_map_id),
        (0x60, page_map_id),
    ] {
        u32_at(&mut header, at, v);
    }
    u64_at(&mut header, 0x2c, (file.len() - 0x100) as u64);
    u64_at(&mut header, 0x34, file.len() as u64);
    u64_at(&mut header, 0x54, (page_map_offset - 0x100) as u64);
    let crc = crc32(&header);
    u32_at(&mut header, 0x68, crc);
    file[..6].copy_from_slice(b"AC1032");
    file[0x0b] = 4;
    file[0x0c] = 3;
    file[0x11] = 32;
    file[0x13..0x15].copy_from_slice(&30u16.to_le_bytes());
    u32_at(&mut file, 0x28, 0x80);
    let mut seed = 1u32;
    for i in 0..0x80 {
        seed = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        file[0x80 + i] = (seed >> 16) as u8 ^ header.get(i).copied().unwrap_or(0);
    }
    let encrypted = file[0x80..0xec].to_vec();
    file.extend_from_slice(&encrypted);
    Ok(file)
}
