//! Master password policy (RFC-0002 "Keys and encodings").
//!
//! New passwords need at least [`MIN_CHARS`] Unicode scalar values and a
//! zxcvbn score of at least [`MIN_SCORE`]. Passwords are exact UTF-8 bytes:
//! no trimming, case folding, or normalization. Unlocking an existing vault
//! never applies this policy, so a stricter future policy cannot lock anyone
//! out.

use crate::{Result, VaultError};

/// Minimum length in Unicode scalar values.
pub const MIN_CHARS: usize = 12;
/// Minimum zxcvbn score (0–4).
pub const MIN_SCORE: u8 = 3;

/// Strength estimate for display while the user types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strength {
    /// zxcvbn score, 0 (weakest) to 4.
    pub score: u8,
    /// Whether the password meets the policy for a new password.
    pub acceptable: bool,
    /// The main problem, if any, in plain English.
    pub warning: Option<String>,
    /// Suggestions for a stronger password.
    pub suggestions: Vec<String>,
}

/// Estimates strength. `user_inputs` are words the password should not lean
/// on, such as the app name.
pub fn strength(password: &str, user_inputs: &[&str]) -> Strength {
    let long_enough = password.chars().count() >= MIN_CHARS;
    if password.is_empty() {
        return Strength {
            score: 0,
            acceptable: false,
            warning: None,
            suggestions: vec![length_hint()],
        };
    }
    let mut inputs = vec!["atlys", "vault"];
    inputs.extend_from_slice(user_inputs);
    let estimate = zxcvbn::zxcvbn(password, &inputs);
    let score = u8::from(estimate.score());
    let (warning, mut suggestions) = match estimate.feedback() {
        Some(f) => (
            f.warning().map(|w| w.to_string()),
            f.suggestions().iter().map(ToString::to_string).collect(),
        ),
        None => (None, Vec::new()),
    };
    if !long_enough {
        suggestions.insert(0, length_hint());
    }
    Strength {
        score,
        acceptable: long_enough && score >= MIN_SCORE,
        warning,
        suggestions,
    }
}

/// Checks a new password against the policy.
pub fn check_new(password: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(password).map_err(|_| VaultError::WeakPassword("not UTF-8"))?;
    if text.chars().count() < MIN_CHARS {
        return Err(VaultError::WeakPassword("shorter than 12 characters"));
    }
    if !strength(text, &[]).acceptable {
        return Err(VaultError::WeakPassword("too easy to guess"));
    }
    Ok(())
}

fn length_hint() -> String {
    format!("Use at least {MIN_CHARS} characters. A few unrelated words work well.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_and_guessable_passwords() {
        for weak in [
            "",
            "short",
            "password1234",
            "aaaaaaaaaaaa",
            "123456789012",
            "qwertyuiopas",
        ] {
            assert!(!strength(weak, &[]).acceptable, "{weak:?}");
            assert!(check_new(weak.as_bytes()).is_err(), "{weak:?}");
        }
        assert!(check_new(&[0xff; 16]).is_err(), "invalid UTF-8");
    }

    #[test]
    fn accepts_long_unrelated_words() {
        for strong in [
            "synthetic correct horse battery",
            "plum cactus orbit ledger",
        ] {
            let s = strength(strong, &[]);
            assert!(s.acceptable, "{strong:?}: {s:?}");
            assert!(check_new(strong.as_bytes()).is_ok());
        }
    }

    #[test]
    fn length_counts_scalar_values_not_bytes() {
        // 11 scalar values but 22 bytes: too short regardless of score.
        let s = strength("ééééééééééé", &[]);
        assert!(!s.acceptable);
        assert!(s.suggestions[0].contains("12"));
    }

    #[test]
    fn feedback_is_offered_for_weak_passwords() {
        let s = strength("password1234", &[]);
        assert!(s.score < MIN_SCORE);
        assert!(s.warning.is_some() || !s.suggestions.is_empty());
    }
}
