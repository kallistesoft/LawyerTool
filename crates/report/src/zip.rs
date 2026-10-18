//! Minimal ZIP writer (entries stored, not compressed): enough for .docx.

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Builds a ZIP archive from (name, contents) pairs, in order.
pub fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    // Fixed DOS date/time (1980-01-01 00:00) so output is reproducible.
    let (time, date) = (0u16, 0x21u16);
    for (name, data) in entries {
        let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let crc = crc32(data);
        let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);

        u32le(&mut out, 0x0403_4b50); // local file header
        u16le(&mut out, 20); // version needed
        u16le(&mut out, 0x0800); // flags: UTF-8 names
        u16le(&mut out, 0); // stored
        u16le(&mut out, time);
        u16le(&mut out, date);
        u32le(&mut out, crc);
        u32le(&mut out, size);
        u32le(&mut out, size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        u32le(&mut central, 0x0201_4b50); // central directory header
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        u16le(&mut central, 0x0800);
        u16le(&mut central, 0);
        u16le(&mut central, time);
        u16le(&mut central, date);
        u32le(&mut central, crc);
        u32le(&mut central, size);
        u32le(&mut central, size);
        u16le(&mut central, name_len);
        u16le(&mut central, 0); // extra
        u16le(&mut central, 0); // comment
        u16le(&mut central, 0); // disk
        u16le(&mut central, 0); // internal attrs
        u32le(&mut central, 0); // external attrs
        u32le(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
    let cd_size = u32::try_from(central.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&central);
    let n = u16::try_from(entries.len()).unwrap_or(u16::MAX);
    u32le(&mut out, 0x0605_4b50); // end of central directory
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, n);
    u16le(&mut out, n);
    u32le(&mut out, cd_size);
    u32le(&mut out, cd_offset);
    u16le(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc32_known_value() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }
}
