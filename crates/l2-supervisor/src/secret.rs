//! The admin secrets nobody has to type (D-56).
//!
//! With `Token` empty in `supervisor.ini`, the supervisor and the GUI share a random token kept
//! in `supervisor.token` next to the ini file: whoever needs it first creates it. The servers the
//! supervisor starts get the token of their admin link through [`ADMIN_TOKEN_ENV`], so their
//! `Admin.ini` needs no token either.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The token file, next to `supervisor.ini`.
pub const TOKEN_FILE: &str = "supervisor.token";

pub use l2_admin_api::ADMIN_TOKEN_ENV;

/// `supervisor.token` in `dir`.
pub fn token_path(dir: &Path) -> PathBuf {
    dir.join(TOKEN_FILE)
}

/// 64 hex digits from the thread RNG (seeded from the OS).
pub fn random_token() -> String {
    (0..4)
        .map(|_| format!("{:016x}", rand::random::<u64>()))
        .collect()
}

/// The token in `dir/supervisor.token`, created when missing or empty. Two processes racing on a
/// fresh folder end up with the same token: `create_new` lets only one write it.
pub fn load_or_create(dir: &Path) -> io::Result<String> {
    let path = token_path(dir);
    for _ in 0..2 {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let token = text.trim();
            if !token.is_empty() {
                return Ok(token.to_string());
            }
            // An empty file (a crash between create and write): start over.
            std::fs::remove_file(&path)?;
        }
        let token = random_token();
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(token.as_bytes())?;
                return Ok(token);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other(format!("cannot create {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once_and_then_reads_the_same_token() {
        let dir = tempfile::tempdir().unwrap();
        let first = load_or_create(dir.path()).unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(load_or_create(dir.path()).unwrap(), first);
    }

    #[test]
    fn an_empty_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(token_path(dir.path()), "  \n").unwrap();
        let token = load_or_create(dir.path()).unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(std::fs::read_to_string(token_path(dir.path())).unwrap(), token);
    }

    #[test]
    fn tokens_differ() {
        assert_ne!(random_token(), random_token());
    }
}
