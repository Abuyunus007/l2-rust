//! `l2-setup`: makes a ready-to-run server folder from this repository (the built programs, the
//! datapack and settings of `dist/`, the geodata) and sets up its database: a new one with the
//! installer's scripts, or an existing L2JMobius H5 database brought up to the schema without
//! losing data ([`migrate`]). `setup.cmd` / `setup.sh` in the repository build the programs and
//! run it.

pub mod dbini;
pub mod layout;
pub mod migrate;
