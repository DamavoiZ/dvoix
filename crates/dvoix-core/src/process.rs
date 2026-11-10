//! Helpers for running programs on behalf of store plugins (the `process`
//! permission): finding a program on `PATH` the way a shell does, and turning
//! the bytes of its output into text without splitting a character.
//!
//! Starting and stopping processes lives in the Tauri shell; this part has no
//! side effects and is tested here.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Finds `command` like a shell would. A name with a path separator is taken
/// as it is (relative ones against `cwd`); a plain name is searched in the
/// folders of `path`. On Windows `pathext` lists the extensions tried for a
/// name without one, because tools installed with npm are `.cmd` files that
/// `CreateProcess` does not find on its own.
pub fn resolve_program(
    command: &str,
    cwd: &Path,
    path: Option<&OsStr>,
    pathext: Option<&str>,
) -> Option<PathBuf> {
    if command.is_empty() {
        return None;
    }
    let candidates = |base: PathBuf| -> Vec<PathBuf> {
        let mut out = vec![];
        if let Some(exts) = pathext
            && base.extension().is_none()
        {
            for ext in exts.split(';').filter(|e| !e.is_empty()) {
                let mut name = base.clone().into_os_string();
                name.push(ext.to_ascii_lowercase());
                out.push(PathBuf::from(name));
            }
        }
        out.push(base);
        out
    };

    if command.contains('/') || command.contains('\\') {
        let p = Path::new(command);
        let base = if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        };
        return candidates(base).into_iter().find(|c| c.is_file());
    }
    let path = path?;
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| candidates(dir.join(command)))
        .find(|c| c.is_file())
}

/// Turns chunks of output into text. A read can end inside a multi-byte
/// character; those bytes are kept back until the next chunk completes them.
/// Bytes that are not UTF-8 at all become U+FFFD, like `from_utf8_lossy`.
#[derive(Default)]
pub struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let keep = incomplete_tail(&self.pending);
        let split = self.pending.len() - keep;
        let text = String::from_utf8_lossy(&self.pending[..split]).into_owned();
        self.pending.drain(..split);
        text
    }

    /// What is left at the end of the output.
    pub fn finish(&mut self) -> String {
        let text = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        text
    }
}

/// Number of bytes at the end that start a character but don't finish it.
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=bytes.len().min(3) {
        let b = bytes[bytes.len() - back];
        if b & 0xC0 == 0x80 {
            continue; // continuation byte, look further back
        }
        let needed = match b {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => 1,
        };
        return if needed > back { back } else { 0 };
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn decoder_keeps_split_characters() {
        let mut d = Utf8Decoder::default();
        let bytes = "aä€😀".as_bytes();
        let mut out = String::new();
        for b in bytes {
            out.push_str(&d.push(std::slice::from_ref(b)));
        }
        out.push_str(&d.finish());
        assert_eq!(out, "aä€😀");
        assert_eq!(d.push(&[0xFF, b'x']), "\u{FFFD}x");
    }

    #[test]
    fn resolves_on_path_with_pathext() {
        let dir = tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        fs::write(bin.join("tool.cmd"), "").expect("write");
        fs::write(bin.join("plain"), "").expect("write");
        let path = std::env::join_paths([bin.clone()]).expect("join");

        let found = resolve_program("tool", dir.path(), Some(&path), Some(".EXE;.CMD"));
        assert_eq!(found, Some(bin.join("tool.cmd")));
        assert_eq!(
            resolve_program("plain", dir.path(), Some(&path), None),
            Some(bin.join("plain"))
        );
        assert_eq!(
            resolve_program("missing", dir.path(), Some(&path), None),
            None
        );
        assert_eq!(
            resolve_program("bin/plain", dir.path(), None, None),
            Some(dir.path().join("bin/plain"))
        );
        assert_eq!(resolve_program("", dir.path(), Some(&path), None), None);
    }
}
