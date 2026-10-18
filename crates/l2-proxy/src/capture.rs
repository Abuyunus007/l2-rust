//! The capture file format: JSON lines, one packet per line, written by the proxy and read by
//! the GUI's packet inspector and by tests (D-34). Captures double as test fixtures, so
//! credentials never end up in them.
//!
//! ```text
//! {"format":"l2-capture","version":1,"tool":"l2-proxy 0.1.0","created_ms":1760000000000}
//! {"ts_ms":1760000000123,"conn":1,"dir":"s2c","channel":"login","opcode":0,"name":"Init","fields":{...},"raw":"00ab.."}
//! ```
//!
//! - `ts_ms`: Unix time in milliseconds when the proxy saw the packet.
//! - `conn`: connection number, unique per proxy run. A login connection and its game
//!   connection are different numbers.
//! - `dir`: `c2s` (client to server) or `s2c`.
//! - `channel`: `login` (client <-> login server), `game` (client <-> game server), `link`
//!   (login server <-> game server). `login` and `game` are produced so far.
//! - `opcode`: the first byte of the plaintext payload. `ex_opcode` is only present for game
//!   packets with a 16-bit sub-opcode (client `0xD0`, server `0xFE`).
//! - `name`: the packet name, or `"unknown"`.
//! - `fields`: the decoded fields as a JSON object, or `null` when the packet is not decoded.
//! - `raw`: lowercase hex of the plaintext payload (opcode included, without length header,
//!   padding or checksum). **Redacted packets** (`RequestAuthLogin`) carry only the opcode byte
//!   here and `"<redacted>"` in `fields`.
//!
//! The reader accepts unknown extra keys, so the format can grow without breaking old readers;
//! it rejects a different `format` string or a newer `version`.

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// The `format` string of the header line.
pub const FORMAT: &str = "l2-capture";
/// The capture format version this crate writes and reads.
pub const VERSION: u32 = 1;
/// Placeholder written instead of credentials.
pub const REDACTED: &str = "<redacted>";

/// Errors while reading or writing a capture.
#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("capture line {line} is not valid: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("not an {FORMAT} file (header says {found:?})")]
    WrongFormat { found: String },

    #[error("capture version {found} is newer than the supported version {VERSION}")]
    TooNew { found: u32 },

    #[error("the capture is empty: the header line is missing")]
    MissingHeader,
}

/// The first line of every capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub format: String,
    pub version: u32,
    /// Who wrote the file, e.g. `l2-proxy 0.1.0`.
    pub tool: String,
    pub created_ms: u64,
}

impl Header {
    /// A header for a capture created now by this crate.
    pub fn new() -> Self {
        Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            tool: format!("l2-proxy {}", env!("CARGO_PKG_VERSION")),
            created_ms: now_ms(),
        }
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

/// Direction of a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Client to server.
    C2s,
    /// Server to client.
    S2c,
}

/// Which protocol a packet belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Login,
    Game,
    Link,
}

/// One captured packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub ts_ms: u64,
    pub conn: u64,
    pub dir: Dir,
    pub channel: Channel,
    pub opcode: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ex_opcode: Option<u16>,
    pub name: String,
    pub fields: Option<Value>,
    pub raw: String,
}

impl Record {
    /// A record stamped with the current time. `raw` is hex-encoded from the plaintext bytes.
    pub fn new(
        conn: u64,
        dir: Dir,
        channel: Channel,
        opcode: u8,
        name: impl Into<String>,
        fields: Option<Value>,
        raw: &[u8],
    ) -> Self {
        Self {
            ts_ms: now_ms(),
            conn,
            dir,
            channel,
            opcode,
            ex_opcode: None,
            name: name.into(),
            fields,
            raw: hex::encode(raw),
        }
    }

    /// The raw payload bytes, if `raw` is valid hex.
    pub fn raw_bytes(&self) -> Result<Vec<u8>, hex::FromHexError> {
        hex::decode(&self.raw)
    }
}

/// Unix time in milliseconds (0 if the clock is before 1970).
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Writes a capture. Every record is flushed right away, so a crash or a Ctrl+C loses nothing
/// and a GUI can tail the file.
pub struct CaptureWriter<W: Write> {
    out: W,
}

impl CaptureWriter<BufWriter<File>> {
    /// Creates (or truncates) `path` and writes the header.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, CaptureError> {
        Self::new(BufWriter::new(File::create(path)?))
    }
}

impl<W: Write> CaptureWriter<W> {
    /// Writes the header line to `out`.
    pub fn new(mut out: W) -> Result<Self, CaptureError> {
        write_json_line(&mut out, &Header::new())?;
        Ok(Self { out })
    }

    pub fn write_record(&mut self, record: &Record) -> Result<(), CaptureError> {
        write_json_line(&mut self.out, record)
    }

    /// Gives the underlying writer back.
    pub fn into_inner(self) -> W {
        self.out
    }
}

fn write_json_line<W: Write, T: Serialize>(out: &mut W, value: &T) -> Result<(), CaptureError> {
    // Serializing plain structs cannot fail; map the error anyway instead of unwrapping.
    let mut line = serde_json::to_vec(value).map_err(|e| CaptureError::Json { line: 0, source: e })?;
    line.push(b'\n');
    out.write_all(&line)?;
    out.flush()?;
    Ok(())
}

/// Reads a capture: checks the header, then yields the records one by one.
pub struct CaptureReader<R: BufRead> {
    lines: std::io::Lines<R>,
    header: Header,
    line_no: usize,
}

impl CaptureReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CaptureError> {
        Self::new(BufReader::new(File::open(path)?))
    }
}

impl<R: BufRead> CaptureReader<R> {
    pub fn new(input: R) -> Result<Self, CaptureError> {
        let mut lines = input.lines();
        let first = lines.next().ok_or(CaptureError::MissingHeader)??;
        let header: Header =
            serde_json::from_str(&first).map_err(|e| CaptureError::Json { line: 1, source: e })?;
        if header.format != FORMAT {
            return Err(CaptureError::WrongFormat { found: header.format });
        }
        if header.version > VERSION {
            return Err(CaptureError::TooNew {
                found: header.version,
            });
        }
        Ok(Self {
            lines,
            header,
            line_no: 1,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Reads every remaining record.
    pub fn read_all(self) -> Result<Vec<Record>, CaptureError> {
        self.collect()
    }
}

impl<R: BufRead> Iterator for CaptureReader<R> {
    type Item = Result<Record, CaptureError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = match self.lines.next()? {
                Ok(line) => line,
                Err(e) => return Some(Err(e.into())),
            };
            self.line_no += 1;
            if line.trim().is_empty() {
                continue;
            }
            return Some(serde_json::from_str(&line).map_err(|e| CaptureError::Json {
                line: self.line_no,
                source: e,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample(n: u8) -> Record {
        Record::new(
            7,
            if n % 2 == 0 { Dir::C2s } else { Dir::S2c },
            Channel::Login,
            n,
            format!("Packet{n}"),
            Some(json!({ "a": n, "nested": { "list": [1, 2, 3] } })),
            &[n, 0xab, 0xcd],
        )
    }

    fn write_all(records: &[Record]) -> Vec<u8> {
        let mut w = CaptureWriter::new(Vec::new()).unwrap();
        for r in records {
            w.write_record(r).unwrap();
        }
        w.into_inner()
    }

    #[test]
    fn round_trip() {
        let records: Vec<Record> = (0..5).map(sample).collect();
        let bytes = write_all(&records);
        let reader = CaptureReader::new(bytes.as_slice()).unwrap();
        assert_eq!(reader.header().format, FORMAT);
        assert_eq!(reader.header().version, VERSION);
        assert_eq!(reader.read_all().unwrap(), records);
    }

    #[test]
    fn one_json_object_per_line_with_the_documented_keys() {
        let mut record = sample(1);
        record.fields = None;
        let text = String::from_utf8(write_all(&[record])).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let header: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["format"], "l2-capture");
        assert_eq!(header["version"], 1);
        let rec: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(rec["dir"], "s2c");
        assert_eq!(rec["channel"], "login");
        assert_eq!(rec["opcode"], 1);
        assert_eq!(rec["name"], "Packet1");
        assert!(rec["fields"].is_null(), "fields is null, not missing");
        assert_eq!(rec["raw"], "01abcd");
        assert!(rec.get("ex_opcode").is_none(), "ex_opcode is omitted when absent");
    }

    #[test]
    fn ex_opcode_round_trips() {
        let mut record = sample(0xd0);
        record.ex_opcode = Some(0x0051);
        let bytes = write_all(std::slice::from_ref(&record));
        let read = CaptureReader::new(bytes.as_slice()).unwrap().read_all().unwrap();
        assert_eq!(read[0].ex_opcode, Some(0x51));
    }

    #[test]
    fn raw_bytes_decodes_the_hex() {
        assert_eq!(sample(9).raw_bytes().unwrap(), vec![9, 0xab, 0xcd]);
    }

    #[test]
    fn unknown_keys_and_blank_lines_are_tolerated() {
        let text = concat!(
            "{\"format\":\"l2-capture\",\"version\":1,\"tool\":\"x\",\"created_ms\":1,\"extra\":true}\n",
            "\n",
            "{\"ts_ms\":1,\"conn\":2,\"dir\":\"c2s\",\"channel\":\"game\",\"opcode\":3,\"name\":\"n\",",
            "\"fields\":null,\"raw\":\"03\",\"future\":1}\n",
        );
        let records = CaptureReader::new(text.as_bytes()).unwrap().read_all().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].channel, Channel::Game);
    }

    #[test]
    fn bad_headers_are_rejected() {
        assert!(matches!(
            CaptureReader::new(&b""[..]),
            Err(CaptureError::MissingHeader)
        ));
        let wrong = "{\"format\":\"other\",\"version\":1,\"tool\":\"x\",\"created_ms\":1}\n";
        assert!(matches!(
            CaptureReader::new(wrong.as_bytes()),
            Err(CaptureError::WrongFormat { .. })
        ));
        let newer = "{\"format\":\"l2-capture\",\"version\":99,\"tool\":\"x\",\"created_ms\":1}\n";
        assert!(matches!(
            CaptureReader::new(newer.as_bytes()),
            Err(CaptureError::TooNew { found: 99 })
        ));
        assert!(matches!(
            CaptureReader::new(&b"not json\n"[..]),
            Err(CaptureError::Json { line: 1, .. })
        ));
    }

    #[test]
    fn a_broken_record_names_its_line() {
        let mut bytes = write_all(&[sample(1)]);
        bytes.extend_from_slice(b"{\"oops\":1}\n");
        let results: Vec<_> = CaptureReader::new(bytes.as_slice()).unwrap().collect();
        assert!(results[0].is_ok());
        assert!(matches!(results[1], Err(CaptureError::Json { line: 3, .. })));
    }

    #[test]
    fn files_round_trip_and_are_readable_while_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.jsonl");
        let mut w = CaptureWriter::create(&path).unwrap();
        let record = sample(4);
        w.write_record(&record).unwrap();
        // Flushed per record: readable while the writer is still open.
        let records = CaptureReader::open(&path).unwrap().read_all().unwrap();
        assert_eq!(records, vec![record]);
        drop(w);
    }
}
