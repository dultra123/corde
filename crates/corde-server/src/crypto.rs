//! The few cryptographic primitives the home server needs, from crates already in the
//! dependency tree: `ring` (system random generator, SHA-256) and `subtle` (constant-time
//! comparison). No home-made algorithm.

use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use subtle::ConstantTimeEq;

/// `n` bytes from the operating system's random generator (`getrandom`).
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    // The system generator only fails when the OS has none at all; nothing sensible can go on.
    SystemRandom::new()
        .fill(&mut buf)
        .expect("the operating system has no random generator");
    buf
}

/// `n` random bytes as lower-case hex (`2n` characters).
pub fn random_hex(n: usize) -> String {
    hex(&random_bytes(n))
}

/// A uniformly random number in `0..bound` (rejection sampling, no modulo bias).
pub fn random_below(bound: u32) -> u32 {
    assert!(bound > 0);
    let limit = u32::MAX - (u32::MAX % bound);
    loop {
        let b = random_bytes(4);
        let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if v < limit {
            return v % bound;
        }
    }
}

/// Lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    s
}

/// SHA-256 of `data`, lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(digest::digest(&digest::SHA256, data).as_ref())
}

/// Incremental SHA-256 (files).
pub struct Sha256(digest::Context);

impl Default for Sha256 {
    fn default() -> Self {
        Self(digest::Context::new(&digest::SHA256))
    }
}

impl Sha256 {
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }
    pub fn finish_hex(self) -> String {
        hex(self.0.finish().as_ref())
    }
}

/// SHA-256 of a whole file, lower-case hex.
pub fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::default();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish_hex())
}

/// `a == b` in time that does not depend on where they differ (lengths may leak).
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut h = Sha256::default();
        h.update(b"a");
        h.update(b"bc");
        assert_eq!(h.finish_hex(), sha256_hex(b"abc"));
        let a = random_hex(32);
        let b = random_hex(32);
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(ct_eq(b"same", b"same"));
        assert!(!ct_eq(b"same", b"Same"));
        assert!(!ct_eq(b"same", b"same!"));
        for _ in 0..1000 {
            assert!(random_below(10) < 10);
        }
    }
}
