//! HPKE (RFC 9180) for the write-only ingest inbox.
//!
//! Suite: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305, base
//! mode. This is the same suite Apple CryptoKit exposes as
//! `HPKE.Ciphersuite.Curve25519_SHA256_ChachaPoly`, so either side can seal.
//!
//! Collectors hold only the [`IngestPublicKey`]: they can encrypt new batches
//! but cannot decrypt anything, including their own earlier output.

use core::fmt;

use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable};
use zeroize::Zeroizing;

use crate::{CryptoError, Result};

type Kem = X25519HkdfSha256;
type Kdf = HkdfSha256;
type Aead = ChaCha20Poly1305;

/// Length of a serialized X25519 public key.
pub const PUBLIC_KEY_LEN: usize = 32;
/// Length of a serialized X25519 private key.
pub const PRIVATE_KEY_LEN: usize = 32;
/// Length of the HPKE encapsulated key (the sender's ephemeral public key).
pub const ENCAPPED_KEY_LEN: usize = 32;
/// Length of the AEAD tag appended to the ciphertext.
pub const TAG_LEN: usize = 16;

/// The ingest public key. Safe to store in plaintext and to hand to
/// background code.
#[derive(Clone, PartialEq, Eq)]
pub struct IngestPublicKey(<Kem as KemTrait>::PublicKey);

impl IngestPublicKey {
    /// Parses a 32-byte X25519 public key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PUBLIC_KEY_LEN {
            return Err(CryptoError::InvalidInput(
                "ingest public key must be 32 bytes",
            ));
        }
        <Kem as KemTrait>::PublicKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| CryptoError::InvalidInput("invalid ingest public key"))
    }

    /// Serializes the key.
    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LEN] {
        let mut out = [0u8; PUBLIC_KEY_LEN];
        self.0.write_exact(&mut out);
        out
    }
}

impl fmt::Debug for IngestPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IngestPublicKey")
            .field(&hex_prefix(&self.to_bytes()))
            .finish()
    }
}

/// The ingest private key. Lives only inside the unlocked vault (wrapped by
/// the vault key at rest). Zeroized on drop by the underlying implementation.
pub struct IngestPrivateKey(<Kem as KemTrait>::PrivateKey);

impl IngestPrivateKey {
    /// Parses a 32-byte X25519 private key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PRIVATE_KEY_LEN {
            return Err(CryptoError::InvalidInput(
                "ingest private key must be 32 bytes",
            ));
        }
        <Kem as KemTrait>::PrivateKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| CryptoError::InvalidInput("invalid ingest private key"))
    }

    /// Serializes the key, for wrapping under the vault key.
    pub fn to_bytes(&self) -> Zeroizing<[u8; PRIVATE_KEY_LEN]> {
        let mut out = Zeroizing::new([0u8; PRIVATE_KEY_LEN]);
        self.0.write_exact(out.as_mut());
        out
    }

    /// The matching public key.
    pub fn public_key(&self) -> IngestPublicKey {
        IngestPublicKey(<Kem as KemTrait>::sk_to_pk(&self.0))
    }
}

impl fmt::Debug for IngestPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IngestPrivateKey(<redacted>)")
    }
}

/// A freshly generated ingest keypair.
#[derive(Debug)]
pub struct IngestKeyPair {
    /// Private half: keep inside the vault.
    pub private: IngestPrivateKey,
    /// Public half: store in the vault header.
    pub public: IngestPublicKey,
}

impl IngestKeyPair {
    /// Generates a keypair from the operating system's CSPRNG.
    pub fn generate() -> Self {
        let (sk, pk) = Kem::gen_keypair();
        Self {
            private: IngestPrivateKey(sk),
            public: IngestPublicKey(pk),
        }
    }
}

/// Output of [`seal`]: the encapsulated key plus `ciphertext || tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedMessage {
    /// The sender's ephemeral public key (HPKE `enc`).
    pub encapped_key: [u8; ENCAPPED_KEY_LEN],
    /// AEAD ciphertext with the 16-byte tag appended.
    pub ciphertext: Vec<u8>,
}

/// Seals `plaintext` to `recipient` (HPKE single-shot, base mode).
pub fn seal(
    recipient: &IngestPublicKey,
    info: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<SealedMessage> {
    let (encapped, ciphertext) = hpke::single_shot_seal::<Aead, Kdf, Kem>(
        &OpModeS::Base,
        &recipient.0,
        info,
        plaintext,
        aad,
    )
    .map_err(|_| CryptoError::Internal)?;
    let mut encapped_key = [0u8; ENCAPPED_KEY_LEN];
    encapped.write_exact(&mut encapped_key);
    Ok(SealedMessage {
        encapped_key,
        ciphertext,
    })
}

/// Opens a message sealed by [`seal`] or any RFC 9180 implementation using
/// the same suite. The plaintext is zeroized on drop.
pub fn open(
    recipient: &IngestPrivateKey,
    info: &[u8],
    aad: &[u8],
    message: &SealedMessage,
) -> Result<Zeroizing<Vec<u8>>> {
    if message.ciphertext.len() < TAG_LEN {
        return Err(CryptoError::Decrypt);
    }
    let encapped = <Kem as KemTrait>::EncappedKey::from_bytes(&message.encapped_key)
        .map_err(|_| CryptoError::Decrypt)?;
    hpke::single_shot_open::<Aead, Kdf, Kem>(
        &OpModeR::Base,
        &recipient.0,
        &encapped,
        info,
        &message.ciphertext,
        aad,
    )
    .map(Zeroizing::new)
    .map_err(|_| CryptoError::Decrypt)
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// RFC 9180 Appendix A.2.1: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256,
    /// ChaCha20-Poly1305, base mode, sequence number 0.
    #[test]
    fn rfc9180_a2_known_answer() {
        let sk = IngestPrivateKey::from_bytes(
            &hex::decode("8057991eef8f1f1af18f4a9491d16a1ce333f695d4db8e38da75975c4478e0fb")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            hex::encode(sk.public_key().to_bytes()),
            "4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a"
        );
        let message = SealedMessage {
            encapped_key: hex::decode("1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a")
                .unwrap()
                .try_into()
                .unwrap(),
            ciphertext: hex::decode(
                "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db21993c62ce81883d2dd1b51a28",
            )
            .unwrap(),
        };
        let info = hex::decode("4f6465206f6e2061204772656369616e2055726e").unwrap();
        let aad = hex::decode("436f756e742d30").unwrap();
        let pt = open(&sk, &info, &aad, &message).unwrap();
        assert_eq!(&pt[..], b"Beauty is truth, truth beauty");
    }

    #[test]
    fn round_trip() {
        let kp = IngestKeyPair::generate();
        let sealed = seal(&kp.public, b"atlys/test", b"hdr", b"batch").unwrap();
        assert_eq!(sealed.ciphertext.len(), 5 + TAG_LEN);
        let pt = open(&kp.private, b"atlys/test", b"hdr", &sealed).unwrap();
        assert_eq!(&pt[..], b"batch");
    }

    #[test]
    fn rejects_wrong_recipient_info_aad_and_enc() {
        let kp = IngestKeyPair::generate();
        let other = IngestKeyPair::generate();
        let sealed = seal(&kp.public, b"info", b"aad", b"batch").unwrap();
        assert!(open(&other.private, b"info", b"aad", &sealed).is_err());
        assert!(open(&kp.private, b"other", b"aad", &sealed).is_err());
        assert!(open(&kp.private, b"info", b"other", &sealed).is_err());
        let mut bad_enc = sealed.clone();
        bad_enc.encapped_key[0] ^= 1;
        assert!(open(&kp.private, b"info", b"aad", &bad_enc).is_err());
    }

    #[test]
    fn keys_round_trip_through_bytes() {
        let kp = IngestKeyPair::generate();
        let sk = IngestPrivateKey::from_bytes(&kp.private.to_bytes()[..]).unwrap();
        assert_eq!(sk.public_key(), kp.public);
        assert_eq!(
            IngestPublicKey::from_bytes(&kp.public.to_bytes()).unwrap(),
            kp.public
        );
        assert!(IngestPublicKey::from_bytes(&[0u8; 31]).is_err());
        assert!(format!("{:?}", kp.private).contains("redacted"));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn any_ciphertext_bit_flip_is_rejected(pt in proptest::collection::vec(any::<u8>(), 0..512),
                                               bit in any::<prop::sample::Index>()) {
            let kp = IngestKeyPair::generate();
            let mut sealed = seal(&kp.public, b"info", b"aad", &pt).unwrap();
            let i = bit.index(sealed.ciphertext.len() * 8);
            sealed.ciphertext[i / 8] ^= 1 << (i % 8);
            prop_assert!(open(&kp.private, b"info", b"aad", &sealed).is_err());
        }
    }
}
