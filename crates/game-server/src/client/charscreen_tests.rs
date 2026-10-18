//! Small checks of the helpers of the character screen.

use super::charscreen::{NameTemplate, game_time_minutes, is_alpha_numeric};

#[test]
fn alphanumeric_means_letters_and_digits_only() {
    assert!(is_alpha_numeric("Hero123"));
    assert!(is_alpha_numeric("Zoë"));
    assert!(!is_alpha_numeric(""));
    assert!(!is_alpha_numeric("a b"));
    assert!(!is_alpha_numeric("a_b"));
    assert!(!is_alpha_numeric("a-b"));
    // A character outside the BMP is two surrogates in Java, and neither is a letter.
    assert!(!is_alpha_numeric("a\u{1F600}"));
}

#[test]
fn only_the_default_name_template_is_understood() {
    assert!(NameTemplate::parse(".*").matches("anything"));
    let strict = NameTemplate::parse("[A-Z][a-z]*");
    assert_eq!(strict, NameTemplate::Unsupported("[A-Z][a-z]*".into()));
    assert!(!strict.matches("Hero"));
}

#[test]
fn the_game_clock_runs_six_times_faster_than_the_real_one() {
    // One game minute is 10 real seconds; a game day (1440 minutes) is 4 real hours.
    assert_eq!(game_time_minutes(0), 0);
    assert_eq!(game_time_minutes(10_000), 1);
    assert_eq!(game_time_minutes(1_000_000), 100);
    assert_eq!(game_time_minutes(14_400_000), 0);
    assert_eq!(game_time_minutes(14_410_000), 1);
}
