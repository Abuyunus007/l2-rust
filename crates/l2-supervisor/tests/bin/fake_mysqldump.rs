//! Stand-in for `mysqldump` in the backup tests. Not part of the product, never touches a database.
//!
//! Writes a known dump to stdout that lists its arguments and whether `MYSQL_PWD` was set, so the
//! tests can check how it was called. A database called `failing` makes it fail like a refused
//! login would.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.last().map(String::as_str) == Some("failing") {
        eprintln!("mysqldump: Got error: 1045: Access denied for user (using password: YES)");
        std::process::exit(2);
    }
    println!("-- fake mysqldump");
    println!("-- args: {}", args.join(" "));
    println!("-- MYSQL_PWD: {}", std::env::var("MYSQL_PWD").unwrap_or_default());
    println!("CREATE TABLE t (id int);");
}
