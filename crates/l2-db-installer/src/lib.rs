//! Database installer (Rust port of the L2JMobius `DatabaseInstaller`).
// `splitter` and `installer` are the logic; `console` and `gui` are the two front ends;
// `paths` finds the SQL folder and `Interface.ini`.

pub mod background;
pub mod console;
pub mod gui;
pub mod installer;
pub mod paths;
pub mod splitter;
