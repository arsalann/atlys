//! The 24-word recovery phrase (RFC-0002 "Keys and encodings").
//!
//! 256 random bits encoded with the BIP-39 English wordlist and checksum.
//! Decoding validates the checksum and returns the original 32 bytes of
//! entropy, which feed HKDF directly. BIP-39's PBKDF2 wallet seed and
//! mnemonic passphrase are deliberately not used.
//!
//! The wordlist (`bip39-english.txt`) is the English list from the BIP-39
//! specification (github.com/bitcoin/bips, `bip-0039/english.txt`); a test
//! pins its SHA-256.

use std::sync::OnceLock;

use atlys_crypto::{SecretKey, random_bytes};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Result, VaultError};

/// Number of words in a recovery phrase.
pub const WORD_COUNT: usize = 24;

const WORDLIST_TEXT: &str = include_str!("bip39-english.txt");

fn wordlist() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| WORDLIST_TEXT.lines().collect())
}

/// The full English wordlist, for input autocompletion.
pub fn words() -> &'static [&'static str] {
    wordlist()
}

/// The 256-bit recovery secret behind the phrase. Zeroized on drop.
#[derive(Debug)]
pub struct RecoverySecret(SecretKey);

impl RecoverySecret {
    /// Generates fresh recovery entropy.
    pub fn generate() -> Result<Self> {
        Ok(Self(SecretKey::from_bytes(random_bytes()?)))
    }

    /// Wraps decoded recovery entropy.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(SecretKey::from_bytes(bytes))
    }

    /// The raw entropy.
    pub fn expose_secret(&self) -> &[u8; 32] {
        self.0.expose_secret()
    }

    pub(crate) fn key(&self) -> &SecretKey {
        &self.0
    }

    /// Encodes the secret as 24 words. The caller should drop the strings as
    /// soon as they have been shown.
    pub fn to_phrase(&self) -> Vec<String> {
        let entropy = self.expose_secret();
        let checksum = Sha256::digest(entropy)[0];
        // 256 entropy bits + 8 checksum bits = 24 groups of 11 bits.
        let mut bits = Zeroizing::new([0u8; 33]);
        bits[..32].copy_from_slice(entropy);
        bits[32] = checksum;
        (0..WORD_COUNT)
            .map(|i| wordlist()[read_11_bits(&bits[..], i * 11)].to_owned())
            .collect()
    }

    /// Decodes a phrase. Words are matched exactly after trimming surrounding
    /// whitespace and lowercasing; the checksum must match.
    pub fn from_phrase<S: AsRef<str>>(phrase: &[S]) -> Result<Self> {
        if phrase.len() != WORD_COUNT {
            return Err(VaultError::InvalidRecoveryPhrase);
        }
        let mut bits = Zeroizing::new([0u8; 33]);
        for (i, word) in phrase.iter().enumerate() {
            let word = Zeroizing::new(word.as_ref().trim().to_lowercase());
            let index = wordlist()
                .binary_search(&word.as_str())
                .map_err(|_| VaultError::InvalidRecoveryPhrase)?;
            write_11_bits(&mut bits[..], i * 11, index);
        }
        let mut entropy = [0u8; 32];
        entropy.copy_from_slice(&bits[..32]);
        let secret = Self::from_bytes(entropy);
        entropy.fill(0);
        if Sha256::digest(secret.expose_secret())[0] != bits[32] {
            return Err(VaultError::InvalidRecoveryPhrase);
        }
        Ok(secret)
    }
}

/// Whether `word` is in the wordlist (after trimming and lowercasing).
pub fn is_word(word: &str) -> bool {
    wordlist()
        .binary_search(&word.trim().to_lowercase().as_str())
        .is_ok()
}

fn read_11_bits(bytes: &[u8], start: usize) -> usize {
    (0..11).fold(0, |acc, j| {
        let bit = start + j;
        (acc << 1) | usize::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
    })
}

fn write_11_bits(bytes: &mut [u8], start: usize, value: usize) {
    for j in 0..11 {
        let bit = start + j;
        if (value >> (10 - j)) & 1 == 1 {
            bytes[bit / 8] |= 1 << (7 - bit % 8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordlist_is_the_bip39_english_list() {
        assert_eq!(
            hex(&Sha256::digest(WORDLIST_TEXT.as_bytes())),
            "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"
        );
        assert_eq!(wordlist().len(), 2048);
        assert!(
            wordlist().windows(2).all(|w| w[0] < w[1]),
            "sorted for binary search"
        );
    }

    // Published BIP-39 test vectors (256-bit entropy), from the reference
    // implementation's vectors.json.
    #[test]
    fn matches_bip39_reference_vectors() {
        let cases: [([u8; 32], &str); 3] = [
            (
                [0x00; 32],
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
            ),
            (
                [0x7f; 32],
                "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title",
            ),
            (
                [0xff; 32],
                "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
            ),
        ];
        for (entropy, phrase) in cases {
            let words = RecoverySecret::from_bytes(entropy).to_phrase();
            assert_eq!(words.join(" "), phrase);
            let parsed: Vec<&str> = phrase.split(' ').collect();
            assert_eq!(
                RecoverySecret::from_phrase(&parsed)
                    .unwrap()
                    .expose_secret(),
                &entropy
            );
        }
    }

    #[test]
    fn random_secrets_round_trip() {
        for _ in 0..32 {
            let secret = RecoverySecret::generate().unwrap();
            let phrase = secret.to_phrase();
            assert_eq!(phrase.len(), WORD_COUNT);
            let back = RecoverySecret::from_phrase(&phrase).unwrap();
            assert_eq!(back.expose_secret(), secret.expose_secret());
        }
    }

    #[test]
    fn input_is_trimmed_and_case_insensitive() {
        let phrase = RecoverySecret::from_bytes([0x7f; 32]).to_phrase();
        let messy: Vec<String> = phrase
            .iter()
            .map(|w| format!("  {}\n", w.to_uppercase()))
            .collect();
        assert_eq!(
            RecoverySecret::from_phrase(&messy).unwrap().expose_secret(),
            &[0x7f; 32]
        );
    }

    #[test]
    fn rejects_bad_phrases() {
        let phrase = RecoverySecret::from_bytes([0x7f; 32]).to_phrase();
        assert!(
            RecoverySecret::from_phrase(&phrase[..23]).is_err(),
            "23 words"
        );
        let mut unknown = phrase.clone();
        unknown[3] = "notaword".into();
        assert!(
            RecoverySecret::from_phrase(&unknown).is_err(),
            "unknown word"
        );
        let mut swapped = phrase.clone();
        swapped.swap(0, 1);
        assert!(RecoverySecret::from_phrase(&swapped).is_err(), "checksum");
        assert!(is_word("abandon") && is_word(" Zoo ") && !is_word("abandons"));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
