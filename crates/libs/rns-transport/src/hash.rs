use core::cmp;
use core::fmt;

use crypto_common::typenum::Unsigned;
use crypto_common::OutputSizeUser;
use rand_core::CryptoRngCore;
use sha2::{Digest, Sha256};

use crate::error::RnsError;

pub const HASH_SIZE: usize = <<Sha256 as OutputSizeUser>::OutputSize as Unsigned>::USIZE;
pub const ADDRESS_HASH_SIZE: usize = 16;

pub fn create_hash(data: &[u8], out: &mut [u8]) {
    out.copy_from_slice(
        &Sha256::new().chain_update(data).finalize().as_slice()[..cmp::min(out.len(), HASH_SIZE)],
    );
}

pub fn address_hash(data: &[u8]) -> [u8; ADDRESS_HASH_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut truncated = [0u8; ADDRESS_HASH_SIZE];
    truncated.copy_from_slice(&out[..ADDRESS_HASH_SIZE]);
    truncated
}

pub fn lxmf_address_hash(hash: &Hash) -> AddressHash {
    AddressHash::new_from_hash(hash)
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct Hash([u8; HASH_SIZE]);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct AddressHash([u8; ADDRESS_HASH_SIZE]);

impl Hash {
    pub fn generator() -> Sha256 {
        Sha256::new()
    }

    pub const fn new(hash: [u8; HASH_SIZE]) -> Self {
        Self(hash)
    }

    pub const fn new_empty() -> Self {
        Self([0u8; HASH_SIZE])
    }

    pub fn new_from_slice(data: &[u8]) -> Self {
        let mut hash = [0u8; HASH_SIZE];
        create_hash(data, &mut hash);
        Self(hash)
    }

    pub fn new_from_rand<R: CryptoRngCore>(mut rng: R) -> Self {
        let mut hash = [0u8; HASH_SIZE];
        let mut data = [0u8; HASH_SIZE];

        rng.fill_bytes(&mut data[..]);

        create_hash(&data, &mut hash);
        Self(hash)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8; HASH_SIZE] {
        &self.0
    }

    pub fn to_bytes(&self) -> [u8; HASH_SIZE] {
        self.0
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl AddressHash {
    pub const fn new(hash: [u8; ADDRESS_HASH_SIZE]) -> Self {
        Self(hash)
    }

    pub fn new_from_slice(data: &[u8]) -> Self {
        let mut hash = [0u8; ADDRESS_HASH_SIZE];
        create_hash(data, &mut hash);
        Self(hash)
    }

    pub fn new_from_hash(hash: &Hash) -> Self {
        let mut address_hash = [0u8; ADDRESS_HASH_SIZE];
        address_hash.copy_from_slice(&hash.0[0..ADDRESS_HASH_SIZE]);
        Self(address_hash)
    }

    pub fn new_from_rand<R: CryptoRngCore>(rng: R) -> Self {
        Self::new_from_hash(&Hash::new_from_rand(rng))
    }

    pub fn new_from_hex_string(hex_string: &str) -> Result<Self, RnsError> {
        let decoded = hex::decode(hex_string).map_err(|_| RnsError::IncorrectHash)?;
        let bytes: [u8; ADDRESS_HASH_SIZE] =
            decoded.try_into().map_err(|_| RnsError::IncorrectHash)?;
        Ok(Self(bytes))
    }

    pub const fn new_empty() -> Self {
        Self([0u8; ADDRESS_HASH_SIZE])
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0[..]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub fn to_hex_string(&self) -> String {
        hex::encode(self.0)
    }

    /// First four bytes as eight hex chars, for log lines that must not carry a full address.
    pub fn short(&self) -> String {
        hex::encode(&self.0[..4])
    }
}

impl From<Hash> for AddressHash {
    fn from(hash: Hash) -> Self {
        Self::new_from_hash(&hash)
    }
}

impl Default for AddressHash {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl fmt::Display for AddressHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        for data in self.0.iter() {
            write!(f, "{:0>2x}", data)?;
        }
        write!(f, "/")?;

        Ok(())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for data in self.0.iter() {
            write!(f, "{:0>2x}", data)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use rand_core::OsRng;

    use crate::error::RnsError;
    use crate::hash::{AddressHash, ADDRESS_HASH_SIZE};

    #[test]
    fn address_hex_string() {
        let original_address_hash = AddressHash::new_from_rand(OsRng);

        let address_hash_hex = original_address_hash.to_hex_string();

        let actual_address_hash =
            AddressHash::new_from_hex_string(&address_hash_hex).expect("valid hash");

        assert_eq!(actual_address_hash.as_slice(), original_address_hash.as_slice());
    }

    #[test]
    fn address_hex_string_rejects_malformed_input() {
        assert!(AddressHash::new_from_hex_string("not-a-hash").is_err());
        assert!(AddressHash::new_from_hex_string(&"00".repeat(15)).is_err());
        assert!(AddressHash::new_from_hex_string(&"00".repeat(17)).is_err());
    }

    #[test]
    fn address_hex_string_rejects_multibyte_char_of_correct_byte_length() {
        // The two-byte char straddles the first two-char slice boundary.
        let input = format!("0\u{00e9}{}", "0".repeat(ADDRESS_HASH_SIZE * 2 - 3));
        assert_eq!(input.len(), ADDRESS_HASH_SIZE * 2);
        assert!(matches!(AddressHash::new_from_hex_string(&input), Err(RnsError::IncorrectHash)));
    }

    #[test]
    fn address_hex_string_rejects_non_hex_ascii_of_correct_length() {
        assert!(matches!(
            AddressHash::new_from_hex_string(&"zz".repeat(ADDRESS_HASH_SIZE)),
            Err(RnsError::IncorrectHash)
        ));
    }

    // `u8::from_str_radix` accepts a leading `+`, so a pairwise parser would
    // take these; the hex decoder must not.
    #[test]
    fn address_hex_string_rejects_plus_prefixed_pairs_of_correct_length() {
        let all_plus = "+1".repeat(ADDRESS_HASH_SIZE);
        let one_plus = format!("+{}", "0".repeat(ADDRESS_HASH_SIZE * 2 - 1));
        for input in [all_plus, one_plus] {
            assert!(matches!(
                AddressHash::new_from_hex_string(&input),
                Err(RnsError::IncorrectHash)
            ));
        }
    }

    #[test]
    fn address_hex_string_rejects_multibyte_char_at_end_and_four_byte_char_in_middle() {
        // The two-byte char starts on the second-to-last byte, straddling the last pair.
        let at_end = format!("{}\u{00e9}0", "0".repeat(ADDRESS_HASH_SIZE * 2 - 3));
        assert_eq!(at_end.len(), ADDRESS_HASH_SIZE * 2);
        let half = (ADDRESS_HASH_SIZE * 2 - 4) / 2;
        let in_middle = format!("{}\u{1F600}{}", "a".repeat(half), "a".repeat(half));
        assert_eq!(in_middle.len(), ADDRESS_HASH_SIZE * 2);
        for input in [at_end, in_middle] {
            assert!(matches!(
                AddressHash::new_from_hex_string(&input),
                Err(RnsError::IncorrectHash)
            ));
        }
    }

    #[test]
    fn address_hex_string_rejects_empty_and_odd_length_and_accepts_uppercase() {
        for input in ["", "0", &"0".repeat(ADDRESS_HASH_SIZE * 2 - 1)] {
            assert!(AddressHash::new_from_hex_string(input).is_err(), "accepted {input:?}");
        }
        let hash = AddressHash::new_from_rand(OsRng);
        let upper = hash.to_hex_string().to_uppercase();
        assert_eq!(AddressHash::new_from_hex_string(&upper).expect("uppercase hex parses"), hash);
    }

    #[test]
    fn short_is_first_eight_hex_chars() {
        let hash = AddressHash::new_from_rand(OsRng);
        assert_eq!(hash.short(), hash.to_hex_string()[..8]);
        assert_eq!(hash.short().len(), 8);
    }
}
