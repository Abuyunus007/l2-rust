//! The by-topic layout of the config editor: topics on the left, the settings of the chosen topic
//! (from several files) on the right (card G-07).
//!
//! indexes the loaded files by it ([`TopicIndex`], no egui, tested) and draws the result.

use l2_config::topics::{Topic, topic_of};

use super::controls::draw_entry;
use super::{ConfigEditorView, FileState};
use crate::views::xml_config::GROUP_TITLE;

/// From this width of the settings area on, the settings flow in two columns.
const TWO_COLUMNS_FROM: f32 = 980.0;

/// An entry of the left list: a topic, or the files no rule knows (added by hand).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bucket {
    Topic(Topic),
    Other,
    /// The "XML configs" group: the form of the XML file chosen in [`XmlConfigView`]. It is not in
    /// the [`TopicIndex`] (it holds no `.ini` setting).
    Xml,
}

impl Bucket {
    pub fn label(&self) -> String {
        match self {
            Bucket::Topic(t) => t.label(),
            Bucket::Other => "Other".to_owned(),
            Bucket::Xml => GROUP_TITLE.to_owned(),
        }
    }

    fn is_custom(&self) -> bool {
        matches!(self, Bucket::Topic(t) if t.is_custom())
    }
}

/// A setting: the index of its file in the editor and of the entry in that file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub file: usize,
    pub entry: usize,
}

/// Every loaded setting under its topic. Built again after each (re)load.
#[derive(Default)]
pub struct TopicIndex {
    /// Sorted by [`Bucket`]; items in file order, then key order.
    buckets: Vec<(Bucket, Vec<Item>)>,
}

impl TopicIndex {
    pub fn build(files: &[FileState]) -> Self {
        let mut buckets: Vec<(Bucket, Vec<Item>)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            for (entry_index, entry) in file.entries.iter().enumerate() {
                let bucket = match topic_of(file.side, &file.rel, &entry.group, &entry.key) {
                    Some(t) => Bucket::Topic(t),
                    None => Bucket::Other,
                };
                let item = Item {
                    file: file_index,
                    entry: entry_index,
                };
                match buckets.iter_mut().find(|(b, _)| *b == bucket) {
                    Some((_, items)) => items.push(item),
                    None => buckets.push((bucket, vec![item])),
                }
            }
        }
        buckets.sort_by(|a, b| a.0.cmp(&b.0));
        Self { buckets }
    }

    pub fn buckets(&self) -> &[(Bucket, Vec<Item>)] {
        &self.buckets
    }

    pub fn has(&self, bucket: &Bucket) -> bool {
        self.buckets.iter().any(|(b, _)| b == bucket)
    }

    pub fn items(&self, bucket: &Bucket) -> &[Item] {
        self.buckets
            .iter()
            .find(|(b, _)| b == bucket)
            .map_or(&[], |(_, items)| items.as_slice())
    }

    /// The items of `bucket` the search (`query`, lower case) finds: by key, comment, group or
    /// file name, or all of them when the topic's own name matches.
    pub fn hits(&self, files: &[FileState], bucket: &Bucket, query: &str) -> Vec<Item> {
        let items = self.items(bucket);
        if query.is_empty() || bucket.label().to_lowercase().contains(query) {
            return items.to_vec();
        }
        items
            .iter()
            .copied()
            .filter(|item| {
                let Some(file) = files.get(item.file) else {
                    return false;
                };
                let Some(entry) = file.entries.get(item.entry) else {
                    return false;
                };
                file.rel.to_lowercase().contains(query) || file.matches(entry, query)
            })
            .collect()
    }
}

/// One row of the topic list.
struct Row {
    bucket: Bucket,
    total: usize,
    hits: usize,
    changed: usize,
}

impl ConfigEditorView {
    fn topic_rows(&self, query: &str) -> Vec<Row> {
        self.index
            .buckets()
            .iter()
            .map(|(bucket, items)| Row {
                bucket: bucket.clone(),
                total: items.len(),
                hits: self.index.hits(&self.files, bucket, query).len(),
                changed: items
                    .iter()
                    .filter(|i| {
                        self.files
                            .get(i.file)
                            .and_then(|f| f.entries.get(i.entry).map(|e| f.is_dirty(e)))
                            .unwrap_or(false)
                    })
                    .count(),
            })
            .collect()
    }

    /// Is there anything to show for `bucket`? (The XML group exists when an XML file was read.)
    pub(super) fn has_topic(&self, bucket: &Bucket) -> bool {
        match bucket {
            Bucket::Xml => self.xml.has_files(),
            _ => self.index.has(bucket),
        }
    }

    /// The topic whose settings are shown: the chosen one, else the first.
    pub(super) fn current_topic(&self) -> Option<Bucket> {
        self.topic
            .clone()
            .filter(|t| self.has_topic(t))
            .or_else(|| self.index.buckets().first().map(|(b, _)| b.clone()))
            .or_else(|| self.xml.has_files().then_some(Bucket::Xml))
    }

    pub(super) fn topics_body(&mut self, ui: &mut egui::Ui, query: &str) {
        let rows = self.topic_rows(query);
        let current = self.current_topic();
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(240.0, height), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("cfg_topics")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.topic_list(ui, &rows, query));
            });
            ui.separator();
            // The XML forms scroll inside (their lists are long), so they get the area itself.
            // (A search that finds nothing among the XML configs shows the settings it finds.)
            if current == Some(Bucket::Xml) && self.xml.hits(query) > 0 {
                ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                    self.xml.form_ui(ui)
                });
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("cfg_settings")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if query.is_empty() {
                        if let Some(bucket) = &current {
                            let items = self.index.items(bucket).to_vec();
                            draw_items(ui, &mut self.files, &items);
                        }
                        return;
                    }
                    // A search looks through all topics, like the by-file layout looks through
                    // all files.
                    let mut any = false;
                    for row in rows.iter().filter(|r| r.hits > 0) {
                        any = true;
                        ui.heading(row.bucket.label());
                        let items = self.index.hits(&self.files, &row.bucket, query);
                        draw_items(ui, &mut self.files, &items);
                    }
                    if !any {
                        if self.xml.hits(query) > 0 {
                            ui.label("No setting matches, but an XML config does: pick it on the left.");
                        } else {
                            ui.label("Nothing matches.");
                        }
                    }
                });
        });
    }

    fn topic_list(&mut self, ui: &mut egui::Ui, rows: &[Row], query: &str) {
        let current = self.current_topic();
        let mut customs_heading = false;
        for row in rows {
            if !query.is_empty() && row.hits == 0 {
                continue;
            }
            if row.bucket.is_custom() && !customs_heading {
                customs_heading = true;
                ui.add_space(8.0);
                ui.strong("Customs");
            }
            if row.bucket == Bucket::Other {
                ui.add_space(8.0);
                ui.strong("Files without a topic");
            }
            let mut label = row.bucket.label();
            if row.changed > 0 {
                label.push_str(" *");
            }
            if query.is_empty() {
                label.push_str(&format!("  ({})", row.total));
            } else {
                label.push_str(&format!("  [{}]", row.hits));
            }
            let selected = current.as_ref() == Some(&row.bucket);
            if ui.selectable_label(selected, label).clicked() {
                self.topic = Some(row.bucket.clone());
            }
        }
        // The XML forms: the last group, found by search through its title and file name too.
        ui.add_space(8.0);
        if self.xml.topic_list(ui, current == Some(Bucket::Xml), query) {
            self.topic = Some(Bucket::Xml);
        }
    }
}

/// Draws `items`; on a wide window in two columns (the first half left, so reading order stays).
fn draw_items(ui: &mut egui::Ui, files: &mut [FileState], items: &[Item]) {
    if ui.available_width() >= TWO_COLUMNS_FROM && items.len() > 1 {
        let (left, right) = items.split_at(items.len().div_ceil(2));
        ui.columns(2, |columns| {
            if let [a, b] = columns {
                draw_column(a, files, left);
                draw_column(b, files, right);
            }
        });
    } else {
        draw_column(ui, files, items);
    }
}

fn draw_column(ui: &mut egui::Ui, files: &mut [FileState], items: &[Item]) {
    let mut last: Option<(usize, String)> = None;
    for item in items {
        let Some(file) = files.get_mut(item.file) else {
            continue;
        };
        let Some(entry) = file.entries.get(item.entry).cloned() else {
            continue;
        };
        let same_group = last
            .as_ref()
            .is_some_and(|(f, g)| *f == item.file && *g == entry.group);
        if !same_group {
            if !entry.group.is_empty() {
                ui.add_space(6.0);
                ui.strong(egui::RichText::new(&entry.group).size(15.0));
            }
            last = Some((item.file, entry.group.clone()));
        }
        draw_entry(ui, file, &entry, true);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use l2_config::defaults::Side;
    use l2_config::topics::StdTopic;

    use super::*;

    fn file(side: Side, rel: &str, text: &str) -> FileState {
        FileState::from_text(side, rel, Path::new("x.ini"), text.to_owned(), false)
    }

    fn files() -> Vec<FileState> {
        vec![
            file(Side::Login, "Server.ini", "# Port.\nLoginserverPort = 2106\n"),
            file(
                Side::Game,
                "Custom/Banking.ini",
                "BankingEnabled = False\nBankingGoldbarCount = 1\n",
            ),
            file(Side::Game, "Rates.ini", "# XP rate\nRateXp = 1\nRateSp = 1\n"),
            file(Side::Game, "Mine.ini", "Whatever = 1\n"),
        ]
    }

    #[test]
    fn settings_are_indexed_by_topic_with_customs_after_the_fixed_ones() {
        let files = files();
        let index = TopicIndex::build(&files);
        let order: Vec<Bucket> = index.buckets().iter().map(|(b, _)| b.clone()).collect();
        assert_eq!(
            order,
            [
                Bucket::Topic(Topic::Std(StdTopic::ServerNetwork)),
                Bucket::Topic(Topic::Std(StdTopic::Rates)),
                Bucket::Topic(Topic::Custom("Banking".into())),
                Bucket::Other,
            ]
        );
        let rates = Bucket::Topic(Topic::Std(StdTopic::Rates));
        assert_eq!(
            index.items(&rates),
            [Item { file: 2, entry: 0 }, Item { file: 2, entry: 1 }]
        );
        assert_eq!(index.items(&Bucket::Other).len(), 1);
        assert!(index.has(&rates) && !index.has(&Bucket::Topic(Topic::Std(StdTopic::Admin))));
        assert!(
            index
                .items(&Bucket::Topic(Topic::Std(StdTopic::Admin)))
                .is_empty()
        );
    }

    #[test]
    fn every_setting_is_in_exactly_one_bucket() {
        let files = files();
        let index = TopicIndex::build(&files);
        let total: usize = index.buckets().iter().map(|(_, items)| items.len()).sum();
        assert_eq!(total, files.iter().map(|f| f.entries.len()).sum::<usize>());
    }

    #[test]
    fn search_finds_keys_comments_files_and_topic_names() {
        let files = files();
        let index = TopicIndex::build(&files);
        let rates = Bucket::Topic(Topic::Std(StdTopic::Rates));
        let banking = Bucket::Topic(Topic::Custom("Banking".into()));
        assert_eq!(index.hits(&files, &rates, "").len(), 2);
        // A key.
        assert_eq!(index.hits(&files, &rates, "ratesp").len(), 1);
        // A comment line.
        assert_eq!(index.hits(&files, &rates, "xp rate").len(), 1);
        // The file name.
        assert_eq!(index.hits(&files, &rates, "rates.ini").len(), 2);
        // The topic name finds everything in it.
        assert_eq!(index.hits(&files, &banking, "bank").len(), 2);
        assert_eq!(index.hits(&files, &rates, "rates").len(), 2);
        assert!(index.hits(&files, &rates, "banking").is_empty());
    }
}
