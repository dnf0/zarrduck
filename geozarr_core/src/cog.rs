pub struct TiffHeader {
    pub is_little_endian: bool,
    pub first_ifd_offset: u32,
}

pub fn parse_tiff_header(buffer: &[u8]) -> Result<TiffHeader, String> {
    if buffer.len() < 8 {
        return Err("Buffer too small for TIFF header".into());
    }

    let is_little_endian = match &buffer[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("Invalid TIFF byte order".into()),
    };

    let magic = if is_little_endian {
        u16::from_le_bytes(buffer[2..4].try_into().unwrap())
    } else {
        u16::from_be_bytes(buffer[2..4].try_into().unwrap())
    };

    if magic != 42 && magic != 43 {
        // BigTIFF is 43, classic is 42
        return Err("Invalid TIFF magic number".into());
    }

    let first_ifd_offset = if is_little_endian {
        u32::from_le_bytes(buffer[4..8].try_into().unwrap())
    } else {
        u32::from_be_bytes(buffer[4..8].try_into().unwrap())
    };

    Ok(TiffHeader {
        is_little_endian,
        first_ifd_offset,
    })
}

#[derive(Debug, Default)]
pub struct CogMetadata {
    pub image_width: u32,
    pub image_length: u32,
    pub tile_width: u32,
    pub tile_length: u32,
    pub tile_offsets: Vec<u64>,
    pub tile_byte_counts: Vec<u64>,
}

pub fn parse_cog_metadata(buffer: &[u8]) -> Result<CogMetadata, String> {
    let header = parse_tiff_header(buffer)?;
    let mut meta = CogMetadata::default();

    let mut offset = header.first_ifd_offset as usize;
    if offset + 2 > buffer.len() {
        return Err("IFD offset out of bounds".into());
    }

    let num_entries = if header.is_little_endian {
        u16::from_le_bytes(buffer[offset..offset + 2].try_into().unwrap())
    } else {
        u16::from_be_bytes(buffer[offset..offset + 2].try_into().unwrap())
    };
    offset += 2;

    for _ in 0..num_entries {
        if offset + 12 > buffer.len() {
            break;
        }

        let tag = if header.is_little_endian {
            u16::from_le_bytes(buffer[offset..offset + 2].try_into().unwrap())
        } else {
            u16::from_be_bytes(buffer[offset..offset + 2].try_into().unwrap())
        };

        // Simplified value extraction for u32 values fitting inline
        let val = if header.is_little_endian {
            u32::from_le_bytes(buffer[offset + 8..offset + 12].try_into().unwrap())
        } else {
            u32::from_be_bytes(buffer[offset + 8..offset + 12].try_into().unwrap())
        };

        match tag {
            256 => meta.image_width = val,
            257 => meta.image_length = val,
            322 => meta.tile_width = val,
            323 => meta.tile_length = val,
            // For true arrays, we would follow the pointer to extract Vec<u64>.
            // Stubbed inline values for minimal implementation:
            324 => meta.tile_offsets.push(val as u64),
            325 => meta.tile_byte_counts.push(val as u64),
            _ => {}
        }
        offset += 12;
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tiff_header() {
        // Little-endian TIFF header (II), 42, IFD offset = 8
        let buffer: &[u8] = &[0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        let header = parse_tiff_header(buffer).unwrap();
        assert_eq!(header.is_little_endian, true);
        assert_eq!(header.first_ifd_offset, 8);
    }

    #[test]
    fn test_parse_ifd() {
        // Dummy buffer with a simple IFD at offset 8 containing 1 tag (ImageWidth)
        // Tag: 256 (ImageWidth), Type: 4 (LONG), Count: 1, Value: 1024
        let mut buffer = vec![0; 32];
        buffer[0..2].copy_from_slice(b"II"); // LE
        buffer[2..4].copy_from_slice(&42u16.to_le_bytes()); // Magic
        buffer[4..8].copy_from_slice(&8u32.to_le_bytes()); // Offset=8

        // IFD starts at 8
        buffer[8..10].copy_from_slice(&1u16.to_le_bytes()); // 1 entry
                                                            // Entry 0 starts at 10
        buffer[10..12].copy_from_slice(&256u16.to_le_bytes()); // Tag=ImageWidth
        buffer[12..14].copy_from_slice(&4u16.to_le_bytes()); // Type=LONG
        buffer[14..18].copy_from_slice(&1u32.to_le_bytes()); // Count=1
        buffer[18..22].copy_from_slice(&1024u32.to_le_bytes()); // Value=1024

        let info = parse_cog_metadata(&buffer).unwrap();
        assert_eq!(info.image_width, 1024);
    }
}
