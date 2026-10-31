//! The admin secrets nobody has to type (D-56).
//!
//! The supervisor's own admin API needs no token on a local `Listen` (see
//! `l2_admin_api::Token::local_only`). The servers it starts get the token of their admin link
//! through [`ADMIN_TOKEN_ENV`], made fresh at every supervisor start, so their `Admin.ini` needs
//! no token either.

pub use l2_admin_api::ADMIN_TOKEN_ENV;

/// 64 hex digits from the thread RNG (seeded from the OS).
pub fn random_token() -> String {
    (0..4)
        .map(|_| format!("{:016x}", rand::random::<u64>()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_hex_and_differ() {
        let token = random_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, random_token());
    }
}
