//! Time-ordered unique file names (ULID layout: 48-bit millisecond timestamp
//! followed by 80 random bits, Crockford base32).

use std::time::{SystemTime, UNIX_EPOCH};

use atlys_crypto::random_bytes;

use crate::Result;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Generates a 26-character ULID string. Lexicographic order follows creation
/// time (to the millisecond), so inbox files merge in the order they were written.
pub(crate) fn new() -> Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let random = random_bytes::<10>()?;
    let mut value: u128 = u128::from(millis & 0xFFFF_FFFF_FFFF) << 80;
    for (i, b) in random.iter().enumerate() {
        value |= u128::from(*b) << (72 - 8 * i);
    }
    let mut out = [0u8; 26];
    for (i, slot) in out.iter_mut().enumerate() {
        let shift = 125 - 5 * i as u32;
        *slot = ALPHABET[((value >> shift) & 0x1F) as usize];
    }
    Ok(String::from_utf8(out.to_vec()).expect("alphabet is ASCII"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn format_and_ordering() {
        let a = super::new().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = super::new().unwrap();
        assert_eq!(a.len(), 26);
        assert!(a.bytes().all(|c| super::ALPHABET.contains(&c)));
        assert!(a < b);
    }
}
