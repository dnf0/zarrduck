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
}
