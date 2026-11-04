//! The scheme buffer NPC (`SchemeBuffer.ini`): saved buff schemes, manual casting and auto-buffs.
// There is no switch: the feature exists when an NPC of type `SchemeBuffer` is in the world (the
// shipped one, id 50008, is in `stats/npcs/custom/` and has no spawn unless the owner adds one),
// so nothing changes for a server that has none.

use std::collections::BTreeMap;
use std::sync::Arc;

use l2_data::{BuffSkillHolder, SchemeBufferSkills};
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::outbox::PersistCommand;
use crate::world::World;

/// `SchemeBufferConfig`; `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeBufferSettings {
    /// `BUFFER_MAX_SCHEMES` (`BufferMaxSchemesPerChar`).
    pub max_schemes: i32,
    /// `BUFFER_ITEM_ID` (`BufferItemId`).
    pub item_id: i32,
    /// `BUFFER_STATIC_BUFF_COST` (`BufferStaticCostPerBuff`).
    pub static_buff_cost: i32,
}

impl Default for SchemeBufferSettings {
    fn default() -> Self {
        Self {
            max_schemes: 4,
            item_id: 57,
            static_buff_cost: -1,
        }
    }
}

/// The schemes of one player: Java's `ConcurrentSkipListMap(String.CASE_INSENSITIVE_ORDER)`. The
/// key is the lower case name; the entry keeps the name as it was first written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PlayerSchemes {
    entries: BTreeMap<String, (String, Vec<i32>)>,
}

impl PlayerSchemes {
    fn key(name: &str) -> String {
        name.to_lowercase()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Java `containsKey(name)`.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&Self::key(name))
    }

    /// The schemes by name (case-insensitive order), with their skill ids.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &[i32])> {
        self.entries
            .values()
            .map(|(name, ids)| (name.as_str(), ids.as_slice()))
    }

    fn get(&self, name: &str) -> Option<&Vec<i32>> {
        self.entries.get(&Self::key(name)).map(|(_, ids)| ids)
    }
}

/// Java `SchemeBufferTable._schemesTable`: player to schemes.
#[derive(Debug, Clone, Default)]
pub(crate) struct SchemeTable {
    players: BTreeMap<ObjectId, PlayerSchemes>,
}

impl SchemeTable {
    /// Java `setScheme(playerId, name, skillIds)`: a new name over the limit is ignored.
    pub(crate) fn set_scheme(&mut self, max_schemes: i32, player: ObjectId, name: &str, ids: &[i32]) {
        let schemes = self.players.entry(player).or_default();
        let limit = usize::try_from(max_schemes).unwrap_or(0);
        if schemes.len() >= limit && !schemes.contains(name) {
            return;
        }
        match schemes.entries.get_mut(&PlayerSchemes::key(name)) {
            // `put` on an existing key keeps the key object and replaces the value.
            Some((_, list)) => *list = ids.to_vec(),
            None => {
                schemes
                    .entries
                    .insert(PlayerSchemes::key(name), (name.to_owned(), ids.to_vec()));
            }
        }
    }

    /// Java `deleteScheme`: `true` when a scheme went; a player without schemes left is dropped.
    pub(crate) fn delete_scheme(&mut self, player: ObjectId, name: &str) -> bool {
        let Some(schemes) = self.players.get_mut(&player) else {
            return false;
        };
        let removed = schemes.entries.remove(&PlayerSchemes::key(name)).is_some();
        if schemes.is_empty() {
            self.players.remove(&player);
        }
        removed
    }

    /// Java `addSkillToScheme`.
    pub(crate) fn add_skill(&mut self, player: ObjectId, name: &str, skill_id: i32) -> bool {
        let Some((_, list)) = self
            .players
            .get_mut(&player)
            .and_then(|s| s.entries.get_mut(&PlayerSchemes::key(name)))
        else {
            return false;
        };
        if list.contains(&skill_id) {
            return false;
        }
        list.push(skill_id);
        true
    }

    /// Java `removeSkillFromScheme`.
    pub(crate) fn remove_skill(&mut self, player: ObjectId, name: &str, skill_id: i32) -> bool {
        let Some((_, list)) = self
            .players
            .get_mut(&player)
            .and_then(|s| s.entries.get_mut(&PlayerSchemes::key(name)))
        else {
            return false;
        };
        let Some(at) = list.iter().position(|s| *s == skill_id) else {
            return false;
        };
        list.remove(at);
        true
    }

    /// Java `getPlayerSchemes(playerId)`: `None` for a player without schemes.
    pub(crate) fn player_schemes(&self, player: ObjectId) -> Option<&PlayerSchemes> {
        self.players.get(&player)
    }

    /// Java `getScheme(playerId, name)`: an empty list when there is none.
    pub(crate) fn scheme(&self, player: ObjectId, name: &str) -> &[i32] {
        self.players
            .get(&player)
            .and_then(|s| s.get(name))
            .map_or(&[], Vec::as_slice)
    }

    /// The rows of `buffer_schemes` of a player: `(scheme_name, "1,2,3")`.
    fn rows_of(&self, player: ObjectId) -> Vec<(String, String)> {
        self.players.get(&player).map_or_else(Vec::new, |schemes| {
            schemes
                .iter()
                .map(|(name, ids)| {
                    let skills = ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
                    (name.to_owned(), skills)
                })
                .collect()
        })
    }

    /// Number of players with a scheme.
    pub(crate) fn player_count(&self) -> usize {
        self.players.len()
    }
}

/// What the scheme buffer keeps in the world.
#[derive(Debug, Default)]
pub(crate) struct SchemeBufferState {
    pub(crate) settings: SchemeBufferSettings,
    pub(crate) skills: Option<Arc<SchemeBufferSkills>>,
    pub(crate) table: SchemeTable,
}

/// `PAGE_LIMIT`.
const PAGE_LIMIT: i32 = 6;
/// `SCHEME_NAME_MAX_LENGTH`.
const SCHEME_NAME_MAX_LENGTH: usize = 14;
/// `TYPES_PER_ROW`.
const TYPES_PER_ROW: i32 = 4;
const UI_WIDTH: i32 = 256;
const UI_HALF_WIDTH: i32 = UI_WIDTH / 2;
const ICON_COL_WIDTH: i32 = 40;
const BTN_COL_WIDTH: i32 = 32;
const NAME_COL_WIDTH: i32 = UI_WIDTH - ICON_COL_WIDTH - BTN_COL_WIDTH;
const FOOTER_SIDE_WIDTH: i32 = 70;
const PAGE_CENTER_WIDTH: i32 = UI_WIDTH - (FOOTER_SIDE_WIDTH * 2);

/// Java `new StringTokenizer(text, ";")`: the non-empty parts.
fn tokens(text: &str) -> Vec<&str> {
    text.split(';').filter(|t| !t.is_empty()).collect()
}

/// Java `parseUnsignedInt`: digits only, no overflow of `int`.
fn parse_unsigned_int(value: &str) -> Option<i32> {
    if value.is_empty() {
        return None;
    }
    let mut result: i32 = 0;
    for c in value.chars() {
        if !c.is_ascii_digit() {
            return None;
        }
        let digit = i32::try_from(c.to_digit(10)?).ok()?;
        if result > ((i32::MAX - digit) / 10) {
            return None;
        }
        result = (result * 10) + digit;
    }
    Some(result)
}

/// Java `HtmlUtil.countPageNumber`.
fn count_page_number(total: i32, per_page: i32) -> i32 {
    if per_page <= 0 {
        return 0;
    }
    (total + per_page - 1) / per_page
}

/// Java `StringUtil.isAlphaNumeric`: letters and digits (`Character.isLetterOrDigit`).
fn is_alphanumeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(char::is_alphanumeric)
}

/// Java `NumberFormat.getInstance(Locale.ENGLISH).format(value)` of an integer.
fn grouped(value: i64) -> String {
    crate::residence::npc::format_adena(value)
}

impl World {
    /// Hands the world the scheme buffer settings and its skill list (Java `SchemeBufferConfig`,
    /// `SchemeBufferTable.loadAvailableBuffs`).
    pub fn set_scheme_buffer(&mut self, settings: SchemeBufferSettings, skills: Arc<SchemeBufferSkills>) {
        self.custom_buffers.scheme.settings = settings;
        self.custom_buffers.scheme.skills = Some(skills);
    }

    /// Java `SchemeBufferTable.loadPlayerSchemes`: the rows of `buffer_schemes` as `(object_id,
    /// scheme_name, skills)`. A skill the list does not offer is left out.
    pub fn load_buffer_schemes(&mut self, rows: Vec<(i32, String, String)>) {
        let state = &mut self.custom_buffers.scheme;
        let max = state.settings.max_schemes;
        for (object_id, name, skills) in rows {
            let mut ids = Vec::new();
            for token in skills.split(',') {
                if token.is_empty() {
                    continue;
                }
                let Ok(skill_id) = token.parse::<i32>() else {
                    tracing::warn!(
                        "SchemeBufferTable: Invalid skill id '{token}' for scheme '{name}' (objectId: {object_id})."
                    );
                    continue;
                };
                if state
                    .skills
                    .as_ref()
                    .is_some_and(|s| s.available_buff(skill_id).is_some())
                {
                    ids.push(skill_id);
                }
            }
            state.table.set_scheme(max, ObjectId(object_id), &name, &ids);
        }
        tracing::info!(
            "SchemeBufferTable: Loaded {} players and {} available skills.",
            state.table.player_count(),
            state.skills.as_ref().map_or(0, |s| s.len())
        );
    }

    /// The schemes of a player after a change go to the persistence task.
    fn save_schemes_of(&mut self, player: ObjectId) {
        let schemes = self.custom_buffers.scheme.table.rows_of(player);
        self.outbox.persist(PersistCommand::BufferSchemes {
            object_id: player,
            schemes,
        });
    }

    fn scheme_skills(&self) -> Arc<SchemeBufferSkills> {
        self.custom_buffers.scheme.skills.clone().unwrap_or_default()
    }

    /// Java `SchemeBuffer.getFee(list)`.
    fn scheme_fee(&self, list: &[i32]) -> i64 {
        let state = &self.custom_buffers.scheme;
        if state.settings.static_buff_cost > 0 {
            return list.len() as i64 * i64::from(state.settings.static_buff_cost);
        }
        let skills = self.scheme_skills();
        // Java's `fee` is an `int`.
        let fee: i32 = list
            .iter()
            .filter_map(|id| skills.available_buff(*id))
            .fold(0i32, |sum, holder| sum.wrapping_add(holder.price));
        i64::from(fee)
    }

    /// Java `SchemeBuffer.getCountOf(skills, dances)`.
    fn scheme_count_of(&self, skills: &[i32], dances: bool) -> i32 {
        let Some(table) = self.skill_table() else {
            return 0;
        };
        let count = skills
            .iter()
            .filter(|id| table.get(**id, 1).is_some_and(|s| s.is_dance() == dances))
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `skill.applyEffects(this, target)` for a skill id at a level.
    fn scheme_apply(&mut self, npc: ObjectId, target: ObjectId, skill_id: i32, level: i32) {
        if self
            .skill_table()
            .is_some_and(|t| t.get(skill_id, level).is_some())
        {
            self.apply_effects(npc, target, SkillKey { id: skill_id, level });
        }
    }

    /// Java `SchemeBuffer.onBypassFeedback(player, commandValue)`. `false`: the command is not the
    /// buffer's, `Npc.onBypassFeedback` takes it (Java's `super` call).
    pub(crate) fn scheme_buffer_bypass_feedback(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) -> bool {
        if command.is_empty() {
            return true;
        }
        let replaced = command.replace("createscheme ", "createscheme;");
        let mut tokens = tokens(&replaced).into_iter();
        let Some(current) = tokens.next() else {
            return true;
        };
        let rest: Vec<&str> = tokens.collect();
        let mut next = rest.iter().copied();

        if current.starts_with("menu") {
            self.scheme_show_main_menu(npc, player);
        } else if current.starts_with("cleanup") {
            self.stop_all_effects(player);
            if let Some(summon) = self.summon_of(player) {
                self.stop_all_effects(summon);
            }
            self.scheme_show_main_menu(npc, player);
        } else if current.starts_with("heal") {
            self.scheme_refill(player, true);
            if let Some(summon) = self.summon_of(player) {
                self.scheme_refill(summon, false);
            }
            self.scheme_show_main_menu(npc, player);
        } else if current.starts_with("support") {
            self.scheme_show_give_buffs_window(npc, player);
        } else if current.starts_with("givebuffs") {
            self.scheme_give_buffs(npc, player, &rest);
        } else if current.starts_with("editschemes") {
            if rest.len() < 3 {
                return true;
            }
            let (group_type, scheme_name) = (rest[0], rest[1]);
            let Some(page) = parse_unsigned_int(rest[2]) else {
                return true;
            };
            self.scheme_show_edit_window(npc, player, group_type, scheme_name, page);
        } else if current.starts_with("skill") {
            self.scheme_skill_command(npc, player, current, &rest);
        } else if current.starts_with("createscheme") {
            self.scheme_create(npc, player, &rest);
        } else if current.starts_with("deletescheme") {
            let Some(scheme_name) = next.next() else {
                self.send_message(player, "This scheme name is invalid.");
                self.scheme_show_give_buffs_window(npc, player);
                return true;
            };
            self.custom_buffers
                .scheme
                .table
                .delete_scheme(player, scheme_name);
            // Java does not save here either; the rows follow the change.
            self.save_schemes_of(player);
            self.scheme_show_give_buffs_window(npc, player);
        } else if current.starts_with("manual") {
            let category = next.next().unwrap_or("Buffs");
            let mut page = 1;
            if let Some(token) = next.next() {
                let Some(parsed) = parse_unsigned_int(token) else {
                    self.scheme_show_main_menu(npc, player);
                    return true;
                };
                page = parsed;
            }
            let target_type = next.next().unwrap_or("me");
            self.scheme_show_manual_window(npc, player, category, page, target_type);
        } else if current.starts_with("castbuff") {
            self.scheme_cast_buff(npc, player, &rest);
        } else if current.starts_with("autobuff") {
            let target_type = next.next().unwrap_or("me");
            self.scheme_auto_buff(npc, player, target_type);
        } else {
            return false;
        }
        true
    }

    /// `setCurrentHpMp(getMaxHp(), getMaxMp())` and, for the player, `setCurrentCp(getMaxCp())`.
    fn scheme_refill(&mut self, id: ObjectId, with_cp: bool) {
        let Some((max_hp, max_mp, max_cp)) = self.stats(id).map(|v| {
            (
                f64::from(v.max_hp()),
                f64::from(v.max_mp()),
                f64::from(v.max_cp()),
            )
        }) else {
            return;
        };
        self.set_current_hp_mp(id, max_hp, max_mp);
        if with_cp {
            self.set_current_cp(id, max_cp, true);
        }
    }

    fn scheme_html_path(&self, npc: ObjectId, value: i32) -> String {
        // `data/html/mods/SchemeBuffer/<id>.htm` / `<id>-<value>.htm`.
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        if value == 0 {
            format!("data/html/mods/SchemeBuffer/{npc_id}.htm")
        } else {
            format!("data/html/mods/SchemeBuffer/{npc_id}-{value}.htm")
        }
    }

    /// A page of the NPC with the replacements applied in order, then sent.
    fn scheme_send_page(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        value: i32,
        replacements: &[(&str, String)],
    ) {
        let path = self.scheme_html_path(npc, value);
        let Some(mut message) = self.html_from_file(player, npc.0, &path) else {
            return;
        };
        for (pattern, text) in replacements {
            message.html = message.html.replace(pattern, text);
        }
        self.send_html(player, &message);
    }

    /// Java `showMainMenu`.
    fn scheme_show_main_menu(&mut self, npc: ObjectId, player: ObjectId) {
        self.scheme_send_page(npc, player, 0, &[("%objectId%", npc.0.to_string())]);
    }

    /// Java `showGiveBuffsWindow`.
    fn scheme_show_give_buffs_window(&mut self, npc: ObjectId, player: ObjectId) {
        let mut html = String::new();
        let schemes = self.custom_buffers.scheme.table.player_schemes(player);
        match schemes.filter(|s| !s.is_empty()) {
            None => html.push_str("<font color=\"LEVEL\">You haven't defined any scheme.</font>"),
            Some(schemes) => {
                for (name, ids) in schemes.iter() {
                    let cost = self.scheme_fee(ids);
                    html.push_str(&format!("<font color=\"LEVEL\">{name} [{} skill(s)]", ids.len()));
                    if cost > 0 {
                        html.push_str(&format!(" - cost: {}", grouped(cost)));
                    }
                    html.push_str("</font><br1>");
                    html.push_str(&format!(
                        "<a action=\"bypass -h npc_%objectId%_givebuffs;{name};{cost}\">Use on Me</a>&nbsp;|&nbsp;"
                    ));
                    html.push_str(&format!(
                        "<a action=\"bypass -h npc_%objectId%_givebuffs;{name};{cost};pet\">Use on Pet</a>&nbsp;|&nbsp;"
                    ));
                    html.push_str(&format!(
                        "<a action=\"bypass -h npc_%objectId%_editschemes;Buffs;{name};1\">Edit</a>&nbsp;|&nbsp;"
                    ));
                    html.push_str(&format!(
                        "<a action=\"bypass -h npc_%objectId%_deletescheme;{name}\">Delete</a><br>"
                    ));
                }
            }
        }
        let max = self.custom_buffers.scheme.settings.max_schemes.to_string();
        self.scheme_send_page(
            npc,
            player,
            1,
            &[
                ("%schemes%", html),
                ("%max_schemes%", max),
                ("%objectId%", npc.0.to_string()),
            ],
        );
    }

    /// Java `showEditSchemeWindow`.
    fn scheme_show_edit_window(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        group_type: &str,
        scheme_name: &str,
        page: i32,
    ) {
        let scheme_skills = self
            .custom_buffers
            .scheme
            .table
            .scheme(player, scheme_name)
            .to_vec();
        let count = format!(
            "{} / {} buffs, {} / {} dances/songs",
            self.scheme_count_of(&scheme_skills, false),
            self.max_buff_count(player),
            self.scheme_count_of(&scheme_skills, true),
            self.settings.effects.dances_max_amount
        );
        let types_frame = self.scheme_types_frame(group_type, scheme_name);
        let list_frame = self.scheme_group_skill_list(npc, player, group_type, scheme_name, page);
        self.scheme_send_page(
            npc,
            player,
            2,
            &[
                ("%schemename%", scheme_name.to_owned()),
                ("%count%", count),
                ("%typesframe%", types_frame),
                ("%skilllistframe%", list_frame),
                ("%objectId%", npc.0.to_string()),
            ],
        );
    }

    /// Java `getTypesFrame`.
    fn scheme_types_frame(&self, group_type: &str, scheme_name: &str) -> String {
        let skills = self.scheme_skills();
        let mut html = String::from("<table>");
        let mut count = 0;
        for kind in skills.skill_types() {
            if count == 0 {
                html.push_str("<tr>");
            }
            if group_type.eq_ignore_ascii_case(kind) {
                html.push_str(&format!("<td width=65>{kind}</td>"));
            } else {
                html.push_str(&format!(
                    "<td width=65><a action=\"bypass -h npc_%objectId%_editschemes;{kind};{scheme_name};1\">{kind}</a></td>"
                ));
            }
            count += 1;
            if count == TYPES_PER_ROW {
                html.push_str("</tr>");
                count = 0;
            }
        }
        if count != 0 {
            html.push_str("</tr>");
        }
        html.push_str("</table>");
        html
    }

    /// The row colour and the footer of the two skill lists.
    fn scheme_row_open(row: i32) -> String {
        if row % 2 == 0 {
            format!("<table width=\"{UI_WIDTH}\" bgcolor=\"000000\"><tr>")
        } else {
            format!("<table width=\"{UI_WIDTH}\"><tr>")
        }
    }

    /// The footer both lists end with; `link` builds the bypass of a page.
    fn scheme_footer(page: i32, max_page: i32, link: &dyn Fn(i32) -> String) -> String {
        let mut html = format!(
            "<br1><img src=\"L2UI.SquareGray\" width={UI_WIDTH} height=1><table width=\"{UI_WIDTH}\" bgcolor=000000><tr>"
        );
        if page > 1 {
            html.push_str(&format!(
                "<td align=left width={FOOTER_SIDE_WIDTH}><a action=\"{}\">Previous</a></td>",
                link(page - 1)
            ));
        } else {
            html.push_str(&format!("<td align=left width={FOOTER_SIDE_WIDTH}>Previous</td>"));
        }
        html.push_str(&format!(
            "<td align=center width={PAGE_CENTER_WIDTH}>Page {page}</td>"
        ));
        if page < max_page {
            html.push_str(&format!(
                "<td align=right width={FOOTER_SIDE_WIDTH}><a action=\"{}\">Next</a></td>",
                link(page + 1)
            ));
        } else {
            html.push_str(&format!("<td align=right width={FOOTER_SIDE_WIDTH}>Next</td>"));
        }
        html.push_str(&format!(
            "</tr></table><img src=\"L2UI.SquareGray\" width={UI_WIDTH} height=1>"
        ));
        html
    }

    /// The ids of one page of a category: `(page, max_page, ids)`.
    fn scheme_page_of(&self, category: &str, page_value: i32) -> (i32, i32, Vec<i32>) {
        let skills = self.scheme_skills();
        let ids = skills.skill_ids_by_type(category);
        let total = i32::try_from(ids.len()).unwrap_or(i32::MAX);
        let max_page = count_page_number(total, PAGE_LIMIT);
        let mut page = page_value;
        if page > max_page {
            page = max_page;
        }
        if page < 1 {
            page = 1;
        }
        let from = usize::try_from((page - 1) * PAGE_LIMIT)
            .unwrap_or(0)
            .min(ids.len());
        let to = usize::try_from(page * PAGE_LIMIT).unwrap_or(0).min(ids.len());
        (page, max_page, ids[from..to].to_vec())
    }

    /// Java `getGroupSkillList`.
    fn scheme_group_skill_list(
        &self,
        npc: ObjectId,
        player: ObjectId,
        group_type: &str,
        scheme_name: &str,
        page_value: i32,
    ) -> String {
        let skills = self.scheme_skills();
        if skills.skill_ids_by_type(group_type).is_empty() {
            return "That group doesn't contain any skills.".to_owned();
        }
        let (page, max_page, ids) = self.scheme_page_of(group_type, page_value);
        let scheme_skills = self
            .custom_buffers
            .scheme
            .table
            .scheme(player, scheme_name)
            .to_vec();
        let Some(table) = self.skill_table() else {
            return String::new();
        };
        let mut html = String::new();
        let mut row = 0;
        for skill_id in ids {
            let Some(holder) = skills.available_buff_in(group_type, skill_id) else {
                continue;
            };
            let Some(skill) = table.get(skill_id, 1) else {
                continue;
            };
            html.push_str(&Self::scheme_row_open(row));
            html.push_str(&format!(
                "<td height=40 width={ICON_COL_WIDTH} align=center><img src=\"{}\" width=32 height=32></td>",
                skill.icon
            ));
            html.push_str(&format!(
                "<td width={NAME_COL_WIDTH}>{}<br1><font color=\"B09878\">{}</font></td>",
                skill.name, holder.description
            ));
            if scheme_skills.contains(&skill_id) {
                html.push_str(&format!(
                    "<td width={BTN_COL_WIDTH} align=center><button action=\"bypass -h npc_%objectId%_skillunselect;{group_type};{scheme_name};{skill_id};{page}\" width=32 height=32 back=\"L2UI_CH3.mapbutton_zoomout2\" fore=\"L2UI_CH3.mapbutton_zoomout1\"></td>"
                ));
            } else {
                html.push_str(&format!(
                    "<td width={BTN_COL_WIDTH} align=center><button action=\"bypass -h npc_%objectId%_skillselect;{group_type};{scheme_name};{skill_id};{page}\" width=32 height=32 back=\"L2UI_CH3.mapbutton_zoomin2\" fore=\"L2UI_CH3.mapbutton_zoomin1\"></td>"
                ));
            }
            html.push_str(&format!(
                "</tr></table><img src=\"L2UI.SquareGray\" width={UI_WIDTH} height=1>"
            ));
            row += 1;
        }
        let object_id = npc.0;
        html.push_str(&Self::scheme_footer(page, max_page, &|p| {
            format!("bypass -h npc_{object_id}_editschemes;{group_type};{scheme_name};{p}")
        }));
        html
    }

    /// Java `showManualWindow`.
    fn scheme_show_manual_window(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        category: &str,
        page_value: i32,
        target_type: &str,
    ) {
        let skills = self.scheme_skills();
        if skills.skill_ids_by_type(category).is_empty() {
            self.send_message(player, "That category doesn't contain any skills.");
            return;
        }
        let (page, max_page, ids) = self.scheme_page_of(category, page_value);
        let object_id = npc.0;
        let Some(table) = self.skill_table() else {
            return;
        };
        let mut html = format!("<table width=\"{UI_WIDTH}\"><tr>");
        html.push_str(&format!("<td width=\"{UI_HALF_WIDTH}\" align=\"center\">"));
        if target_type.eq_ignore_ascii_case("me") {
            html.push_str("<font color=\"LEVEL\">Me</font>");
        } else {
            html.push_str(&format!(
                "<a action=\"bypass -h npc_{object_id}_manual;{category};{page};me\">Me</a>"
            ));
        }
        html.push_str("</td>");
        html.push_str(&format!("<td width=\"{UI_HALF_WIDTH}\" align=\"center\">"));
        if target_type.eq_ignore_ascii_case("pet") {
            html.push_str("<font color=\"LEVEL\">Pet</font>");
        } else {
            html.push_str(&format!(
                "<a action=\"bypass -h npc_{object_id}_manual;{category};{page};pet\">Pet</a>"
            ));
        }
        html.push_str("</td>");
        html.push_str("</tr></table><br1>");

        let mut row = 0;
        for skill_id in ids {
            let Some(holder) = skills.available_buff_in(category, skill_id) else {
                continue;
            };
            let Some(skill) = table.get(skill_id, holder.level) else {
                continue;
            };
            html.push_str(&Self::scheme_row_open(row));
            html.push_str(&format!(
                "<td height=40 width={ICON_COL_WIDTH} align=center><img src=\"{}\" width=32 height=32></td>",
                skill.icon
            ));
            html.push_str(&format!("<td width={NAME_COL_WIDTH}>{}<br1>", skill.name));
            html.push_str(&format!(
                "<font color=\"B09878\">{}</font></td>",
                holder.description
            ));
            html.push_str(&format!(
                "<td width={BTN_COL_WIDTH} align=center><button action=\"bypass -h npc_{object_id}_castbuff;{skill_id};{category};{page};{target_type}\" width=32 height=32 back=\"L2UI_CH3.mapbutton_zoomin2\" fore=\"L2UI_CH3.mapbutton_zoomin1\"></td>"
            ));
            html.push_str("</tr></table>");
            html.push_str(&format!(
                "<img src=\"L2UI.SquareGray\" width={UI_WIDTH} height=1>"
            ));
            row += 1;
        }
        html.push_str(&Self::scheme_footer(page, max_page, &|p| {
            format!("bypass -h npc_{object_id}_manual;{category};{p};{target_type}")
        }));
        html.push_str(&format!(
            "<br1><center><a action=\"bypass -h npc_{object_id}_menu\">Back</a></center>"
        ));

        self.scheme_send_page(
            npc,
            player,
            3,
            &[
                ("%category%", category.to_owned()),
                ("%skills%", html),
                ("%objectId%", npc.0.to_string()),
            ],
        );
    }

    /// The `givebuffs` command.
    fn scheme_give_buffs(&mut self, npc: ObjectId, player: ObjectId, args: &[&str]) {
        let mut next = args.iter().copied();
        let Some(scheme_name) = next.next() else {
            return;
        };
        // Compatibility: keep consuming the bypass cost token, but never trust it.
        let _cost_token = next.next();

        if !self
            .custom_buffers
            .scheme
            .table
            .player_schemes(player)
            .is_some_and(|s| s.contains(scheme_name))
        {
            self.send_message(player, &format!("Invalid scheme name: {scheme_name}."));
            self.scheme_show_give_buffs_window(npc, player);
            return;
        }
        let scheme = self
            .custom_buffers
            .scheme
            .table
            .scheme(player, scheme_name)
            .to_vec();
        let cost = self.scheme_fee(&scheme);

        let mut target = Some(player);
        if next.next().is_some_and(|t| t.eq_ignore_ascii_case("pet")) {
            target = self.summon_of(player);
        }
        let Some(target) = target else {
            self.send_message(player, "You don't have a pet.");
            return;
        };

        let item_id = self.custom_buffers.scheme.settings.item_id;
        let paid = cost == 0
            || (item_id == 57 && self.reduce_adena(player, cost, true))
            || (item_id != 57 && self.destroy_item_by_item_id(player, item_id, cost, true));
        if paid {
            let skills = self.scheme_skills();
            for skill_id in scheme {
                let Some(holder) = skills.available_buff(skill_id) else {
                    continue;
                };
                self.scheme_apply(npc, target, skill_id, holder.level);
            }
        }
    }

    /// The `skill*` commands (`skillselect`, `skillunselect`).
    fn scheme_skill_command(&mut self, npc: ObjectId, player: ObjectId, current: &str, args: &[&str]) {
        if args.len() < 4 {
            return;
        }
        let (group_type, scheme_name) = (args[0], args[1]);
        let (Some(skill_id), Some(page)) = (parse_unsigned_int(args[2]), parse_unsigned_int(args[3])) else {
            return;
        };
        if !self
            .custom_buffers
            .scheme
            .table
            .player_schemes(player)
            .is_some_and(|s| s.contains(scheme_name))
        {
            self.send_message(player, &format!("Invalid scheme name: {scheme_name}."));
            self.scheme_show_give_buffs_window(npc, player);
            return;
        }
        let scheme_skills = self
            .custom_buffers
            .scheme
            .table
            .scheme(player, scheme_name)
            .to_vec();

        if current.starts_with("skillselect") && !scheme_name.eq_ignore_ascii_case("none") {
            let skill = self
                .skill_table()
                .and_then(|t| t.get(skill_id, 1).map(|s| s.is_dance()));
            let Some(is_dance) = skill else {
                self.scheme_show_edit_window(npc, player, group_type, scheme_name, page);
                return;
            };
            if is_dance {
                if self.scheme_count_of(&scheme_skills, true) < self.settings.effects.dances_max_amount {
                    if !scheme_skills.contains(&skill_id)
                        && self
                            .custom_buffers
                            .scheme
                            .table
                            .add_skill(player, scheme_name, skill_id)
                    {
                        self.save_schemes_of(player);
                    }
                } else {
                    self.send_message(
                        player,
                        "This scheme has reached the maximum amount of dances/songs.",
                    );
                }
            } else if self.scheme_count_of(&scheme_skills, false) < self.max_buff_count(player) {
                if !scheme_skills.contains(&skill_id)
                    && self
                        .custom_buffers
                        .scheme
                        .table
                        .add_skill(player, scheme_name, skill_id)
                {
                    self.save_schemes_of(player);
                }
            } else {
                self.send_message(player, "This scheme has reached the maximum amount of buffs.");
            }
        } else if current.starts_with("skillunselect")
            && self
                .custom_buffers
                .scheme
                .table
                .remove_skill(player, scheme_name, skill_id)
        {
            self.save_schemes_of(player);
        }
        self.scheme_show_edit_window(npc, player, group_type, scheme_name, page);
    }

    /// The `createscheme` command.
    fn scheme_create(&mut self, npc: ObjectId, player: ObjectId, args: &[&str]) {
        let too_long = format!("Scheme's name must contain up to {SCHEME_NAME_MAX_LENGTH} chars.");
        let Some(first) = args.first() else {
            self.send_message(player, &too_long);
            return;
        };
        let scheme_name = first.trim();
        if scheme_name.is_empty() || scheme_name.chars().count() > SCHEME_NAME_MAX_LENGTH {
            self.send_message(player, &too_long);
            return;
        }
        let normalized: String = scheme_name
            .replace(' ', "")
            .replace(['.', ',', '-', '+', '!', '?'], "");
        if !is_alphanumeric(&normalized) {
            self.send_message(player, "Please use plain alphanumeric characters.");
            return;
        }
        let max = self.custom_buffers.scheme.settings.max_schemes;
        if let Some(schemes) = self.custom_buffers.scheme.table.player_schemes(player) {
            if i64::try_from(schemes.len()).unwrap_or(i64::MAX) >= i64::from(max) {
                self.send_message(player, "Maximum schemes amount is already reached.");
                return;
            }
            if schemes.contains(scheme_name) {
                self.send_message(player, &format!("The scheme name already exists: {scheme_name}."));
                return;
            }
        }
        self.custom_buffers
            .scheme
            .table
            .set_scheme(max, player, scheme_name, &[]);
        self.save_schemes_of(player);
        self.scheme_show_give_buffs_window(npc, player);
    }

    /// The `castbuff` command.
    fn scheme_cast_buff(&mut self, npc: ObjectId, player: ObjectId, args: &[&str]) {
        if args.len() < 4 {
            return;
        }
        let skill_id = parse_unsigned_int(args[0]);
        let category = args[1];
        let page = parse_unsigned_int(args[2]);
        let target_type = args[3];
        let (Some(skill_id), Some(page)) = (skill_id, page) else {
            return;
        };
        let target = if target_type.eq_ignore_ascii_case("pet") {
            self.summon_of(player)
        } else {
            Some(player)
        };
        let Some(target) = target else {
            self.send_message(player, "You don't have a pet.");
            self.scheme_show_manual_window(npc, player, category, page, target_type);
            return;
        };
        let holder: Option<BuffSkillHolder> = self
            .scheme_skills()
            .available_buff_in(category, skill_id)
            .cloned();
        if let Some(holder) = holder {
            self.scheme_apply(npc, target, skill_id, holder.level);
        }
        self.scheme_show_manual_window(npc, player, category, page, target_type);
    }

    /// Java `applyAutoBuff`.
    fn scheme_auto_buff(&mut self, npc: ObjectId, player: ObjectId, target_type: &str) {
        let target = if target_type.eq_ignore_ascii_case("pet") {
            self.summon_of(player)
        } else {
            Some(player)
        };
        let Some(target) = target else {
            self.send_message(player, "You don't have a pet.");
            self.scheme_show_main_menu(npc, player);
            return;
        };
        let is_mage = self
            .players
            .get(&player)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .is_some_and(|class| l2_data::ClassId(class).is_mage());
        let category = if is_mage { "MAGE_GROUP" } else { "FIGHTER_GROUP" };
        let skills = self.scheme_skills();
        let ids = skills.skill_ids_by_type(category).to_vec();
        if ids.is_empty() {
            self.send_message(player, "Auto buff configuration is not available.");
            self.scheme_show_main_menu(npc, player);
            return;
        }
        for skill_id in ids {
            let Some(holder) = skills.available_buff_in(category, skill_id) else {
                continue;
            };
            self.scheme_apply(npc, target, skill_id, holder.level);
        }
        self.send_message(player, "Auto buff applied successfully!");
        self.scheme_show_main_menu(npc, player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_skip_empty_parts() {
        assert_eq!(tokens(";a;;b;"), ["a", "b"]);
        assert!(tokens("").is_empty());
    }

    #[test]
    fn unsigned_ints_are_strict() {
        assert_eq!(parse_unsigned_int("12"), Some(12));
        assert_eq!(parse_unsigned_int(""), None);
        assert_eq!(parse_unsigned_int("-1"), None);
        assert_eq!(parse_unsigned_int("1a"), None);
        assert_eq!(parse_unsigned_int("2147483647"), Some(i32::MAX));
        assert_eq!(parse_unsigned_int("2147483648"), None);
    }

    #[test]
    fn page_numbers() {
        assert_eq!(count_page_number(0, 6), 0);
        assert_eq!(count_page_number(6, 6), 1);
        assert_eq!(count_page_number(7, 6), 2);
        assert_eq!(count_page_number(7, 0), 0);
    }

    #[test]
    fn schemes_are_case_insensitive_and_limited() {
        let mut table = SchemeTable::default();
        let me = ObjectId(1);
        table.set_scheme(2, me, "Alpha", &[1, 2]);
        table.set_scheme(2, me, "beta", &[]);
        // A third name is over the limit and ignored.
        table.set_scheme(2, me, "gamma", &[3]);
        assert_eq!(table.player_schemes(me).unwrap().len(), 2);
        // The same name in another case replaces the value and keeps the first spelling.
        table.set_scheme(2, me, "ALPHA", &[9]);
        assert_eq!(table.scheme(me, "alpha"), [9]);
        assert_eq!(
            table.player_schemes(me).unwrap().iter().next().unwrap().0,
            "Alpha"
        );
        assert!(table.add_skill(me, "BETA", 5));
        assert!(!table.add_skill(me, "BETA", 5));
        assert!(table.remove_skill(me, "beta", 5));
        assert!(!table.remove_skill(me, "beta", 5));
        assert!(table.delete_scheme(me, "alpha"));
        assert!(table.delete_scheme(me, "beta"));
        // The last scheme gone: the player has no entry.
        assert!(table.player_schemes(me).is_none());
        assert!(!table.delete_scheme(me, "beta"));
        assert_eq!(table.scheme(me, "x"), [] as [i32; 0]);
    }

    #[test]
    fn rows_join_the_ids() {
        let mut table = SchemeTable::default();
        let me = ObjectId(1);
        table.set_scheme(4, me, "B", &[3, 4]);
        table.set_scheme(4, me, "a", &[]);
        assert_eq!(
            table.rows_of(me),
            [
                ("a".to_owned(), String::new()),
                ("B".to_owned(), "3,4".to_owned())
            ]
        );
        assert!(table.rows_of(ObjectId(2)).is_empty());
    }

    #[test]
    fn the_scheme_name_filter() {
        assert!(is_alphanumeric("Abc123"));
        assert!(!is_alphanumeric("a_b"));
        assert!(!is_alphanumeric("a;b"));
        assert!(!is_alphanumeric(""));
    }
}
