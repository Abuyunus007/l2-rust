//! `DropSearchBoard`: search an item by name, list the NPCs that drop it, trace an NPC on the
//! radar.
// Java: dist/game/data/scripts/handlers/bypass/communityboard/DropSearchBoard.java
//
// The index is Java's `DROP_INDEX_CACHE`, built once from all NPC templates (Java builds it in the
// constructor at start-up; here on the first command, the templates never change after start).
// The rates are the ones `NpcViewMod` shows (`script_handlers/npc_view.rs`); the premium system is
// off (`PremiumSystem.ini`), so the premium branches of Java are not taken.
//
// Java throws (`NumberFormatException`, `NullPointerException` on a missing page or an item nobody
// drops) out of `onCommand` in several places; the exception ends the command and nothing is sent.
// Those places `return` here.

use std::collections::BTreeMap;

use l2_data::item::ADENA_ID;
use l2_data::npc::DropType;
use l2_protocol_game::server::{RadarControl, ShowMiniMap};
use l2_util::ObjectId;

use crate::rewards::drop::{group_list, ungrouped_list};
use crate::script_handlers::npc_view::chance_format;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

const NAVIGATION_PATH: &str = "data/html/CommunityBoard/Custom/navigation.html";
const MAIN_PATH: &str = "data/html/CommunityBoard/Custom/dropsearch/main.html";

/// Java `CBDropHolder`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CbDrop {
    item_id: i32,
    npc_id: i32,
    npc_level: i8,
    min: i64,
    max: i64,
    chance: f64,
    is_spoil: bool,
    is_raid: bool,
}

/// Java `DROP_INDEX_CACHE`: item id to the drops of it, each list sorted by NPC level.
pub(crate) type DropIndex = BTreeMap<i32, Vec<CbDrop>>;

/// The tasks of the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardTimer {
    /// `_bbs_npc_trace`'s `ThreadPool.schedule(.., 500)`: the radar marker.
    RadarTrace {
        player: ObjectId,
        x: i32,
        y: i32,
        z: i32,
    },
}

/// Java `Double.toString` for the values this board prints: plain with a fraction for
/// `1e-3 <= |v| < 1e7`, else the `1.0E7` form.
pub(crate) fn java_double(value: f64) -> String {
    let abs = value.abs();
    if abs == 0.0 || (1e-3..1e7).contains(&abs) {
        return format!("{value:?}");
    }
    let text = format!("{value:e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let mantissa = if mantissa.contains('.') {
        mantissa.to_owned()
    } else {
        format!("{mantissa}.0")
    };
    format!("{mantissa}E{exponent}")
}

impl World {
    /// Java `new DropSearchBoard()`: `buildDropIndex()`.
    fn build_drop_index(&self) -> DropIndex {
        let mut index = DropIndex::new();
        let (Some(templates), Some(items)) = (self.npc_world.templates_arc(), self.item_templates()) else {
            return index;
        };
        // The raw lists of the template: no boss drops of `Rates.ini`.
        let mut settings = self.settings.drops.clone();
        settings.boss_drop_enabled = false;
        let mut add = |npc: &l2_data::NpcTemplate, drop: &l2_data::npc::DropEntry, chance: f64| {
            // `BLOCK_ID`: adena is not searchable.
            if drop.item_id == ADENA_ID {
                return;
            }
            index.entry(drop.item_id).or_default().push(CbDrop {
                item_id: drop.item_id,
                npc_id: npc.id,
                npc_level: npc.level,
                min: drop.min,
                max: drop.max,
                chance,
                is_spoil: drop.drop_type == DropType::Spoil,
                is_raid: npc.npc_type == "RaidBoss" || npc.npc_type == "GrandBoss",
            });
        };
        for npc in templates.iter() {
            for group in group_list(&items, npc) {
                let chance = group.chance / 100.0;
                for drop in &group.items {
                    add(npc, drop, drop.chance * chance);
                }
            }
        }
        for npc in templates.iter() {
            for drop in ungrouped_list(&settings, &items, npc, DropType::Drop) {
                add(npc, &drop, drop.chance);
            }
        }
        for npc in templates.iter() {
            for drop in ungrouped_list(&settings, &items, npc, DropType::Spoil) {
                add(npc, &drop, drop.chance);
            }
        }
        // `Byte.compare(npcLevel)`, a stable sort.
        for list in index.values_mut() {
            list.sort_by_key(|d| d.npc_level);
        }
        index
    }

    /// `DropSearchBoard.onCommand(command, player)`.
    pub(crate) fn drop_search_board_command(&mut self, player: ObjectId, command: &str) {
        if self.community_board.drop_index.is_none() {
            let index = self.build_drop_index();
            self.community_board.drop_index = Some(index);
        }
        let navigation = self.get_htm(player, NAVIGATION_PATH);
        let params: Vec<&str> = command.split(' ').collect();
        let mut html = self.get_htm(player, MAIN_PATH);
        match params[0] {
            "_bbs_search_item" => {
                // `html.replace` on a missing page throws.
                let Some(page) = html.take() else { return };
                let item_name = params[1..].join(" ");
                let result = self.build_item_search_result(&item_name);
                html = Some(page.replace("%searchResult%", &result));
            }
            "_bbs_search_drop" => {
                let (Some(item_id), Some(page)) = (
                    params.get(1).and_then(|p| p.parse::<i32>().ok()),
                    params.get(2).and_then(|p| p.parse::<i32>().ok()),
                ) else {
                    return;
                };
                let Some(page_html) = html.take() else { return };
                // `list` is null for an item nobody drops.
                let Some(list) = self
                    .community_board
                    .drop_index
                    .as_ref()
                    .and_then(|i| i.get(&item_id))
                    .cloned()
                else {
                    return;
                };
                let Some(built) = self.build_drop_page(player, &page_html, item_id, page, &list) else {
                    return;
                };
                html = Some(built);
            }
            "_bbs_npc_trace" => {
                let Some(npc_id) = params.get(1).and_then(|p| p.parse::<i32>().ok()) else {
                    return;
                };
                // `SpawnTable.getAnySpawn(npcId)`.
                let spawn = self
                    .npc_world
                    .spawn_data()
                    .and_then(|data| data.spawns_of(npc_id).next())
                    .map(|s| (s.location.x, s.location.y, s.location.z));
                match spawn {
                    None => self.send_message(
                        player,
                        "Cannot find any spawn. Maybe dropped by a boss or instance monster.",
                    ),
                    Some((x, y, z)) => {
                        self.send(
                            player,
                            &ShowMiniMap {
                                map_id: -1,
                                seven_signs_period: self.ss_current_period() as u8,
                            },
                        );
                        let due = self.tick + TimerWheel::ms_to_ticks(500);
                        self.timers.schedule_at(
                            due,
                            TimerKind::Board(BoardTimer::RadarTrace { player, x, y, z }),
                        );
                    }
                }
            }
            _ => {}
        }
        if let Some(html) = html {
            // `html.replace("%navigation%", null)` throws.
            let Some(navigation) = navigation else { return };
            let html = html.replace("%navigation%", &navigation);
            self.send_cb_html(player, &html);
        }
    }

    /// The task of [`BoardTimer`].
    pub(crate) fn on_board_timer(&mut self, timer: BoardTimer) {
        match timer {
            BoardTimer::RadarTrace { player, x, y, z } => {
                if !self.players.contains_key(&player) {
                    return;
                }
                // `getRadar().addMarker(x, y, z)`.
                let markers = self.scripts.radar.entry(player).or_default();
                if !markers.contains(&(x, y, z)) {
                    markers.push((x, y, z));
                }
                let radar = |show_radar, radar_type| RadarControl {
                    show_radar,
                    radar_type,
                    x,
                    y,
                    z,
                };
                self.send(player, &radar(2, 2));
                self.send(player, &radar(0, 1));
                self.send(player, &radar(0, 2));
            }
        }
    }

    /// Java `buildItemSearchResult(itemName)`.
    fn build_item_search_result(&self, item_name: &str) -> String {
        let Some(items) = self.item_templates() else {
            return "<tr><td width=100 align=CENTER>No Match</td></tr>".to_owned();
        };
        let index = self.community_board.drop_index.as_ref();
        let wanted = item_name.to_lowercase();
        let mut found: Vec<(i32, String)> = Vec::new();
        for item in items.iter() {
            if !index.is_some_and(|i| i.contains_key(&item.id)) {
                continue;
            }
            if item.name.to_lowercase().contains(&wanted) {
                found.push((item.id, item.icon.clone().unwrap_or_default()));
            }
            if found.len() == 14 {
                break;
            }
        }
        if found.is_empty() {
            return "<tr><td width=100 align=CENTER>No Match</td></tr>".to_owned();
        }
        let mut line = 0;
        let mut out = String::new();
        let mut i = 0;
        for (id, icon) in &found {
            i += 1;
            if i == 1 {
                line += 1;
                out.push_str("<tr>");
            }
            let icon = if icon.is_empty() {
                "icon.etc_question_mark_i00"
            } else {
                icon
            };
            out.push_str(&format!("<td width=30 background=\"{icon}\">"));
            out.push_str(&format!(
                "<button value=\" \" action=\"bypass _bbs_search_drop {id} 1 $order $level\" width=30 height=30 back=\"\" fore=\"\">"
            ));
            out.push_str("</td><td width=200>");
            out.push_str(&format!("&#{id};"));
            out.push_str("</td>");
            if i == 2 {
                out.push_str("</tr>");
                i = 0;
            }
        }
        if i % 2 == 1 {
            out.push_str("</tr>");
        }
        if line < 7 {
            for _ in 0..(7 - line) {
                out.push_str("<tr><td height=36></td></tr>");
            }
        }
        out
    }

    /// The `_bbs_search_drop` page.
    fn build_drop_page(&self, player: ObjectId, html: &str, item_id: i32, page: i32, list: &[CbDrop]) -> Option<String> {
        let settings = &self.settings.drops;
        let killer = self.npc_view_killer(player);
        let items = self.item_templates();
        let size = list.len() as i64;
        let mut pages = size / 14;
        if pages == 0 {
            pages += 1;
        }
        let start = (i64::from(page) - 1) * 14;
        // NOTE(java-quirk): `end` is inclusive, so a page shows up to 15 rows.
        let end = (size - 1).min(start + 14);
        let mut builder = String::new();
        let mut index = start;
        while index <= end {
            // A page below 1 throws in Java (`list.get(-14)`): nothing is sent.
            let d = list.get(usize::try_from(index).ok()?)?;
            index += 1;
            let ex_immediate = items
                .as_ref()
                .and_then(|t| t.get(d.item_id))
                .is_some_and(|t| t.ex_immediate_effect);
            let mut rate_chance = 1.0_f64;
            let mut rate_amount = 1.0_f64;
            if d.is_spoil {
                rate_chance = f64::from(settings.rate_spoil_drop_chance_multiplier);
                rate_amount = f64::from(settings.rate_spoil_drop_amount_multiplier);
                rate_chance *= killer.bonus_spoil_rate;
            } else {
                if let Some(by_id) = settings.rate_drop_chance_by_id.get(&d.item_id) {
                    rate_chance *= f64::from(*by_id);
                    if d.item_id == ADENA_ID && rate_chance > 100.0 {
                        rate_chance = 100.0;
                    }
                } else if ex_immediate {
                    rate_chance *= f64::from(settings.rate_herb_drop_chance_multiplier);
                } else if d.is_raid {
                    rate_chance *= f64::from(settings.rate_raid_drop_chance_multiplier);
                } else {
                    rate_chance *= f64::from(settings.rate_death_drop_chance_multiplier);
                }
                if let Some(by_id) = settings.rate_drop_amount_by_id.get(&d.item_id) {
                    rate_amount *= f64::from(*by_id);
                } else if ex_immediate {
                    rate_amount *= f64::from(settings.rate_herb_drop_amount_multiplier);
                } else if d.is_raid {
                    rate_amount *= f64::from(settings.rate_raid_drop_amount_multiplier);
                } else {
                    rate_amount *= f64::from(settings.rate_death_drop_amount_multiplier);
                }
                rate_amount *= killer.bonus_drop_amount;
                if d.item_id == ADENA_ID {
                    rate_amount *= killer.bonus_drop_adena;
                }
                rate_chance *= killer.bonus_drop_rate;
            }
            builder.push_str("<tr>");
            builder.push_str(&format!("<td width=30>{}</td>", d.npc_level));
            builder.push_str(&format!(
                "<td width=170><a action=\"bypass _bbs_npc_trace {0}\">&@{0};</a></td>",
                d.npc_id
            ));
            builder.push_str(&format!(
                "<td width=80 align=CENTER>{}-{}</td>",
                java_double(d.min as f64 * rate_amount),
                java_double(d.max as f64 * rate_amount)
            ));
            builder.push_str(&format!(
                "<td width=50 align=CENTER>{}%</td>",
                chance_format(d.chance * rate_chance)
            ));
            builder.push_str(&format!(
                "<td width=50 align=CENTER>{}</td>",
                if d.is_spoil { "Spoil" } else { "Drop" }
            ));
            builder.push_str("</tr>");
        }
        let mut html = html.replace("%searchResult%", &builder);
        let mut pager = String::from("<tr>");
        for p in 1..=pages {
            pager.push_str(&format!(
                "<td><a action=\"bypass -h _bbs_search_drop {item_id} {p} $order $level\">{p}</a></td>"
            ));
        }
        pager.push_str("</tr>");
        html = html.replace("%pages%", &pager);
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use super::java_double;

    #[test]
    fn doubles_print_like_java() {
        assert_eq!(java_double(1.0), "1.0");
        assert_eq!(java_double(2.5), "2.5");
        assert_eq!(java_double(0.0), "0.0");
        assert_eq!(java_double(12_345.0), "12345.0");
        assert_eq!(java_double(1.0e7), "1.0E7");
        assert_eq!(java_double(1.5e9), "1.5E9");
        assert_eq!(java_double(0.0001), "1.0E-4");
    }
}
