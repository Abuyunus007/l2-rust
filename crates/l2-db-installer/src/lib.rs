//! Database installer (Rust port of the L2JMobius `DatabaseInstaller`).
// Java: org/l2jmobius/tools/DatabaseInstaller.java
//
// `splitter` and `installer` are the logic; `console` and `gui` are the two front ends;
// `paths` finds the SQL folder and `Interface.ini`.

pub mod console;
pub mod gui;
pub mod installer;
pub mod paths;
pub mod splitter;
