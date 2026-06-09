use subtle::ConstantTimeEq;

use crate::domain::NonEmptyString;

pub mod domain;
pub mod error;

/// Uses constant-time verification to check API key correspondence.
///
/// If the API keys correspond, returns `true`,
/// otherwise `false`.
///
/// ### Cryptographic security
///
/// The function short-circuits if lengths of `expected` and `provided` are unequal.
/// While this allows an attacker to extract the key length,
/// it is order of magnitudes safer than using a simple string equality test,
/// which would allow the attacker to gradually know the exact key
/// over many requests.
pub fn verify_api_key(expected: Option<&NonEmptyString>, provided: Option<&str>) -> bool {
    expected.zip(provided).is_some_and(|(key, header)| {
        let key_bytes = key.as_bytes();
        let header_bytes = header.as_bytes();
        key_bytes.ct_eq(header_bytes).into()
    })
}
