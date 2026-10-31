//! Shared-secret token check.

use std::fmt;

use subtle::ConstantTimeEq;

/// The admin token configured on the listening side.
///
/// An empty configured token rejects everyone: the admin API must not be reachable
/// with an unset secret. The one exception is [`Token::local_only`] (D-56).
#[derive(Clone)]
pub struct Token {
    secret: String,
    /// Accept any connection from this machine without a secret.
    local_open: bool,
}

impl Token {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            local_open: false,
        }
    }

    /// No secret at all for connections from this machine (a listener on a loopback address
    /// with an empty `Token`, D-56): other machines are still refused.
    pub fn local_only() -> Self {
        Self {
            secret: String::new(),
            local_open: true,
        }
    }

    /// Constant-time comparison, so response timing doesn't leak the token.
    pub fn matches(&self, given: &str) -> bool {
        !self.secret.is_empty() && bool::from(self.secret.as_bytes().ct_eq(given.as_bytes()))
    }

    /// Whether a connection from `peer` that sent `given` gets in.
    pub fn allows(&self, given: &str, peer: std::net::IpAddr) -> bool {
        (self.local_open && peer.is_loopback()) || self.matches(given)
    }

    /// The secret, for the connecting side.
    pub fn expose(&self) -> &str {
        &self.secret
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
    fn local_only_lets_this_machine_in_without_a_secret() {
        let t = Token::local_only();
        assert!(t.allows("", "127.0.0.1".parse().unwrap()));
        assert!(t.allows("anything", "::1".parse().unwrap()));
        assert!(!t.allows("", "192.168.1.5".parse().unwrap()));
        assert!(!Token::new("s").allows("", "127.0.0.1".parse().unwrap()));
        assert!(Token::new("s").allows("s", "10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn debug_hides_secret() {
        assert_eq!(format!("{:?}", Token::new("s3cret")), "Token(<redacted>)");
    }
}
