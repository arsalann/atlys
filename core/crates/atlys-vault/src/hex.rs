//! Strict lowercase hex, as RFC-0002 requires for binary fields in JSON.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Encodes bytes as lowercase hex.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0x0F)] as char);
    }
    out
}

/// Decodes lowercase hex into exactly `N` bytes. Rejects uppercase, prefixes,
/// odd lengths, invalid digits, and any other length.
pub(crate) fn decode_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    decode_into(text.as_bytes(), &mut out)?;
    Some(out)
}

/// Decodes lowercase hex of any even length.
pub(crate) fn decode_vec(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut out = vec![0u8; text.len() / 2];
    decode_into(text.as_bytes(), &mut out)?;
    Some(out)
}

fn decode_into(text: &[u8], out: &mut [u8]) -> Option<()> {
    let (pairs, _) = text.as_chunks::<2>();
    for (slot, pair) in out.iter_mut().zip(pairs) {
        *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(())
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_strictness() {
        assert_eq!(encode(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(decode_array::<3>("00abff"), Some([0x00, 0xab, 0xff]));
        assert_eq!(decode_vec(""), Some(vec![]));
        assert_eq!(decode_array::<3>("00ABff"), None, "uppercase");
        assert_eq!(decode_array::<2>("00abff"), None, "wrong length");
        assert_eq!(decode_vec("abc"), None, "odd length");
        assert_eq!(decode_vec("0x"), None, "prefix");
        assert_eq!(decode_vec("zz"), None, "invalid digit");
        assert_eq!(decode_vec(" 0"), None, "whitespace");
    }
}
