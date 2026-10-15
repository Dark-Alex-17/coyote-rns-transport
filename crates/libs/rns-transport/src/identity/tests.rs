use super::{Identity, PrivateIdentity, PUBLIC_KEY_LENGTH};
use crate::error::RnsError;
use rand_core::OsRng;

const HEX_IDENTITY_LENGTH: usize = PUBLIC_KEY_LENGTH * 2 * 2;

/// The two-byte char straddles the first two-char slice boundary.
fn multibyte_hex_of_correct_byte_length() -> String {
    let input = format!("0\u{00e9}{}", "0".repeat(HEX_IDENTITY_LENGTH - 3));
    assert_eq!(input.len(), HEX_IDENTITY_LENGTH);
    input
}

#[test]
fn private_identity_hex_string() {
    let original_id = PrivateIdentity::new_from_rand(OsRng);
    let original_hex = original_id.to_hex_string();

    let actual_id = PrivateIdentity::new_from_hex_string(&original_hex).expect("valid identity");

    assert_eq!(actual_id.private_key.as_bytes(), original_id.private_key.as_bytes());
    assert_eq!(actual_id.sign_key.as_bytes(), original_id.sign_key.as_bytes());
}

#[test]
fn public_identity_constructors_reject_malformed_keys() {
    let valid = PrivateIdentity::new_from_rand(OsRng);
    let identity = valid.as_identity();
    assert!(Identity::try_new_from_slices(
        identity.public_key_bytes(),
        identity.verifying_key_bytes()
    )
    .is_ok());
    assert!(Identity::try_new_from_slices(&[0_u8; PUBLIC_KEY_LENGTH - 1], &[0_u8; 32]).is_err());
    let invalid_verifying_key = (0_u8..=u8::MAX)
        .map(|byte| [byte; PUBLIC_KEY_LENGTH])
        .find(|bytes| ed25519_dalek::VerifyingKey::from_bytes(bytes).is_err())
        .expect("at least one compressed point encoding must be invalid");
    assert!(Identity::try_new_from_slices(&[0_u8; 32], &invalid_verifying_key).is_err());
    assert!(Identity::new_from_hex_string(&"zz".repeat(PUBLIC_KEY_LENGTH * 2)).is_err());
    assert!(Identity::new_from_hex_string(&"00".repeat(PUBLIC_KEY_LENGTH * 2 + 1)).is_err());
}

#[test]
fn private_identity_hex_constructor_rejects_malformed_keys() {
    assert!(PrivateIdentity::new_from_hex_string("not-an-identity").is_err());
    assert!(PrivateIdentity::new_from_hex_string(&"00".repeat(PUBLIC_KEY_LENGTH * 2 - 1)).is_err());
    assert!(PrivateIdentity::new_from_hex_string(&"00".repeat(PUBLIC_KEY_LENGTH * 2 + 1)).is_err());
}

#[test]
fn public_identity_hex_constructor_rejects_multibyte_char_of_correct_byte_length() {
    assert!(matches!(
        Identity::new_from_hex_string(&multibyte_hex_of_correct_byte_length()),
        Err(RnsError::IncorrectHash)
    ));
}

/// The two-byte char starts on the second-to-last byte, so the last two-byte slice is the
/// one that would straddle a char boundary.
fn multibyte_hex_at_end_of_correct_byte_length() -> String {
    let input = format!("{}\u{00e9}0", "0".repeat(HEX_IDENTITY_LENGTH - 3));
    assert_eq!(input.len(), HEX_IDENTITY_LENGTH);
    input
}

/// A four-byte char in the middle of an otherwise valid string.
fn four_byte_char_in_middle_of_correct_byte_length() -> String {
    let half = (HEX_IDENTITY_LENGTH - 4) / 2;
    let input = format!("{}\u{1F600}{}", "a".repeat(half), "a".repeat(half));
    assert_eq!(input.len(), HEX_IDENTITY_LENGTH);
    input
}

#[test]
fn identity_hex_constructors_reject_multibyte_char_at_end_and_in_middle() {
    for input in [
        multibyte_hex_at_end_of_correct_byte_length(),
        four_byte_char_in_middle_of_correct_byte_length(),
    ] {
        assert!(matches!(Identity::new_from_hex_string(&input), Err(RnsError::IncorrectHash)));
        assert!(matches!(
            PrivateIdentity::new_from_hex_string(&input),
            Err(RnsError::IncorrectHash)
        ));
    }
}

#[test]
fn identity_hex_constructors_reject_empty_and_odd_length_input() {
    for input in
        ["", "0", &"0".repeat(HEX_IDENTITY_LENGTH - 1), &"0".repeat(HEX_IDENTITY_LENGTH + 1)]
    {
        assert!(Identity::new_from_hex_string(input).is_err(), "Identity accepted {input:?}");
        assert!(
            PrivateIdentity::new_from_hex_string(input).is_err(),
            "PrivateIdentity accepted {input:?}"
        );
    }
}

#[test]
fn identity_hex_constructors_accept_uppercase_hex() {
    let original = PrivateIdentity::new_from_rand(OsRng);
    let upper = original.to_hex_string().to_uppercase();
    let restored = PrivateIdentity::new_from_hex_string(&upper).expect("uppercase hex parses");
    assert_eq!(restored.to_hex_string(), original.to_hex_string());
    let public = original.as_identity();
    let restored = Identity::new_from_hex_string(&public.to_hex_string().to_uppercase())
        .expect("uppercase public hex parses");
    assert_eq!(restored.to_hex_string(), public.to_hex_string());
}

#[test]
fn public_identity_hex_constructor_rejects_non_hex_ascii_of_correct_length() {
    assert!(matches!(
        Identity::new_from_hex_string(&"zz".repeat(HEX_IDENTITY_LENGTH / 2)),
        Err(RnsError::IncorrectHash)
    ));
}

#[test]
fn private_identity_hex_constructor_rejects_multibyte_char_of_correct_byte_length() {
    assert!(matches!(
        PrivateIdentity::new_from_hex_string(&multibyte_hex_of_correct_byte_length()),
        Err(RnsError::IncorrectHash)
    ));
}

#[test]
fn private_identity_hex_constructor_rejects_non_hex_ascii_of_correct_length() {
    assert!(matches!(
        PrivateIdentity::new_from_hex_string(&"zz".repeat(HEX_IDENTITY_LENGTH / 2)),
        Err(RnsError::IncorrectHash)
    ));
}
