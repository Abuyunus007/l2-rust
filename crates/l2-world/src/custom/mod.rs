//! The Java custom features of `config/Custom/*.ini` (D-57), one file per feature. Each is off by
//! default, as in Java, and switched by the keys of its own ini file.
//!
//! M9-C06: the screen welcome message, `.online`, `.changepassword`, the multilingual support,
//! the custom mail manager, the dual-box check, the captcha, the chat moderation commands, the
//! L2Walker protection, the Hellbound command switch and the banking commands (`misc.rs` has the
//! switches and the hooks of these).

pub mod banking;
pub mod captcha;
pub mod captcha_image;
pub mod chat_admin;
pub mod custom_mail;
pub mod dualbox;
pub mod misc;
pub mod multilingual;
pub mod online_info;
pub mod password_change;
pub mod screen_welcome;
pub mod walker_bot;

pub use custom_mail::CustomMailEntry;
pub use misc::{
    BankingSettings, CaptchaSettings, CustomMiscSettings, CustomMiscState, CustomMiscTimer, DualboxSettings,
    MultilangSettings, ScreenWelcomeSettings,
};
