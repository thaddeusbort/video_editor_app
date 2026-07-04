//! Content hashing for `SourceFile::hash`.
//!
//! This is deliberately *not* a full-file SHA. Vacation footage folders can
//! hold hundreds of gigabytes of 4K/drone footage, and the hash's only job
//! here is to (a) give thumbnails a cache key that survives a file being
//! renamed or moved, and (b) cheaply recognize "this is probably the same
//! file I already imported" on a re-import. Hashing every byte of every
//! clip on every import would make re-imports as slow as the first import.
//!
//! Instead we hash the file size plus the first and last `SAMPLE_BYTES`
//! bytes. Video container formats (MP4/MOV/AVI/...) keep their headers and
//! metadata atoms at the start and/or end of the file, so this is enough to
//! distinguish essentially all real-world files while reading only a few KB
//! per file. It is *not* cryptographically meaningful and two files could in
//! theory collide (e.g. two clips of identical size with identical first/last
//! bytes but different middles) - acceptable for a cache key, not suitable
//! for integrity verification.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SAMPLE_BYTES: u64 = 64 * 1024;

pub fn partial_hash(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();

    let mut hasher = Fnv1a64::new();
    hasher.write_u64(size);

    let head_len = SAMPLE_BYTES.min(size) as usize;
    let mut head = vec![0u8; head_len];
    file.read_exact(&mut head)?;
    hasher.write(&head);

    if size > SAMPLE_BYTES {
        let tail_len = SAMPLE_BYTES.min(size) as usize;
        file.seek(SeekFrom::End(-(tail_len as i64)))?;
        let mut tail = vec![0u8; tail_len];
        file.read_exact(&mut tail)?;
        hasher.write(&tail);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

/// Minimal FNV-1a implementation - no need to pull in a crate just to mix
/// a few KB of bytes into a fixed-size digest for a cache key.
struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn same_content_same_hash() {
        let dir = std::env::temp_dir().join(format!("hash-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.mp4");
        let b = dir.join("b.mp4");
        let mut fa = File::create(&a).unwrap();
        fa.write_all(&vec![7u8; 200_000]).unwrap();
        let mut fb = File::create(&b).unwrap();
        fb.write_all(&vec![7u8; 200_000]).unwrap();

        assert_eq!(partial_hash(&a).unwrap(), partial_hash(&b).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn different_size_different_hash() {
        let dir = std::env::temp_dir().join(format!("hash-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        std::fs::write(&a, vec![1u8; 100]).unwrap();
        std::fs::write(&b, vec![1u8; 101]).unwrap();

        assert_ne!(partial_hash(&a).unwrap(), partial_hash(&b).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn survives_rename() {
        let dir = std::env::temp_dir().join(format!("hash-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let original = dir.join("original.mp4");
        std::fs::write(&original, b"some fake video bytes").unwrap();
        let hash_before = partial_hash(&original).unwrap();

        let renamed = dir.join("renamed.mp4");
        std::fs::rename(&original, &renamed).unwrap();
        let hash_after = partial_hash(&renamed).unwrap();

        assert_eq!(hash_before, hash_after);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_file_does_not_panic() {
        let dir = std::env::temp_dir().join(format!("hash-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty.mp4");
        std::fs::write(&empty, b"").unwrap();
        assert!(partial_hash(&empty).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }
}
