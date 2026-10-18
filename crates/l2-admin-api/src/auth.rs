//! Shared-secret token check.

use std::fmt;

use subtle::ConstantTimeEq;

/// The admin token configured on the listening side.
///
/// An empty configured token rejects everyone: the admin API must not be reachable
/// with an unset secret.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Constant-time comparison, so response timing doesn't leak the token.
    pub fn matches(&self, given: &str) -> bool {
        !self.0.is_empty() && bool::from(self.0.as_bytes().ct_eq(given.as_bytes()))
    }

    /// The secret, for the connecting side.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_exact_token() {
        let t = Token::new("s3cret");
        assert!(t.matches("s3cret"));
        assert!(!t.matches("s3cre"));
        assert!(!t.matches("s3cret!"));
        assert!(!t.matches(""));
    }

    #[test]
    fn empty_token_rejects_everyone() {
        assert!(!Token::new("").matches(""));
    }

    #[test]
    fn debug_hides_secret() {
        assert_eq!(format!("{:?}", Token::new("s3cret")), "Token(<redacted>)");
    }
}
