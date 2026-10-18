//! Data browser: the NPC templates and item templates of a datapack (the folder that contains
//! `data/`), loaded with `l2-data` on a background thread, searched and sorted here.
//!
//! Read-only (card M3-GUI-02: editing is out of scope). Java: none (tooling); the tables are the
//! `l2-data` ports of Java's `NpcData` and `ItemData`.
//!
//! [`DataTable`] holds the rules (search, sort, selection) and has no egui in it, so it is tested
//! directly. The view only draws it and starts the loader.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use l2_data::{ItemKind, ItemTemplate, ItemTemplates, NpcLoadOptions, NpcTemplate, NpcTemplates};

use crate::app::AppCtx;
use crate::views::View;

/// Most rows drawn at once. A search narrows the list; the count line says how many match.
pub const SHOW_LIMIT: usize = 500;

/// Both tables as the loaders return them. Immutable once loaded.
#[derive(Debug, Default)]
pub struct Catalog {
    pub npcs: NpcTemplates,
    pub items: ItemTemplates,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Npcs,
    Items,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Npcs, Tab::Items];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Npcs => "NPCs",
            Tab::Items => "Items",
        }
    }

    /// The columns this tab shows, in order. Only these can be sorted by.
    pub fn columns(self) -> &'static [Column] {
        match self {
            Tab::Npcs => &[
                Column::Id,
                Column::Name,
                Column::Title,
                Column::Type,
                Column::Level,
            ],
            Tab::Items => &[
                Column::Id,
                Column::Name,
                Column::Type,
                Column::Grade,
                Column::Weight,
                Column::Price,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Id,
    Name,
    Title,
    /// NPC type (`Monster`, `Folk`, ...) or item kind (`Weapon: SWORD`, ...).
    Type,
    Level,
    /// Crystal grade of an item (`D`, `S80`, `NONE`, ...).
    Grade,
    Weight,
    Price,
}

impl Column {
    pub fn label(self) -> &'static str {
        match self {
            Column::Id => "Id",
            Column::Name => "Name",
            Column::Title => "Title",
            Column::Type => "Type",
            Column::Level => "Level",
            Column::Grade => "Grade",
            Column::Weight => "Weight",
            Column::Price => "Price",
        }
    }

    /// The text of this column in `row`. A value the row does not have shows as `-`.
    fn cell(self, row: &Row) -> String {
        match self {
            Column::Id => row.id.to_string(),
            Column::Name => row.name.clone(),
            Column::Title => row.title.clone(),
            Column::Type => row.kind.clone(),
            Column::Level => opt_text(row.level),
            Column::Grade => row.grade.clone(),
            Column::Weight => opt_text(row.weight),
            Column::Price => opt_text(row.price),
        }
    }

    /// Ascending order of this column. Ties are broken by id by the caller.
    fn compare(self, a: &Row, b: &Row) -> Ordering {
        match self {
            Column::Id => a.id.cmp(&b.id),
            Column::Name => a.name_key.cmp(&b.name_key).then_with(|| a.name.cmp(&b.name)),
            Column::Title => ci_cmp(&a.title, &b.title),
            Column::Type => ci_cmp(&a.kind, &b.kind),
            // `None` (no such value) sorts first.
            Column::Level => a.level.cmp(&b.level),
            Column::Grade => ci_cmp(&a.grade, &b.grade),
            Column::Weight => a.weight.cmp(&b.weight),
            Column::Price => a.price.cmp(&b.price),
        }
    }
}

fn opt_text<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

fn ci_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// One line of the table: the columns of both tabs, with the values a tab does not use left
/// empty or `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: i32,
    pub name: String,
    /// Lower-case name, computed once for the search and the sort.
    name_key: String,
    /// NPC title; empty for items.
    pub title: String,
    /// NPC type, or the item kind.
    pub kind: String,
    /// NPC level.
    pub level: Option<i32>,
    /// Item crystal grade; empty for NPCs.
    pub grade: String,
    pub weight: Option<i32>,
    pub price: Option<i32>,
}

impl Row {
    fn npc(t: &NpcTemplate) -> Self {
        Self {
            id: t.id,
            name: t.name.clone(),
            name_key: t.name.to_lowercase(),
            title: t.title.clone(),
            kind: t.npc_type.clone(),
            level: Some(i32::from(t.level)),
            grade: String::new(),
            weight: None,
            price: None,
        }
    }

    fn item(t: &ItemTemplate) -> Self {
        Self {
            id: t.id,
            name: t.name.clone(),
            name_key: t.name.to_lowercase(),
            title: String::new(),
            kind: item_kind_text(&t.kind),
            level: None,
            grade: t.crystal_type.name().to_string(),
            weight: Some(t.weight),
            price: Some(t.price),
        }
    }
}

fn item_kind_text(kind: &ItemKind) -> String {
    match kind {
        ItemKind::Weapon(w) => format!("Weapon: {}", w.weapon_type.name()),
        ItemKind::Armor(a) => format!("Armor: {}", a.armor_type.name()),
        ItemKind::Etc(e) => format!("Etc: {}", e.etc_type.name()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}

impl Default for Sort {
    /// Id ascending, the order the datapack files are read in.
    fn default() -> Self {
        Sort {
            column: Column::Id,
            descending: false,
        }
    }
}

/// `needle` is trimmed and lower case. A number matches the id exactly; any text matches a name
/// that contains it.
fn row_matches(row: &Row, needle: &str, id: Option<i32>) -> bool {
    id == Some(row.id) || row.name_key.contains(needle)
}

/// The rules of the table: the rows of both tabs, the current tab's search and sort, and the
/// selected row. [`DataTable::rows`] is kept up to date by every change, so drawing is cheap.
#[derive(Debug, Default)]
pub struct DataTable {
    npc_rows: Vec<Row>,
    item_rows: Vec<Row>,
    tab: Tab,
    sort: Sort,
    query: String,
    /// Indexes into the current tab's rows, after the search and the sort.
    visible: Vec<usize>,
    selected: Option<i32>,
}

impl DataTable {
    pub fn set_catalog(&mut self, catalog: &Catalog) {
        self.npc_rows = catalog.npcs.iter().map(Row::npc).collect();
        self.item_rows = catalog.items.iter().map(Row::item).collect();
        // The loaders already return id order; sorting again keeps the default order explicit.
        self.npc_rows.sort_by_key(|r| r.id);
        self.item_rows.sort_by_key(|r| r.id);
        self.selected = None;
        self.refresh();
    }

    pub fn clear(&mut self) {
        self.npc_rows.clear();
        self.item_rows.clear();
        self.selected = None;
        self.refresh();
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Switching tabs drops the selection. A sort by a column the new tab does not have goes back
    /// to the default sort.
    pub fn set_tab(&mut self, tab: Tab) {
        if tab == self.tab {
            return;
        }
        self.tab = tab;
        self.selected = None;
        if !tab.columns().contains(&self.sort.column) {
            self.sort = Sort::default();
        }
        self.refresh();
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: &str) {
        if query == self.query {
            return;
        }
        self.query = query.to_string();
        self.refresh();
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// A click on a column header: the same column flips the direction, another column sorts
    /// ascending. Columns the current tab does not show are ignored.
    pub fn click(&mut self, column: Column) {
        if !self.tab.columns().contains(&column) {
            return;
        }
        if self.sort.column == column {
            self.sort.descending = !self.sort.descending;
        } else {
            self.sort = Sort {
                column,
                descending: false,
            };
        }
        self.refresh();
    }

    pub fn selected(&self) -> Option<i32> {
        self.selected
    }

    pub fn select(&mut self, id: i32) {
        self.selected = Some(id);
    }

    /// Rows in the current tab, before the search.
    pub fn total(&self) -> usize {
        self.current().len()
    }

    /// The rows the search keeps, in sort order (all of them; the view draws [`SHOW_LIMIT`]).
    pub fn rows(&self) -> Vec<&Row> {
        let current = self.current();
        self.visible.iter().map(|&i| &current[i]).collect()
    }

    fn current(&self) -> &[Row] {
        match self.tab {
            Tab::Npcs => &self.npc_rows,
            Tab::Items => &self.item_rows,
        }
    }

    fn refresh(&mut self) {
        let needle = self.query.trim().to_lowercase();
        let id_needle: Option<i32> = needle.parse().ok();
        let sort = self.sort;
        let rows = self.current();
        let mut visible: Vec<usize> = (0..rows.len())
            .filter(|&i| needle.is_empty() || row_matches(&rows[i], &needle, id_needle))
            .collect();
        visible.sort_by(|&a, &b| {
            let (ra, rb) = (&rows[a], &rows[b]);
            let order = sort.column.compare(ra, rb);
            let order = if sort.descending { order.reverse() } else { order };
            order.then_with(|| ra.id.cmp(&rb.id))
        });
        self.visible = visible;
    }
}

/// Every field of the template with this id, as Rust's pretty `Debug` text. `None` when the
/// tab's table has no such id.
pub fn detail_text(catalog: &Catalog, tab: Tab, id: i32) -> Option<String> {
    match tab {
        Tab::Npcs => catalog.npcs.get(id).map(|t| format!("{t:#?}")),
        Tab::Items => catalog.items.get(id).map(|t| format!("{t:#?}")),
    }
}

/// The datapack folder typed in the settings: surrounding spaces and quotes (Explorer's "Copy as
/// path" adds them) are removed. `None` when nothing is set.
pub fn datapack_root(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"').trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Loads the NPC and item tables below `root` (the folder that contains `data/`). Runs on a
/// background thread: it reads many files. Without a `data` folder it is an error, so a wrong
/// path is reported instead of showing empty tables.
pub fn load_catalog(root: &Path) -> Result<Catalog, String> {
    if !root.join("data").is_dir() {
        return Err(format!(
            "{} has no data folder; set the folder that contains data/",
            root.display()
        ));
    }
    Ok(Catalog {
        npcs: NpcTemplates::load(root, &NpcLoadOptions::default()),
        // The Java defaults: custom item files are off unless the server config turns them on.
        items: ItemTemplates::load(root, false),
    })
}

/// The clicks of one frame, applied after the drawing (the table is borrowed while drawing).
#[derive(Default)]
struct Clicks {
    column: Option<Column>,
    row: Option<i32>,
}

#[derive(Default)]
pub struct DataBrowserView {
    table: DataTable,
    catalog: Option<Catalog>,
    /// The loader thread's answer, while a load is running.
    loading: Option<Receiver<Result<Catalog, String>>>,
    /// Why the last load failed (no folder, thread could not start, ...).
    error: Option<String>,
}

impl DataBrowserView {
    /// Starts loading `root` on a background thread. The thread wakes the UI when it is done.
    fn start_load(&mut self, root: PathBuf, ctx: egui::Context) {
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("datapack-load".into())
            .spawn(move || {
                let result = load_catalog(&root);
                // The view may be gone when the app closes; then nobody needs the answer.
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        match spawned {
            // The handle is dropped: the thread is detached and ends after one load.
            Ok(_handle) => {
                self.loading = Some(rx);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("cannot start the loader thread: {e}")),
        }
    }

    /// Takes the loader's answer if it has arrived.
    fn poll_loader(&mut self) {
        let Some(rx) = self.loading.as_ref() else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the loader thread stopped without an answer".to_string()),
        };
        self.loading = None;
        self.finish_load(result);
    }

    /// Shows a finished load. A failed load drops the previous tables so the view never shows
    /// data that the error message contradicts.
    fn finish_load(&mut self, result: Result<Catalog, String>) {
        match result {
            Ok(catalog) => {
                self.table.set_catalog(&catalog);
                self.catalog = Some(catalog);
                self.error = None;
            }
            Err(message) => {
                self.table.clear();
                self.catalog = None;
                self.error = Some(message);
            }
        }
    }
}

/// The header and the rows of the table. Returns the clicks; the caller applies them.
fn table_grid(ui: &mut egui::Ui, rows: &[&Row], tab: Tab, sort: Sort, selected: Option<i32>) -> Clicks {
    let mut clicks = Clicks::default();
    egui::Grid::new("data_rows")
        .striped(true)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for &column in tab.columns() {
                let active = sort.column == column;
                let label = match (active, sort.descending) {
                    (true, false) => format!("{} (asc)", column.label()),
                    (true, true) => format!("{} (desc)", column.label()),
                    (false, _) => column.label().to_string(),
                };
                if ui.selectable_label(active, label).clicked() {
                    clicks.column = Some(column);
                }
            }
            ui.end_row();
            for &row in rows.iter().take(SHOW_LIMIT) {
                for &column in tab.columns() {
                    let is_selected = selected == Some(row.id);
                    if ui.selectable_label(is_selected, column.cell(row)).clicked() {
                        clicks.row = Some(row.id);
                    }
                }
                ui.end_row();
            }
        });
    clicks
}

impl View for DataBrowserView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>) {
        self.poll_loader();

        ui.heading("Data browser");
        ui.add_space(8.0);
        let mut load_clicked = false;
        ui.horizontal(|ui| {
            ui.label("Datapack folder:");
            ui.add(
                egui::TextEdit::singleline(&mut cx.settings.datapack_path)
                    .hint_text("the folder that contains data/, e.g. the Java server's game folder")
                    .desired_width(420.0),
            );
            let can_load = datapack_root(&cx.settings.datapack_path).is_some() && self.loading.is_none();
            load_clicked = ui.add_enabled(can_load, egui::Button::new("Load")).clicked();
        });
        ui.add_space(4.0);

        let root = datapack_root(&cx.settings.datapack_path);
        // Load once when the view first opens with a folder set; after a failure the user retries.
        let first_open = self.catalog.is_none() && self.loading.is_none() && self.error.is_none();
        if let Some(root) = root.as_ref().filter(|_| load_clicked || first_open) {
            self.start_load(root.clone(), ui.ctx().clone());
        }
        if root.is_none() {
            ui.label(
                "Set the datapack folder to browse NPC and item templates. It is the folder \
                 that contains data/ (for the Java server, its game folder).",
            );
            return;
        }
        if self.loading.is_some() {
            ui.label("Loading NPC and item templates...");
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
        let Some(catalog) = &self.catalog else {
            return;
        };

        ui.label(format!(
            "{} NPCs and {} items loaded.",
            catalog.npcs.len(),
            catalog.items.len()
        ));
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui
                    .selectable_label(self.table.tab() == tab, tab.label())
                    .clicked()
                {
                    self.table.set_tab(tab);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Search:");
            let mut query = self.table.query().to_string();
            let edit = egui::TextEdit::singleline(&mut query)
                .hint_text("id or name")
                .desired_width(260.0);
            if ui.add(edit).changed() {
                self.table.set_query(&query);
            }
        });

        let tab = self.table.tab();
        let sort = self.table.sort();
        let selected = self.table.selected();
        let total = self.table.total();
        let rows = self.table.rows();
        let limited = if rows.len() > SHOW_LIMIT {
            format!("; showing the first {SHOW_LIMIT}, refine the search for more")
        } else {
            String::new()
        };
        ui.label(format!(
            "{} of {total} {} match{limited}",
            rows.len(),
            tab.label()
        ));
        ui.add_space(4.0);

        let detail = selected.and_then(|id| detail_text(catalog, tab, id));
        let mut clicks = Clicks::default();
        ui.columns(2, |cols| {
            egui::ScrollArea::both()
                .id_salt("data_rows_scroll")
                .auto_shrink([false, false])
                .show(&mut cols[0], |ui| {
                    clicks = table_grid(ui, &rows, tab, sort, selected);
                });
            if let Some(text) = &detail {
                egui::ScrollArea::vertical()
                    .id_salt("data_detail_scroll")
                    .auto_shrink([false, false])
                    .show(&mut cols[1], |ui| {
                        ui.label(egui::RichText::new(text).monospace());
                    });
            } else {
                cols[1].label("Click a row to see all of its fields.");
            }
        });

        if let Some(column) = clicks.column {
            self.table.click(column);
        }
        if let Some(id) = clicks.row {
            self.table.select(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Catalog {
        let npcs = match NpcTemplates::from_xml(
            r#"<?xml version="1.0"?><list>
                <npc id="1" level="10" type="Monster" name="Wolf" title="Hungry"/>
                <npc id="2" level="40" type="Folk" name="alice" title="Guide"/>
                <npc id="30" level="5" type="Merchant" name="Bob"/>
            </list>"#,
            &NpcLoadOptions::default(),
        ) {
            Ok(t) => t,
            Err(e) => panic!("npc fixture: {e}"),
        };
        let items = match ItemTemplates::from_xml(
            r#"<?xml version="1.0"?><list>
                <item id="1" type="Weapon" name="Short Sword">
                    <set name="weapon_type" val="SWORD" />
                    <set name="bodypart" val="rhand" />
                    <set name="weight" val="1600" />
                    <set name="price" val="590" />
                    <set name="crystal_type" val="D" />
                </item>
                <item id="2" type="Armor" name="Plate">
                    <set name="armor_type" val="HEAVY" />
                    <set name="bodypart" val="chest" />
                    <set name="weight" val="3000" />
                    <set name="price" val="9000" />
                    <set name="crystal_type" val="S84" />
                </item>
                <item id="57" type="EtcItem" name="Potion of Healing">
                    <set name="etcitem_type" val="POTION" />
                    <set name="weight" val="20" />
                    <set name="price" val="100" />
                </item>
            </list>"#,
        ) {
            Ok(t) => t,
            Err(e) => panic!("item fixture: {e}"),
        };
        Catalog { npcs, items }
    }

    fn table() -> DataTable {
        let mut t = DataTable::default();
        t.set_catalog(&fixture());
        t
    }

    fn names(t: &DataTable) -> Vec<&str> {
        t.rows().iter().map(|r| r.name.as_str()).collect()
    }

    fn ids(t: &DataTable) -> Vec<i32> {
        t.rows().iter().map(|r| r.id).collect()
    }

    #[test]
    fn npc_rows_hold_id_name_title_type_and_level() {
        let t = table();
        assert_eq!(t.tab(), Tab::Npcs);
        assert_eq!(ids(&t), [1, 2, 30], "default sort is by id");
        let alice = t.rows()[1].clone();
        assert_eq!(alice.name, "alice");
        assert_eq!(alice.title, "Guide");
        assert_eq!(alice.kind, "Folk");
        assert_eq!(alice.level, Some(40));
        assert_eq!(alice.grade, "", "items only");
        assert_eq!(alice.weight, None);
    }

    #[test]
    fn item_rows_hold_kind_grade_weight_and_price() {
        let mut t = table();
        t.set_tab(Tab::Items);
        assert_eq!(ids(&t), [1, 2, 57]);
        let sword = t.rows()[0].clone();
        assert_eq!(sword.kind, "Weapon: SWORD");
        assert_eq!(sword.grade, "D");
        assert_eq!((sword.weight, sword.price), (Some(1600), Some(590)));
        assert_eq!(sword.level, None);
        assert_eq!(t.rows()[1].grade, "S84", "sub-grades are kept");
        assert_eq!(t.rows()[1].kind, "Armor: HEAVY");
        assert_eq!(t.rows()[2].kind, "Etc: POTION");
    }

    #[test]
    fn search_matches_names_ignoring_case_and_ids_exactly() {
        let mut t = table();
        t.set_query("ALI");
        assert_eq!(names(&t), ["alice"]);
        t.set_query("wo");
        assert_eq!(names(&t), ["Wolf"]);
        t.set_query("30");
        assert_eq!(names(&t), ["Bob"], "a number matches the id");
        t.set_query("3");
        assert!(names(&t).is_empty(), "id 3 does not exist and no name has a 3");
        t.set_query("  bob ");
        assert_eq!(names(&t), ["Bob"], "surrounding spaces are ignored");
        t.set_query("nobody");
        assert!(t.rows().is_empty());
        t.set_query("");
        assert_eq!(t.rows().len(), 3, "a blank search shows everything");
        assert_eq!(t.total(), 3, "the total does not change with the search");
    }

    #[test]
    fn search_applies_to_the_tab_it_is_typed_in() {
        let mut t = table();
        t.set_query("sword");
        assert!(t.rows().is_empty(), "no NPC is called Sword");
        t.set_tab(Tab::Items);
        assert_eq!(names(&t), ["Short Sword"], "the search stays and reapplies");
        assert_eq!(t.query(), "sword");
    }

    #[test]
    fn sort_by_name_is_case_insensitive() {
        let mut t = table();
        t.click(Column::Name);
        assert_eq!(names(&t), ["alice", "Bob", "Wolf"]);
        t.click(Column::Name);
        assert_eq!(names(&t), ["Wolf", "Bob", "alice"]);
    }

    #[test]
    fn clicking_a_column_sorts_then_flips() {
        let mut t = table();
        t.click(Column::Level);
        assert_eq!(names(&t), ["Bob", "Wolf", "alice"], "levels 5, 10, 40");
        t.click(Column::Level);
        assert_eq!(names(&t), ["alice", "Wolf", "Bob"]);
        assert_eq!(
            t.sort(),
            Sort {
                column: Column::Level,
                descending: true
            }
        );
    }

    #[test]
    fn sort_by_numbers_is_numeric_not_textual() {
        let mut t = table();
        t.set_tab(Tab::Items);
        t.click(Column::Price);
        assert_eq!(ids(&t), [57, 1, 2], "100 < 590 < 9000");
        t.click(Column::Weight);
        assert_eq!(ids(&t), [57, 1, 2], "weights 20 < 1600 < 3000, ascending again");
    }

    #[test]
    fn ties_keep_id_order_in_both_directions() {
        let npcs = match NpcTemplates::from_xml(
            r#"<?xml version="1.0"?><list>
                <npc id="9" level="10" name="Nine"/>
                <npc id="3" level="10" name="Three"/>
                <npc id="5" level="1" name="Five"/>
            </list>"#,
            &NpcLoadOptions::default(),
        ) {
            Ok(t) => t,
            Err(e) => panic!("tie fixture: {e}"),
        };
        let mut t = DataTable::default();
        t.set_catalog(&Catalog {
            npcs,
            items: ItemTemplates::default(),
        });
        t.click(Column::Level);
        assert_eq!(ids(&t), [5, 3, 9], "level 1, then the two level 10 by id");
        t.click(Column::Level);
        assert_eq!(ids(&t), [3, 9, 5], "descending still breaks ties by id ascending");
    }

    #[test]
    fn a_column_the_tab_does_not_show_cannot_be_clicked() {
        let mut t = table();
        t.click(Column::Weight);
        assert_eq!(t.sort(), Sort::default(), "NPCs have no weight");
    }

    #[test]
    fn switching_tabs_resets_a_sort_the_new_tab_lacks() {
        let mut t = table();
        t.click(Column::Level);
        t.set_tab(Tab::Items);
        assert_eq!(t.sort(), Sort::default(), "items have no level");
        t.click(Column::Price);
        t.set_tab(Tab::Npcs);
        assert_eq!(t.sort(), Sort::default(), "NPCs have no price");
        t.click(Column::Level);
        t.set_tab(Tab::Npcs);
        assert_eq!(
            t.sort(),
            Sort {
                column: Column::Level,
                descending: false
            },
            "switching to the same tab keeps the sort"
        );
    }

    #[test]
    fn selection_is_dropped_by_tab_and_catalog_changes() {
        let mut t = table();
        t.select(2);
        assert_eq!(t.selected(), Some(2));
        t.set_tab(Tab::Items);
        assert_eq!(t.selected(), None);
        t.select(2);
        t.set_catalog(&fixture());
        assert_eq!(t.selected(), None);
    }

    #[test]
    fn clear_empties_the_table() {
        let mut t = table();
        t.select(1);
        t.clear();
        assert_eq!(t.total(), 0);
        assert!(t.rows().is_empty());
        assert_eq!(t.selected(), None);
    }

    #[test]
    fn detail_shows_every_field_of_the_template() {
        let c = fixture();
        let npc = detail_text(&c, Tab::Npcs, 1).unwrap_or_default();
        assert!(npc.contains("name: \"Wolf\""), "{npc}");
        assert!(npc.contains("npc_type: \"Monster\""), "{npc}");
        assert!(npc.contains("title: \"Hungry\""), "{npc}");
        let item = detail_text(&c, Tab::Items, 1).unwrap_or_default();
        assert!(item.contains("weight: 1600"), "{item}");
        assert!(item.contains("price: 590"), "{item}");
    }

    #[test]
    fn detail_is_none_for_an_id_the_tab_does_not_have() {
        let c = fixture();
        assert!(detail_text(&c, Tab::Npcs, 57).is_none(), "57 is an item");
        assert!(detail_text(&c, Tab::Items, 30).is_none(), "30 is an NPC");
        assert!(detail_text(&c, Tab::Items, 999).is_none());
    }

    #[test]
    fn datapack_path_ignores_quotes_and_blanks() {
        assert_eq!(datapack_root("   "), None);
        assert_eq!(datapack_root(""), None);
        assert_eq!(datapack_root("\"\""), None);
        assert_eq!(
            datapack_root("  \"some/game\" "),
            Some(PathBuf::from("some/game"))
        );
    }

    #[test]
    fn a_folder_without_data_is_an_error_not_empty_tables() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_catalog(dir.path()).err().unwrap_or_default();
        assert!(err.contains("no data folder"), "{err}");
        std::fs::create_dir(dir.path().join("data")).unwrap();
        let catalog = load_catalog(dir.path()).unwrap();
        assert!(catalog.npcs.is_empty() && catalog.items.is_empty());
    }

    #[test]
    fn a_finished_load_fills_the_view_and_a_failed_one_clears_it() {
        let mut view = DataBrowserView::default();
        view.finish_load(Ok(fixture()));
        assert_eq!(view.table.total(), 3);
        assert!(view.catalog.is_some() && view.error.is_none());

        view.finish_load(Err("no data folder".into()));
        assert!(view.catalog.is_none());
        assert_eq!(view.table.total(), 0);
        assert_eq!(view.error.as_deref(), Some("no data folder"));
    }

    #[test]
    fn the_loader_answer_is_taken_once_it_arrives() {
        let mut view = DataBrowserView::default();
        let (tx, rx) = mpsc::channel();
        view.loading = Some(rx);
        view.poll_loader();
        assert!(view.loading.is_some(), "nothing arrived yet");
        assert!(tx.send(Ok(fixture())).is_ok());
        view.poll_loader();
        assert!(view.loading.is_none());
        assert_eq!(view.table.total(), 3);
    }

    #[test]
    fn a_dropped_loader_reports_an_error() {
        let mut view = DataBrowserView::default();
        let (tx, rx) = mpsc::channel::<Result<Catalog, String>>();
        view.loading = Some(rx);
        drop(tx);
        view.poll_loader();
        assert!(view.loading.is_none());
        assert!(view.error.is_some());
    }
}
