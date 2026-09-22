//! KMST 1206 hash-version derivation, ported from WzComparerR2 `WzVersionHasher.ComputePkg2HashVersion1206`.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const KMST1206_VERSION_STRING: &str = "v410_260106_1_A1F3C9E2";

/// KMST1206 derives its hash version with two chained HMAC-SHA256 operations.
pub fn compute_pkg2_hash_version_1206(version: &str) -> u64 {
    const KEY1: [u8; 32] = [
        0xDE, 0x17, 0x00, 0xB9, 0x5C, 0x7C, 0xF2, 0xFE, 0x10, 0x11, 0x5B, 0x6E, 0xE3, 0xE0, 0xE9,
        0x84, 0xA8, 0x35, 0x3C, 0xF4, 0x21, 0x87, 0xD8, 0x52, 0x52, 0xCA, 0x5E, 0x0E, 0x71, 0x3C,
        0x25, 0x87,
    ];
    const KEY2: [u8; 17] = [
        0xEC, 0xEB, 0x71, 0xCE, 0x04, 0xCD, 0x3B, 0xB3, 0xEE, 0xAD, 0x10, 0x78, 0x0E, 0x2F, 0x58,
        0x0A, 0x01,
    ];

    let utf16_le: Vec<u8> = version.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut first = Hmac::<Sha256>::new_from_slice(&KEY1).expect("fixed HMAC key");
    first.update(&utf16_le);
    let first_hash = first.finalize().into_bytes();

    let mut second = Hmac::<Sha256>::new_from_slice(&first_hash).expect("SHA-256 HMAC key");
    second.update(&KEY2);
    let second_hash = second.finalize().into_bytes();
    u64::from_le_bytes(second_hash[..8].try_into().expect("SHA-256 output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_pkg2_hash_version_1206_yields_known_seed() {
        assert_eq!(
            compute_pkg2_hash_version_1206(KMST1206_VERSION_STRING),
            0xB804_7BAB_2410_606A
        );
    }
}
