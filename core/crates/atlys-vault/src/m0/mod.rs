//! M0 spike formats: the version 1 sealed inbox and Secure Enclave envelope.
//!
//! The product formats are specified by RFC-0002 and implemented in
//! [`crate::inbox`] and [`crate::envelope`]. These modules exist only so the
//! spike app and the current FFI keep working until the FFI moves to the
//! product vault (M1.3b), when they are deleted. Product readers never fall
//! back to these formats.

pub mod envelope;
pub mod inbox;
