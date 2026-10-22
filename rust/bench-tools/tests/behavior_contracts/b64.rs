//! Standard padded base64, decoded strictly as `base64.b64decode(value, validate=True)` does.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let group = u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(
                    ALPHABET[(group >> (18 - 6 * index) & 63) as usize],
                ));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err("base64: incorrect padding".to_string());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (number, chunk) in bytes.chunks(4).enumerate() {
        let last = number + 1 == bytes.len() / 4;
        let padding = chunk.iter().rev().take_while(|byte| **byte == b'=').count();
        if padding > 2 || padding > 0 && !last {
            return Err("base64: misplaced padding".to_string());
        }
        let mut group = 0u32;
        for byte in &chunk[..4 - padding] {
            let value = ALPHABET
                .iter()
                .position(|candidate| candidate == byte)
                .ok_or_else(|| "base64: only base64 characters are allowed".to_string())?;
            group = group << 6 | value as u32;
        }
        group <<= 6 * padding as u32;
        let decoded = [(group >> 16) as u8, (group >> 8) as u8, group as u8];
        out.extend_from_slice(&decoded[..3 - padding]);
    }
    Ok(out)
}
