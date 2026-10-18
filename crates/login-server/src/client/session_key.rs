//! The four random ints that tie a client's login to its game server login.
//!
//! Java: `loginserver/controller/SessionKey.java`.

use l2_util::WorldRng;

/// `LoginOk` sends the login pair, `PlayOk` the play pair. The game server later sends all four
/// back to the login server (`PlayerAuthRequest`) to prove the client logged in here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub login_ok1: i32,
    pub login_ok2: i32,
    pub play_ok1: i32,
    pub play_ok2: i32,
}

impl SessionKey {
    // Java: LoginController.assignSessionKeyToClient (4 × Rnd.nextInt, in this order)
    pub fn random(rng: &mut WorldRng) -> Self {
        Self {
            login_ok1: rng.next_i32(),
            login_ok2: rng.next_i32(),
            play_ok1: rng.next_i32(),
            play_ok2: rng.next_i32(),
        }
    }

    // Java: SessionKey.checkLoginPair
    pub fn check_login_pair(&self, login_ok1: i32, login_ok2: i32) -> bool {
        self.login_ok1 == login_ok1 && self.login_ok2 == login_ok2
    }

    /// Java `SessionKey.equals`: the play pair always, the login pair only when `ShowLicence`
    /// is on (otherwise the client never received it).
    pub fn matches(&self, other: &SessionKey, show_licence: bool) -> bool {
        self.play_ok1 == other.play_ok1
            && self.play_ok2 == other.play_ok2
            && (!show_licence || self.check_login_pair(other.login_ok1, other.login_ok2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_pair_is_only_compared_with_licence() {
        let a = SessionKey {
            login_ok1: 1,
            login_ok2: 2,
            play_ok1: 3,
            play_ok2: 4,
        };
        let other_login = SessionKey { login_ok1: 9, ..a };
        let other_play = SessionKey { play_ok2: 9, ..a };
        assert!(a.matches(&a, true));
        assert!(!a.matches(&other_login, true));
        assert!(a.matches(&other_login, false));
        assert!(!a.matches(&other_play, false));
    }

    #[test]
    fn random_keys_differ() {
        let mut rng = WorldRng::from_seed(5);
        assert_ne!(SessionKey::random(&mut rng), SessionKey::random(&mut rng));
    }
}
