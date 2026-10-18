//! File-level behaviour of `IniFile` and the warnings it logs.

use std::io;
use std::sync::{Arc, Mutex};

use l2_config::{ConfigError, IniFile};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut inner) = self.0.lock() {
            inner.extend_from_slice(buf);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> CaptureWriter {
        CaptureWriter(Arc::clone(&self.0))
    }
}

/// Runs `f` and returns what it logged (no colours, no timestamps).
fn logged(f: impl FnOnce()) -> String {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = capture.0.lock().map(|b| b.clone()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn loads_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Server.ini");
    std::fs::write(&path, "# header\nPort = 7777\nName=Test Server\r\nOn=true\n").unwrap();

    let file = IniFile::load(&path).unwrap();
    assert_eq!(file.file_name(), "Server.ini");
    assert_eq!(file.get_int("Port", 0), 7777);
    assert_eq!(file.get_string("Name", ""), "Test Server");
    assert!(file.get_bool("On", false));
    assert_eq!(file.keys().collect::<Vec<_>>(), ["Name", "On", "Port"]);
}

#[test]
fn missing_file_warns_once_and_uses_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Nope.ini");

    let mut file = None;
    let out = logged(|| file = Some(IniFile::load(&path).unwrap()));
    assert!(out.contains("Configuration file not found"), "{out}");
    assert!(out.contains("Nope.ini"), "{out}");

    let file = file.unwrap();
    assert!(file.is_empty());
    let out = logged(|| assert_eq!(file.get_int("Port", 8080), 8080));
    assert!(
        out.contains("Config 'Port' not found in file 'Nope.ini', using default: 8080."),
        "{out}"
    );
}

#[test]
fn directory_instead_of_file_does_not_fail() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = None;
    let out = logged(|| file = Some(IniFile::load(dir.path()).unwrap()));
    assert!(out.contains("Failed to load configurations from"), "{out}");
    assert!(file.unwrap().is_empty());
}

#[test]
fn invalid_utf8_is_replaced_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Bytes.ini");
    std::fs::write(&path, b"A=ok\nB=x\xFFy\n").unwrap();
    let file = IniFile::load(&path).unwrap();
    assert_eq!(file.get_value("B"), Some("x\u{fffd}y"));
}

#[test]
fn malformed_unicode_escape_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Bad.ini");
    std::fs::write(&path, "A=\\u00\n").unwrap();
    assert_eq!(
        IniFile::load(&path).unwrap_err(),
        ConfigError::MalformedUnicodeEscape {
            file: "Bad.ini".into()
        }
    );
}

#[test]
fn warning_texts_match_java() {
    let file = IniFile::parse("T.ini", "I=abc\nB=nope\nF=x\nD=zz\nA=q\nE=Blue\n").unwrap();

    let out = logged(|| {
        assert_eq!(file.get_int("I", 5), 5);
    });
    assert!(
        out.contains("Invalid int for config 'I' in file 'T.ini', using default: 5."),
        "{out}"
    );

    let out = logged(|| {
        assert_eq!(file.get_float("F", 1.0), 1.0);
    });
    assert!(
        out.contains("Invalid float for config 'F' in file 'T.ini', using default: 1.0."),
        "{out}"
    );

    let out = logged(|| {
        assert!(file.get_duration("D", "5min").is_ok());
    });
    assert!(
        out.contains("Invalid duration for config 'D' in file 'T.ini', using default: 5min."),
        "{out}"
    );

    let out = logged(|| {
        assert_eq!(file.get_int_array("A", ",", "1,2"), [1, 2]);
        assert_eq!(file.get_int_array("none", ",", "1,2"), [1, 2]);
    });
    assert!(
        out.contains("Invalid int array for config 'A' in file 'T.ini', using default values."),
        "{out}"
    );
    assert!(
        out.contains("Config 'none' not found in file 'T.ini', using default values."),
        "{out}"
    );

    // Boolean never warns about a bad value (Boolean.parseBoolean does not throw).
    let out = logged(|| assert!(!file.get_bool("B", true)));
    assert!(out.is_empty(), "{out}");

    // A present key does not warn.
    let out = logged(|| assert_eq!(file.get_string("E", "x"), "Blue"));
    assert!(out.is_empty(), "{out}");
}
