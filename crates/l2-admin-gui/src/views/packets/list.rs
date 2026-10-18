//! The packet store, the filters and the virtual packet list.
//!
//! [`Store`] keeps the records (capped, oldest dropped first) and a list of the ids that pass the
//! current filter. New records are filtered as they arrive; a changed filter re-filters
//! everything once. The list draws only the rows on screen (`show_rows`), so 100k+ records cost
//! the same per frame as 100.

use std::collections::{HashSet, VecDeque};

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};
use l2_proxy::{Channel, Dir, Record};

/// Records kept from a live proxy. The oldest go first (a trim happens in steps of a tenth).
pub const MAX_LIVE_RECORDS: usize = 300_000;

/// A stored record. `id` numbers the records of one store in arrival order and is never reused.
#[derive(Debug)]
pub struct Row {
    pub id: u64,
    pub record: Record,
}

/// Records plus the ids that pass the filter.
#[derive(Debug)]
pub struct Store {
    rows: VecDeque<Row>,
    /// Ids of the rows that pass the filter, ascending.
    visible: Vec<u64>,
    /// Rows with an id below this have been tested against the filter of generation `filter_gen`.
    indexed_until: u64,
    filter_gen: u64,
    next_id: u64,
    cap: usize,
    /// Connection numbers seen so far (also of dropped records).
    connections: HashSet<u64>,
    /// Records ever added (also the dropped ones).
    total: u64,
}

impl Store {
    pub fn new(cap: usize) -> Self {
        Self {
            rows: VecDeque::new(),
            visible: Vec::new(),
            indexed_until: 0,
            filter_gen: 0,
            next_id: 0,
            cap: cap.max(1),
            connections: HashSet::new(),
            total: 0,
        }
    }

    pub fn from_records(records: Vec<Record>) -> Self {
        let mut store = Self::new(usize::MAX);
        store.push_all(records);
        store
    }

    pub fn push_all(&mut self, records: Vec<Record>) {
        for record in records {
            self.connections.insert(record.conn);
            self.rows.push_back(Row {
                id: self.next_id,
                record,
            });
            self.next_id += 1;
            self.total += 1;
        }
        // Trim in steps so the shift of the ids happens rarely, not on every record.
        if self.rows.len() > self.cap.saturating_add(self.cap / 10) {
            let drop = self.rows.len() - self.cap;
            self.rows.drain(..drop);
            let first = self.first_id();
            let keep_from = self.visible.partition_point(|id| *id < first);
            self.visible.drain(..keep_from);
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.visible.clear();
        self.indexed_until = self.next_id;
        self.connections.clear();
        self.total = 0;
    }

    fn first_id(&self) -> u64 {
        self.rows.front().map_or(self.next_id, |r| r.id)
    }

    /// Brings the visible list up to date with `filter` (generation `generation`: bump it
    /// whenever the filter changes). Cheap when nothing changed.
    pub fn refresh(&mut self, filter: &CompiledFilter, generation: u64) {
        if self.filter_gen != generation {
            self.filter_gen = generation;
            self.indexed_until = 0;
            self.visible.clear();
        }
        let start = self.indexed_until.max(self.first_id());
        let skip = usize::try_from(start - self.first_id()).unwrap_or(usize::MAX);
        for row in self.rows.iter().skip(skip) {
            if filter.accepts(&row.record) {
                self.visible.push(row.id);
            }
        }
        self.indexed_until = self.next_id;
    }

    pub fn get(&self, id: u64) -> Option<&Row> {
        let index = usize::try_from(id.checked_sub(self.first_id())?).ok()?;
        self.rows.get(index).filter(|r| r.id == id)
    }

    /// Number of rows that pass the filter.
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    /// The `index`th row that passes the filter.
    pub fn visible_row(&self, index: usize) -> Option<&Row> {
        self.get(*self.visible.get(index)?)
    }

    /// Records kept.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Records added since the start (or the last clear), including dropped ones.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Number of distinct connections seen.
    pub fn connections(&self) -> usize {
        self.connections.len()
    }

    /// Every kept record, oldest first.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.rows.iter().map(|r| &r.record)
    }
}

// ---------------------------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------------------------

/// What the user typed into the filter row. Compare with `==` to see whether it changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub dir: Option<Dir>,
    pub channel: Option<Channel>,
    /// Opcode or name text, see [`CompiledFilter`].
    pub text: String,
    pub hide_noisy: bool,
    /// The noisy list as typed (names or hex opcodes, separated by commas or spaces).
    pub noisy: String,
}

/// A `0x..` opcode written as a filter term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Code {
    /// The first byte of the packet.
    Opcode(u8),
    /// The `u16` sub-opcode of an ex packet.
    Ex(u16),
}

impl Code {
    /// `0x59` is an opcode, `0x0036` (above `0xFF`, or four digits) an ex opcode. With
    /// `bare_ok`, plain hex digits count too: two digits an opcode, four an ex opcode.
    fn parse(term: &str, bare_ok: bool) -> Option<Self> {
        let (digits, prefixed) = match term.strip_prefix("0x") {
            Some(rest) => (rest, true),
            None => (term, false),
        };
        if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        if !prefixed && !(bare_ok && matches!(digits.len(), 2 | 4)) {
            return None;
        }
        let value = u16::from_str_radix(digits, 16).ok()?;
        if digits.len() <= 2 {
            u8::try_from(value).ok().map(Code::Opcode)
        } else {
            Some(Code::Ex(value))
        }
    }

    fn matches(self, record: &Record) -> bool {
        match self {
            Code::Opcode(op) => record.opcode == op,
            Code::Ex(ex) => record.ex_opcode == Some(ex),
        }
    }
}

/// `MOVE_TO_LOCATION`, `MoveToLocation` and `move to location` all become `movetolocation`.
fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '_' | ' ' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// One entry of the noisy list: a whole name, or an opcode.
#[derive(Debug, PartialEq, Eq)]
enum Noisy {
    Name(String),
    Code(Code),
}

fn parse_noisy(list: &str) -> Vec<Noisy> {
    list.split([',', ';', ' ', '\n', '\t'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|term| {
            let lower = term.to_lowercase();
            match Code::parse(&lower, false) {
                Some(code) => Noisy::Code(code),
                None => Noisy::Name(normalize(term)),
            }
        })
        .collect()
}

/// A [`Filter`] prepared for testing many records: the text is normalised once.
///
/// The text matches when it is part of the packet name (ignoring case and `_`), or when it is
/// the packet's hex opcode (`59`, `0x59`) or ex opcode (`0036`, `0x36`). The noisy list matches
/// whole names or `0x` opcodes.
#[derive(Debug)]
pub struct CompiledFilter {
    dir: Option<Dir>,
    channel: Option<Channel>,
    text: String,
    code: Option<Code>,
    noisy: Vec<Noisy>,
}

impl CompiledFilter {
    pub fn new(filter: &Filter) -> Self {
        let text = filter.text.trim().to_lowercase();
        Self {
            dir: filter.dir,
            channel: filter.channel,
            code: Code::parse(&text, true),
            text: normalize(&text),
            noisy: if filter.hide_noisy {
                parse_noisy(&filter.noisy)
            } else {
                Vec::new()
            },
        }
    }

    pub fn accepts(&self, record: &Record) -> bool {
        if self.dir.is_some_and(|d| d != record.dir) || self.channel.is_some_and(|c| c != record.channel) {
            return false;
        }
        if !self.noisy.is_empty() && self.is_noisy(record) {
            return false;
        }
        if self.text.is_empty() {
            return true;
        }
        self.code.is_some_and(|c| c.matches(record)) || normalize(&record.name).contains(&self.text)
    }

    fn is_noisy(&self, record: &Record) -> bool {
        let name = normalize(&record.name);
        self.noisy.iter().any(|n| match n {
            Noisy::Name(wanted) => *wanted == name,
            Noisy::Code(code) => code.matches(record),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------------------------

/// Time of day in UTC (`HH:MM:SS.mmm`); the GUI has no time zone support.
pub fn format_time(ts_ms: u64) -> String {
    let ms_of_day = ts_ms % 86_400_000;
    let h = ms_of_day / 3_600_000;
    let m = ms_of_day / 60_000 % 60;
    let s = ms_of_day / 1_000 % 60;
    let ms = ms_of_day % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// `0x2b`, or `0xd0:0x0036` for an ex packet.
pub fn format_opcode(record: &Record) -> String {
    match record.ex_opcode {
        Some(ex) => format!("0x{:02x}:0x{ex:04x}", record.opcode),
        None => format!("0x{:02x}", record.opcode),
    }
}

/// Payload size in bytes (a redacted packet only has its opcode byte, and says so in `fields`).
pub fn payload_len(record: &Record) -> usize {
    record.raw.len() / 2
}

pub fn dir_arrow(dir: Dir) -> &'static str {
    match dir {
        Dir::C2s => "→",
        Dir::S2c => "←",
    }
}

pub fn dir_label(dir: Dir) -> &'static str {
    match dir {
        Dir::C2s => "client to server",
        Dir::S2c => "server to client",
    }
}

pub fn channel_label(channel: Channel) -> &'static str {
    match channel {
        Channel::Login => "login",
        Channel::Game => "game",
        Channel::Link => "link",
    }
}

fn dir_color(dir: Dir, dark: bool) -> Color32 {
    match (dir, dark) {
        (Dir::C2s, true) => Color32::from_rgb(0x6c, 0xb4, 0xf5),
        (Dir::C2s, false) => Color32::from_rgb(0x1a, 0x5f, 0xb4),
        (Dir::S2c, true) => Color32::from_rgb(0x6c, 0xd0, 0x8a),
        (Dir::S2c, false) => Color32::from_rgb(0x1e, 0x7a, 0x3c),
    }
}

/// Column widths in characters (the list uses a monospace font, so padding makes columns).
const W_TIME: usize = 12;
const W_CONN: usize = 5;
const W_CHANNEL: usize = 5;
const W_OPCODE: usize = 11;
const W_NAME: usize = 40;
const W_SIZE: usize = 6;

/// The header above the list, aligned with [`row_job`].
pub fn header_text() -> String {
    format!(
        "{:<W_TIME$} {:>W_CONN$}    {:<W_CHANNEL$} {:<W_OPCODE$} {:<W_NAME$} {:>W_SIZE$}  state",
        "time (UTC)", "conn", "chan", "opcode", "name", "size"
    )
}

/// One list row: monospace columns, the direction arrow and its neighbours coloured.
pub fn row_job(record: &Record, dark: bool, text_color: Color32) -> LayoutJob {
    let font = FontId::monospace(13.0);
    let plain = TextFormat::simple(font.clone(), text_color);
    let colored = TextFormat::simple(font, dir_color(record.dir, dark));
    let mut job = LayoutJob::default();
    job.append(
        &format!("{:<W_TIME$} {:>W_CONN$} ", format_time(record.ts_ms), record.conn),
        0.0,
        plain.clone(),
    );
    job.append(&format!("{}  ", dir_arrow(record.dir)), 0.0, colored.clone());
    job.append(
        &format!(
            "{:<W_CHANNEL$} {:<W_OPCODE$} ",
            channel_label(record.channel),
            format_opcode(record)
        ),
        0.0,
        plain.clone(),
    );
    job.append(&format!("{:<W_NAME$} ", record.name), 0.0, colored);
    job.append(
        &format!(
            "{:>W_SIZE$}  {}",
            payload_len(record),
            record.state.as_deref().unwrap_or("")
        ),
        0.0,
        plain,
    );
    job
}

/// Draws the header and the virtual list. Returns the id of the row clicked this frame.
pub fn list_ui(ui: &mut egui::Ui, store: &Store, selected: Option<u64>) -> Option<u64> {
    let dark = ui.visuals().dark_mode;
    let text_color = ui.visuals().text_color();
    let row_height = ui.spacing().interact_size.y;
    ui.label(egui::RichText::new(header_text()).monospace().weak());

    let mut clicked = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        // Follows the newest packet until the user scrolls up; scrolling back to the bottom
        // resumes following.
        .stick_to_bottom(true)
        .show_rows(ui, row_height, store.visible_len(), |ui, range| {
            for index in range {
                let Some(row) = store.visible_row(index) else {
                    continue;
                };
                let job = row_job(&row.record, dark, text_color);
                let response = ui.add_sized(
                    [ui.available_width(), row_height],
                    egui::Button::selectable(selected == Some(row.id), job).truncate(),
                );
                if response.clicked() {
                    clicked = Some(row.id);
                }
            }
        });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(conn: u64, dir: Dir, channel: Channel, opcode: u8, ex: Option<u16>, name: &str) -> Record {
        let mut r = Record::new(conn, dir, channel, opcode, name, None, &[opcode, 1, 2]);
        r.ex_opcode = ex;
        r.ts_ms = 1_700_000_000_123;
        r
    }

    fn game(opcode: u8, name: &str) -> Record {
        rec(2, Dir::C2s, Channel::Game, opcode, None, name)
    }

    fn matching(filter: &Filter, records: &[Record]) -> Vec<String> {
        let compiled = CompiledFilter::new(filter);
        records
            .iter()
            .filter(|r| compiled.accepts(r))
            .map(|r| r.name.clone())
            .collect()
    }

    fn sample() -> Vec<Record> {
        vec![
            rec(1, Dir::S2c, Channel::Login, 0, None, "INIT"),
            rec(1, Dir::C2s, Channel::Login, 0x07, None, "REQUEST_AUTH_LOGIN"),
            game(0x0F, "MOVE_TO_LOCATION"),
            rec(2, Dir::S2c, Channel::Game, 0x2F, None, "MOVE_TO_LOCATION"),
            game(0x59, "VALIDATE_POSITION"),
            rec(2, Dir::C2s, Channel::Game, 0xD0, Some(0x36), "REQUEST_GOTO_LOBBY"),
            rec(2, Dir::S2c, Channel::Game, 0xFE, Some(0xE8), "EX_SAY2_FAIL"),
        ]
    }

    #[test]
    fn no_filter_accepts_everything() {
        assert_eq!(matching(&Filter::default(), &sample()).len(), 7);
    }

    #[test]
    fn direction_and_channel_filters() {
        let dir = Filter {
            dir: Some(Dir::S2c),
            ..Filter::default()
        };
        assert_eq!(
            matching(&dir, &sample()),
            ["INIT", "MOVE_TO_LOCATION", "EX_SAY2_FAIL"]
        );
        let channel = Filter {
            channel: Some(Channel::Login),
            ..Filter::default()
        };
        assert_eq!(matching(&channel, &sample()), ["INIT", "REQUEST_AUTH_LOGIN"]);
        let both = Filter {
            dir: Some(Dir::C2s),
            channel: Some(Channel::Game),
            ..Filter::default()
        };
        assert_eq!(
            matching(&both, &sample()),
            ["MOVE_TO_LOCATION", "VALIDATE_POSITION", "REQUEST_GOTO_LOBBY"]
        );
    }

    #[test]
    fn text_matches_names_ignoring_case_and_underscores() {
        let text = |t: &str| Filter {
            text: t.into(),
            ..Filter::default()
        };
        assert_eq!(
            matching(&text("move_to"), &sample()),
            ["MOVE_TO_LOCATION", "MOVE_TO_LOCATION"]
        );
        assert_eq!(matching(&text("MoveToLoc"), &sample()).len(), 2);
        assert_eq!(matching(&text("validate"), &sample()), ["VALIDATE_POSITION"]);
        assert!(matching(&text("no such packet"), &sample()).is_empty());
    }

    #[test]
    fn text_matches_hex_opcodes_and_ex_opcodes() {
        let text = |t: &str| Filter {
            text: t.into(),
            ..Filter::default()
        };
        assert_eq!(matching(&text("0x59"), &sample()), ["VALIDATE_POSITION"]);
        assert_eq!(matching(&text("59"), &sample()), ["VALIDATE_POSITION"]);
        assert_eq!(matching(&text("0F"), &sample()), ["MOVE_TO_LOCATION"]);
        // Ex opcode 0x36 (four digits or with a prefix and more than two digits).
        assert_eq!(matching(&text("0036"), &sample()), ["REQUEST_GOTO_LOBBY"]);
        assert_eq!(matching(&text("0x00e8"), &sample()), ["EX_SAY2_FAIL"]);
        // All ex packets share the opcode byte.
        assert_eq!(
            matching(&text("d0"), &sample()),
            ["REQUEST_GOTO_LOBBY"],
            "0xd0 is the client ex opcode"
        );
    }

    #[test]
    fn hide_noisy_hides_whole_names_and_opcodes_in_both_directions() {
        let filter = |noisy: &str, hide: bool| Filter {
            hide_noisy: hide,
            noisy: noisy.into(),
            ..Filter::default()
        };
        // The name hides the packet in both directions.
        let shown = matching(&filter("MOVE_TO_LOCATION, VALIDATE_POSITION", true), &sample());
        assert_eq!(
            shown,
            ["INIT", "REQUEST_AUTH_LOGIN", "REQUEST_GOTO_LOBBY", "EX_SAY2_FAIL"]
        );
        // Off: the list is ignored.
        assert_eq!(matching(&filter("MOVE_TO_LOCATION", false), &sample()).len(), 7);
        // CamelCase and other separators.
        let shown = matching(&filter("MoveToLocation;validate_position", true), &sample());
        assert_eq!(shown.len(), 4);
        // A name must match completely, unlike the text filter.
        assert_eq!(matching(&filter("MOVE", true), &sample()).len(), 7);
        // Opcodes need the 0x prefix.
        let shown = matching(&filter("0x59 0x00e8", true), &sample());
        assert_eq!(shown.len(), 5);
        assert!(!shown.contains(&"VALIDATE_POSITION".to_string()));
        assert!(!shown.contains(&"EX_SAY2_FAIL".to_string()));
        // An empty list hides nothing.
        assert_eq!(matching(&filter("", true), &sample()).len(), 7);
    }

    #[test]
    fn noisy_list_parsing() {
        assert_eq!(
            parse_noisy(" MOVE_TO_LOCATION,0x59 ,, 0x00e8\nNetPing"),
            vec![
                Noisy::Name("movetolocation".into()),
                Noisy::Code(Code::Opcode(0x59)),
                Noisy::Code(Code::Ex(0xe8)),
                Noisy::Name("netping".into()),
            ]
        );
        assert!(parse_noisy("  ,  ").is_empty());
    }

    #[test]
    fn text_and_noisy_and_direction_combine() {
        let filter = Filter {
            dir: Some(Dir::C2s),
            text: "location".into(),
            hide_noisy: true,
            noisy: "VALIDATE_POSITION".into(),
            ..Filter::default()
        };
        assert_eq!(matching(&filter, &sample()), ["MOVE_TO_LOCATION"]);
    }

    fn visible_names(store: &Store) -> Vec<String> {
        (0..store.visible_len())
            .map(|i| store.visible_row(i).unwrap().record.name.clone())
            .collect()
    }

    #[test]
    fn store_filters_new_records_incrementally_and_refilters_on_change() {
        let mut store = Store::new(100);
        let only_moves = Filter {
            text: "move".into(),
            ..Filter::default()
        };
        let compiled = CompiledFilter::new(&only_moves);
        store.push_all(sample());
        store.refresh(&compiled, 1);
        assert_eq!(visible_names(&store), ["MOVE_TO_LOCATION", "MOVE_TO_LOCATION"]);

        // New records are tested without redoing the old ones.
        store.push_all(vec![game(0x01, "MOVE_BACKWARD_TO_LOCATION"), game(0x02, "OTHER")]);
        store.refresh(&compiled, 1);
        assert_eq!(store.visible_len(), 3);
        // Refreshing again adds nothing twice.
        store.refresh(&compiled, 1);
        assert_eq!(store.visible_len(), 3);

        // A new generation re-filters everything.
        store.refresh(&CompiledFilter::new(&Filter::default()), 2);
        assert_eq!(store.visible_len(), 9);
        assert_eq!(store.len(), 9);
        assert_eq!(store.total(), 9);
        assert_eq!(store.connections(), 2);
    }

    #[test]
    fn store_drops_the_oldest_records_beyond_the_cap_and_keeps_ids_valid() {
        let mut store = Store::new(100);
        let all = CompiledFilter::new(&Filter::default());
        for n in 0..=200u16 {
            store.push_all(vec![game(0x01, &format!("P{n}"))]);
            store.refresh(&all, 1);
        }
        assert!(store.len() <= 110, "trimmed in steps of a tenth: {}", store.len());
        assert!(store.len() >= 100);
        assert_eq!(store.total(), 201);
        assert_eq!(store.visible_len(), store.len(), "visible ids were trimmed along");
        // The newest record is still reachable by its id, the oldest are gone.
        let last = store.visible_row(store.visible_len() - 1).unwrap();
        assert_eq!(last.record.name, "P200");
        assert_eq!(store.get(last.id).unwrap().record.name, "P200");
        assert!(store.get(0).is_none());
        let first = store.visible_row(0).unwrap();
        assert_eq!(first.record.name, format!("P{}", 201 - store.len()));
    }

    #[test]
    fn file_stores_keep_everything() {
        let records: Vec<Record> = (0..1000).map(|n| game(0x01, &format!("P{n}"))).collect();
        let mut store = Store::from_records(records);
        store.refresh(&CompiledFilter::new(&Filter::default()), 1);
        assert_eq!(store.len(), 1000);
        assert_eq!(store.visible_len(), 1000);
    }

    #[test]
    fn clear_forgets_the_records_but_new_ones_still_show() {
        let mut store = Store::new(100);
        let all = CompiledFilter::new(&Filter::default());
        store.push_all(sample());
        store.refresh(&all, 1);
        store.clear();
        store.refresh(&all, 1);
        assert_eq!((store.len(), store.visible_len(), store.total()), (0, 0, 0));
        store.push_all(vec![game(0x01, "AFTER")]);
        store.refresh(&all, 1);
        assert_eq!(visible_names(&store), ["AFTER"]);
        assert_eq!(store.connections(), 1);
    }

    #[test]
    fn a_hundred_thousand_records_filter_quickly_enough_to_do_it_every_change() {
        let records: Vec<Record> = (0..100_000u32)
            .map(|n| {
                game(
                    (n % 200) as u8,
                    if n % 2 == 0 {
                        "MOVE_TO_LOCATION"
                    } else {
                        "USE_ITEM"
                    },
                )
            })
            .collect();
        let mut store = Store::from_records(records);
        let filter = Filter {
            text: "use".into(),
            ..Filter::default()
        };
        store.refresh(&CompiledFilter::new(&filter), 1);
        assert_eq!(store.visible_len(), 50_000);
    }

    #[test]
    fn formats_time_opcode_and_size() {
        // 1_700_000_000 s = 2023-11-14 22:13:20 UTC.
        assert_eq!(format_time(1_700_000_000_123), "22:13:20.123");
        assert_eq!(format_time(0), "00:00:00.000");
        let plain = game(0x2b, "AUTH_LOGIN");
        assert_eq!(format_opcode(&plain), "0x2b");
        assert_eq!(payload_len(&plain), 3);
        let ex = rec(2, Dir::C2s, Channel::Game, 0xD0, Some(0x36), "X");
        assert_eq!(format_opcode(&ex), "0xd0:0x0036");
    }

    #[test]
    fn header_and_rows_line_up() {
        let record = rec(7, Dir::C2s, Channel::Game, 0xD0, Some(0x36), "REQUEST_GOTO_LOBBY");
        let job = row_job(&record, true, Color32::WHITE);
        // The row text starts each column where the header does (counted in characters: the
        // arrow is more than one byte).
        let header = header_text();
        let col = |text: &str, pat: &str| text[..text.find(pat).unwrap()].chars().count();
        assert_eq!(col(&job.text, "REQUEST_GOTO_LOBBY"), col(&header, "name"));
        assert_eq!(col(&job.text, "0xd0:0x0036"), col(&header, "opcode"));
        assert_eq!(col(&job.text, "game"), col(&header, "chan"));
        assert!(job.text.ends_with(" 3  "), "no state: nothing after the size");
        let mut with_state = record.clone();
        with_state.state = Some("IN_GAME".into());
        let text = row_job(&with_state, true, Color32::WHITE).text;
        assert!(text.ends_with("IN_GAME"));
        assert_eq!(col(&text, "IN_GAME"), col(&header, "state"));
    }
}
