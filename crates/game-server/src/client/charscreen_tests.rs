//! Small checks of the helpers of the character screen.

use l2_config::GameConfig;

use super::charscreen::{game_time_minutes, is_alpha_numeric, is_valid_char_name};

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
fn a_name_must_also_match_the_cname_template() {
    // The default `.*` accepts any name that is letters and digits.
    assert!(is_valid_char_name(&GameConfig::default(), "Hero"));

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Server.ini"), "CnameTemplate = [A-Z][a-z]*\n").unwrap();
    let config = GameConfig::load(dir.path()).unwrap();
    assert!(is_valid_char_name(&config, "Hero"));
    // Refused by the template, although every character is a letter or a digit.
    assert!(!is_valid_char_name(&config, "hero"));
    assert!(!is_valid_char_name(&config, "Hero2"));
    // Refused by the letters-and-digits rule.
    assert!(!is_valid_char_name(&config, "He ro"));
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
