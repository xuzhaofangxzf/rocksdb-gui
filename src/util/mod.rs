pub fn format_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) if text.chars().all(|character| !character.is_control()) => text.to_string(),
        _ => format!("0x{}", hex(bytes)),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_printable_utf8_as_text() {
        assert_eq!(format_bytes(b"records/00000001"), "records/00000001");
    }

    #[test]
    fn formats_binary_bytes_as_hex() {
        assert_eq!(format_bytes(&[0, 1, 0xff]), "0x0001ff");
    }

    #[test]
    fn formats_control_text_as_hex() {
        assert_eq!(format_bytes(b"line\nbreak"), "0x6c696e650a627265616b");
    }
}
