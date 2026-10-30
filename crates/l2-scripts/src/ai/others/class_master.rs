//! The class master (Mr. Cat and Miss Queen) of `config/ClassMaster.xml`: class changes for a
//! price, noblesse, the third class, clan levels.
// Java: dist/game/data/scripts/ai/others/ClassMaster/ClassMaster.java
//
// # The configuration
// Java reads `config/ClassMaster.xml` in the constructor and keeps it in fields. Here the parsed
// file is a [`ClassMasterConfig`] (loaded by `l2-data`, S-DATA-01) the script is built with
// (read-only, like the Java fields after `load()`); [`ClassMasterConfig::default`] is the shipped
// file (`classChangeEnabled="false"`). The game server builds the script with the loaded file
// (`l2_scripts::all_with`).
//
// # Left out, with the system that brings it
// - `ON_PLAYER_BYPASS` (the `Script ClassMaster ...` links of the tutorial window),
//   `ON_PLAYER_PRESS_TUTORIAL_MARK` (the question mark 1001), `ON_PLAYER_LEVEL_CHANGED` and
//   `ON_PLAYER_PROFESSION_CHANGE`: the engine has no player listeners yet. The login popup runs
//   on `on_enter_world` (Java `ON_PLAYER_LOGIN`); [`ClassMaster::on_player_press_tutorial_mark`]
//   is ported for the listener to call.

use l2_util::ObjectId;
use l2_world::subclass::categories::{ClassChangeData, ClassMasterConfig};
use l2_world::subclass::{CategoryType, ClassId};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::warn;

// NPCs
const CLASS_MASTERS: [i32; 2] = [
    31756, // Mr. Cat
    31757, // Miss Queen
];

/// `PlayerClass` ids the occupation pages switch on.
mod class {
    pub const FIGHTER: i32 = 0;
    pub const WARRIOR: i32 = 1;
    pub const KNIGHT: i32 = 4;
    pub const ROGUE: i32 = 7;
    pub const MAGE: i32 = 10;
    pub const WIZARD: i32 = 11;
    pub const CLERIC: i32 = 15;
    pub const ELVEN_FIGHTER: i32 = 18;
    pub const ELVEN_KNIGHT: i32 = 19;
    pub const ELVEN_SCOUT: i32 = 22;
    pub const ELVEN_MAGE: i32 = 25;
    pub const ELVEN_WIZARD: i32 = 26;
    pub const ORACLE: i32 = 29;
    pub const DARK_FIGHTER: i32 = 31;
    pub const PALUS_KNIGHT: i32 = 32;
    pub const ASSASSIN: i32 = 35;
    pub const DARK_MAGE: i32 = 38;
    pub const DARK_WIZARD: i32 = 39;
    pub const SHILLIEN_ORACLE: i32 = 42;
    pub const ORC_FIGHTER: i32 = 44;
    pub const ORC_RAIDER: i32 = 45;
    pub const ORC_MONK: i32 = 47;
    pub const ORC_MAGE: i32 = 49;
    pub const ORC_SHAMAN: i32 = 50;
    pub const DWARVEN_FIGHTER: i32 = 53;
    pub const SCAVENGER: i32 = 54;
    pub const ARTISAN: i32 = 56;
    pub const MALE_SOLDIER: i32 = 123;
    pub const FEMALE_SOLDIER: i32 = 124;
    pub const TROOPER: i32 = 125;
    pub const WARDER: i32 = 126;
}

/// Java `ai.others.ClassMaster.ClassMaster`.
#[derive(Default)]
pub struct ClassMaster {
    config: ClassMasterConfig,
}

/// Java `load()`'s spawns: `addSpawn(npcId, x, y, z, heading)`.
const SPAWNS: [(i32, i32, i32, i32, i32); 52] = [
    (31756, -116948, 46841, 360, 3355),    // Aden
    (31756, -84336, 242156, -3725, 24500), // Gludio
    (31756, -84119, 243254, -3725, 8000),  // Gludio
    (31756, -82032, 150160, -3122, 16500), // Gludio
    (31756, -44979, -113508, -194, 32000), // Rune
    (31756, -14048, 123184, -3115, 32000), // Gludio
    (31756, 10968, 17540, -4567, 55000),   // Oren
    (31756, 11340, 15972, -4577, 14000),   // Oren
    (31756, 15584, 142784, -2699, 16500),  // Dion
    (31756, 17956, 170536, -3499, 48000),  // Dion
    (31756, 44176, -48732, -800, 33000),   // Rune
    (31756, 44319, -47640, -792, 50000),   // Rune
    (31756, 45472, 49312, -3067, 53000),   // Oren
    (31756, 47648, 51296, -2989, 38500),   // Oren
    (31756, 81136, 54576, -1517, 32000),   // Oren
    (31756, 83076, 147912, -3467, 32000),  // Giran
    (31756, 87596, -140674, -1536, 16500), // Schuttgart
    (31756, 87824, -142256, -1336, 7342),  // Schuttgart
    (31756, 110592, 220400, -3667, 0),     // Innadril
    (31756, 114880, -178144, -827, 0),     // Schuttgart
    (31756, 116224, -181728, -1373, 0),    // Schuttgart
    (31756, 117200, 75824, -2725, 25000),  // Aden
    (31756, 147262, -56450, -2776, 33000), // Goddard
    (31756, 147728, 27408, -2198, 16500),  // Aden
    (31756, 147888, -58048, -2979, 49000), // Goddard
    (31756, 148560, -57952, -2974, 53000), // Goddard
    (31757, -116902, 46841, 360, 49151),   // Aden
    (31757, -84294, 242204, -3725, 24500), // Gludio
    (31757, -84047, 243193, -3725, 8000),  // Gludio
    (31757, -81967, 150160, -3122, 16500), // Gludio
    (31757, -44983, -113554, -194, 32000), // Rune
    (31757, -14050, 123229, -3115, 32000), // Gludio
    (31757, 10918, 17511, -4567, 55000),   // Oren
    (31757, 11353, 16022, -4577, 14000),   // Oren
    (31757, 15631, 142778, -2699, 16500),  // Dion
    (31757, 17913, 170536, -3499, 48000),  // Dion
    (31757, 44176, -48688, -800, 33000),   // Rune
    (31757, 44294, -47642, -792, 50000),   // Rune
    (31757, 45414, 49296, -3067, 53000),   // Oren
    (31757, 47680, 51255, -2989, 38500),   // Oren
    (31757, 81126, 54519, -1517, 32000),   // Oren
    (31757, 83082, 147845, -3467, 32000),  // Giran
    (31757, 87644, -140674, -1536, 16500), // Schuttgart
    (31757, 87856, -142272, -1336, 44000), // Schuttgart
    (31757, 110592, 220443, -3667, 0),     // Innadril
    (31757, 114880, -178196, -827, 0),     // Schuttgart
    (31757, 116218, -181793, -1379, 0),    // Schuttgart
    (31757, 117160, 75784, -2725, 25000),  // Aden
    (31757, 147285, -56461, -2776, 11500), // Goddard
    (31757, 147761, 27408, -2198, 16500),  // Aden
    (31757, 147924, -58052, -2979, 49000), // Goddard
    (31757, 148514, -57972, -2974, 53000), // Goddard
];

/// Java `ClassChangeData.isInCategory(player)`: the player is in one of the option's categories.
fn option_in_category(option: &ClassChangeData, ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    option
        .applied_categories
        .iter()
        .any(|&c| ctx.world().is_in_category(player, c))
}

fn level_of(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.player_level(player)
}

fn in_category(ctx: &ScriptCtx<'_>, player: ObjectId, category: CategoryType) -> bool {
    ctx.world().is_in_category(player, category)
}

fn class_of(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world().player(player).map_or(-1, |p| p.data.class_id)
}

impl ClassMaster {
    /// The script with a configuration (Java's fields after `load()`).
    pub fn new(config: ClassMasterConfig) -> Self {
        Self { config }
    }

    /// Java `getFirstOccupationChangeHtml(player)`.
    fn get_first_occupation_change_html(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<&'static str> {
        use class::*;
        let mut htmltext = None;
        if in_category(ctx, player, CategoryType::FirstClassGroup) {
            if level_of(ctx, player) < 20 {
                htmltext = Some("test_server_helper027.html");
            } else {
                htmltext = match class_of(ctx, player) {
                    FIGHTER => Some("test_server_helper026a.html"),
                    MAGE => Some("test_server_helper026b.html"),
                    ELVEN_FIGHTER => Some("test_server_helper026c.html"),
                    ELVEN_MAGE => Some("test_server_helper026d.html"),
                    DARK_FIGHTER => Some("test_server_helper026e.html"),
                    DARK_MAGE => Some("test_server_helper026f.html"),
                    ORC_FIGHTER => Some("test_server_helper026g.html"),
                    ORC_MAGE => Some("test_server_helper026h.html"),
                    DWARVEN_FIGHTER => Some("test_server_helper026i.html"),
                    MALE_SOLDIER => Some("test_server_helper026j.html"),
                    FEMALE_SOLDIER => Some("test_server_helper026k.html"),
                    _ => None,
                };
            }
        } else if in_category(ctx, player, CategoryType::SecondClassGroup) {
            htmltext = Some("test_server_helper028.html");
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            htmltext = Some("test_server_helper010.html");
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            htmltext = Some("test_server_helper011.html");
        }
        htmltext
    }

    /// Java `getSecondOccupationChangeHtml(player)`.
    fn get_second_occupation_change_html(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<&'static str> {
        use class::*;
        if in_category(ctx, player, CategoryType::SecondClassGroup)
            || in_category(ctx, player, CategoryType::FirstClassGroup)
        {
            if level_of(ctx, player) < 40 {
                Some("test_server_helper023.html")
            } else {
                match class_of(ctx, player) {
                    FIGHTER => Some("test_server_helper012.html"),
                    WARRIOR => Some("test_server_helper012a.html"),
                    KNIGHT => Some("test_server_helper012b.html"),
                    ROGUE => Some("test_server_helper012c.html"),
                    MAGE => Some("test_server_helper013.html"),
                    WIZARD => Some("test_server_helper013a.html"),
                    CLERIC => Some("test_server_helper013b.html"),
                    ELVEN_FIGHTER => Some("test_server_helper014.html"),
                    ELVEN_KNIGHT => Some("test_server_helper014a.html"),
                    ELVEN_SCOUT => Some("test_server_helper014b.html"),
                    ELVEN_MAGE => Some("test_server_helper015.html"),
                    ELVEN_WIZARD => Some("test_server_helper015a.html"),
                    ORACLE => Some("test_server_helper015b.html"),
                    DARK_FIGHTER => Some("test_server_helper016.html"),
                    PALUS_KNIGHT => Some("test_server_helper016a.html"),
                    ASSASSIN => Some("test_server_helper016b.html"),
                    DARK_MAGE => Some("test_server_helper017.html"),
                    DARK_WIZARD => Some("test_server_helper017a.html"),
                    SHILLIEN_ORACLE => Some("test_server_helper017b.html"),
                    ORC_FIGHTER => Some("test_server_helper018.html"),
                    ORC_RAIDER => Some("test_server_helper018a.html"),
                    ORC_MONK => Some("test_server_helper018b.html"),
                    ORC_MAGE | ORC_SHAMAN => Some("test_server_helper019.html"),
                    DWARVEN_FIGHTER => Some("test_server_helper020.html"),
                    ARTISAN => Some("test_server_helper020b.html"),
                    SCAVENGER => Some("test_server_helper020a.html"),
                    MALE_SOLDIER | TROOPER => Some("test_server_helper020c.html"),
                    FEMALE_SOLDIER | WARDER => Some("test_server_helper020d.html"),
                    _ => None,
                }
            }
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("test_server_helper010.html")
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            Some("test_server_helper011.html")
        } else {
            Some("test_server_helper029.html")
        }
    }

    /// The `for (ClassChangeData ...) if (isInCategory(player)) found = true;` loops.
    fn any_option_applies(&self, ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
        self.config
            .class_change_data
            .iter()
            .any(|d| option_in_category(d, ctx, player))
    }

    /// Java `checkIfClassChangeHasOptions(player)`: an option with a price applies, or more than
    /// one option with a reward.
    fn check_if_class_change_has_options(&self, ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
        let data = &self.config.class_change_data;
        // Check if there are requirements.
        let show_options = data
            .iter()
            .any(|ccd| !ccd.items_required.is_empty() && option_in_category(ccd, ctx, player));
        if show_options {
            return true;
        }
        // Check if there is more than 1 reward to chose.
        let count = data
            .iter()
            .filter(|ccd| !ccd.items_rewarded.is_empty() && option_in_category(ccd, ctx, player))
            .count();
        count > 1
    }

    /// Java `getClassChangeData(index)`.
    fn get_class_change_data(&self, index: i32) -> Option<&ClassChangeData> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.config.class_change_data.get(i))
    }

    /// Java `getClassChangeOptions(player, selectedClassId)`: the rows of `%options%`.
    fn get_class_change_options(
        &self,
        ctx: &ScriptCtx<'_>,
        player: ObjectId,
        selected_class_id: i32,
    ) -> String {
        let item_name = |id: i32| ctx.world().item_template_name(id).unwrap_or_default();
        let mut sb = String::new();
        for (i, option) in self.config.class_change_data.iter().enumerate() {
            if !option
                .applied_categories
                .iter()
                .any(|&c| in_category(ctx, player, c))
            {
                continue;
            }
            sb.push_str("<tr><td><img src=L2UI_CT1.ChatBalloon_DF_TopCenter width=276 height=1 /></td></tr>");
            sb.push_str("<tr><td><table bgcolor=3f3f3f width=100%>");
            sb.push_str(&format!(
                "<tr><td align=center><a action=\"bypass -h Script ClassMaster setclass {selected_class_id} {i}\">{}:</a></td></tr>",
                option.name
            ));
            sb.push_str("<tr><td><table width=276>");
            sb.push_str("<tr><td>Requirements:</td></tr>");
            if option.items_required.is_empty() {
                sb.push_str("<tr><td><font color=LEVEL>Free</font></td></tr>");
            } else {
                for ih in &option.items_required {
                    sb.push_str(&format!(
                        "<tr><td><font color=\"LEVEL\">{}</font></td><td>{}</td><td width=30></td></tr>",
                        ih.count,
                        item_name(ih.id)
                    ));
                }
            }
            sb.push_str("<tr><td>Rewards:</td></tr>");
            if option.items_rewarded.is_empty() {
                if option.reward_noblesse {
                    sb.push_str("<tr><td><font color=\"LEVEL\">Noblesse status.</font></td></tr>");
                }
                if option.reward_hero {
                    sb.push_str("<tr><td><font color=\"LEVEL\">Hero status.</font></td></tr>");
                }
                if !option.reward_noblesse && !option.reward_hero {
                    sb.push_str("<tr><td><font color=LEVEL>none</font></td></tr>");
                }
            } else {
                for ih in &option.items_rewarded {
                    sb.push_str(&format!(
                        "<tr><td><font color=\"LEVEL\">{}</font></td><td>{}</td><td width=30></td></tr>",
                        ih.count,
                        item_name(ih.id)
                    ));
                }
                if option.reward_noblesse {
                    sb.push_str("<tr><td><font color=\"LEVEL\">Noblesse status.</font></td></tr>");
                }
                if option.reward_hero {
                    sb.push_str("<tr><td><font color=\"LEVEL\">Hero status.</font></td></tr>");
                }
            }
            sb.push_str("</table></td></tr>");
            sb.push_str("</table></td></tr>");
            sb.push_str("<tr><td><img src=L2UI_CT1.ChatBalloon_DF_TopCenter width=276 height=1 /></td></tr>");
        }
        sb
    }

    /// Java `cc_options.html` with `%name%` and `%options%`.
    fn options_page(&self, ctx: &ScriptCtx<'_>, player: ObjectId, class_id: i32) -> String {
        ctx.get_htm(player, "cc_options.html")
            .unwrap_or_default()
            .replace("%name%", &ctx.world().class_name(class_id))
            .replace("%options%", &self.get_class_change_options(ctx, player, class_id))
    }

    /// The payment and rewards of an option. `false` when the player cannot pay ("You do not
    /// have enough items.", no class change).
    fn pay_and_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, data: &ClassChangeData) -> bool {
        // Required items.
        if !data.items_required.is_empty() {
            for ri in &data.items_required {
                if ctx.get_quest_items_count(player, ri.id) < ri.count {
                    ctx.world().send_message(player, "You do not have enough items.");
                    return false; // No class change if payment failed.
                }
            }
            for ri in &data.items_required {
                ctx.world_mut()
                    .destroy_item_by_item_id(player, ri.id, ri.count, true);
            }
        }
        // Give possible rewards.
        for ri in &data.items_rewarded {
            ctx.give_items(player, ri.id, ri.count);
        }
        // Give possible nobless status reward.
        if data.reward_noblesse {
            ctx.world_mut().set_noble(player, true);
        }
        // Give possible hero status reward.
        if data.reward_hero {
            ctx.world_mut().set_hero(player, true);
        }
        true
    }

    /// The class change after the payment: the class, the base class or the subclass holder,
    /// `AutoLearnSkills`, the store, the user info and the skill list.
    fn apply_class(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) {
        ctx.world_mut().set_player_class(player, class_id);
        let active = class_of(ctx, player);
        if ctx.world().is_subclass_active(player) {
            ctx.world_mut().set_active_sub_class_holder_class(player, active);
        } else {
            ctx.world_mut().set_base_class(player, active);
        }
        let (auto_learn, by_fs, without_items) = ctx.world().auto_learn_settings();
        if auto_learn {
            ctx.world_mut()
                .give_available_skills_now(player, by_fs, true, without_items);
        }
        // Save player cause if server crashes before this char is saved, he will lose class and
        // the money payed for class change.
        ctx.world_mut().store_player(player);
        ctx.world_mut().broadcast_user_info(player);
        ctx.world_mut().send_skill_list(player);
    }

    /// Java `changeToNextClass(player)`.
    fn change_to_next_class(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
        let current = class_of(ctx, player);
        // `Arrays.stream(PlayerClass.values()).filter(cid -> player.getPlayerClass() ==
        // cid.getParent()).findAny()`: the first child in declaration (id) order.
        let new_class = (0..=136u16)
            .map(ClassId)
            .filter(|c| c.is_player_class())
            .find(|c| c.enum_parent().map(|p| i32::from(p.0)) == Some(current));
        let Some(new_class) = new_class.map(|c| i32::from(c.0)) else {
            warn!("ClassMaster: No new classId found for {player}");
            return false;
        };
        if self.check_if_class_change_has_options(ctx, player) {
            let html = self.options_page(ctx, player, new_class);
            ctx.show_result(player, Some(&html), None);
            return false;
        }
        let data = self
            .config
            .class_change_data
            .iter()
            .find(|ccd| option_in_category(ccd, ctx, player))
            .cloned();
        if let Some(data) = data {
            if !Self::pay_and_reward(ctx, player, &data) {
                return false;
            }
        }
        Self::apply_class(ctx, player, new_class);
        true
    }

    /// Java `showPopupWindow(player)`: the question mark 1001 when a class change is due.
    fn show_popup_window(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if !self.config.show_popup_window {
            return;
        }
        if !self.any_option_applies(ctx, player) {
            return;
        }
        let level = level_of(ctx, player);
        let first = in_category(ctx, player, CategoryType::FirstClassGroup);
        let second = in_category(ctx, player, CategoryType::SecondClassGroup);
        let third = in_category(ctx, player, CategoryType::ThirdClassGroup);
        if (first && level >= 20) || ((second || first) && level >= 40) || (third && level >= 76) {
            ctx.world_mut().send_tutorial_question_mark(player, 1001);
        }
    }

    /// Java `onPlayerPressTutorialMark(event)` (`ON_PLAYER_PRESS_TUTORIAL_MARK`, not fired by the
    /// engine yet, see the module notes).
    pub fn on_player_press_tutorial_mark(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, mark_id: i32) {
        if !self.config.show_popup_window || mark_id != 1001 {
            return;
        }
        if !self.any_option_applies(ctx, player) {
            return;
        }
        let level = level_of(ctx, player);
        let mut html = None;
        if in_category(ctx, player, CategoryType::FirstClassGroup) && level >= 20 {
            html = Self::get_first_occupation_change_html(ctx, player).and_then(|f| ctx.get_htm(player, f));
        } else if in_category(ctx, player, CategoryType::SecondClassGroup) && level >= 40 {
            html = Self::get_second_occupation_change_html(ctx, player).and_then(|f| ctx.get_htm(player, f));
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) && level >= 76 {
            html = ctx.get_htm(player, "qm_thirdclass.html");
        }
        if let Some(html) = html {
            ctx.show_result(player, Some(&html), None);
        }
    }

    /// The `setclass <classId> [option]` event.
    fn set_class(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        args: &mut std::str::SplitWhitespace<'_>,
    ) -> Option<String> {
        if !self.any_option_applies(ctx, player) {
            return None;
        }
        // NOTE(java-quirk): a class id or option index that is no number throws in Java; the
        // error page goes to a GM only.
        let class_id: i32 = args.next()?.parse().ok()?;
        let level = level_of(ctx, player);
        let first = in_category(ctx, player, CategoryType::FirstClassGroup);
        let second = in_category(ctx, player, CategoryType::SecondClassGroup);
        let mut can_change = false;
        if (second || first) && level >= 40 {
            // In retail you can skip first occupation.
            can_change = ctx
                .world()
                .id_in_category(CategoryType::ThirdClassGroup, class_id)
                || (first
                    && ctx
                        .world()
                        .id_in_category(CategoryType::SecondClassGroup, class_id));
        } else if first && level >= 20 {
            can_change = ctx
                .world()
                .id_in_category(CategoryType::SecondClassGroup, class_id);
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) && level >= 76 {
            can_change = ctx
                .world()
                .id_in_category(CategoryType::FourthClassGroup, class_id);
        }

        // Validate that the requested class belongs to the player's own class tree, preventing
        // bypass exploits.
        let target = u16::try_from(class_id)
            .ok()
            .map(ClassId)
            .filter(|c| c.is_player_class());
        let current = u16::try_from(class_of(ctx, player)).ok().map(ClassId);
        let in_tree = target.zip(current).is_some_and(|(t, c)| t.equals_or_child_of(c));
        if !(can_change && in_tree) {
            return None;
        }
        let class_data_index = match args.next() {
            Some(token) => token.parse().ok()?,
            None => -1,
        };
        if self.check_if_class_change_has_options(ctx, player) && class_data_index == -1 {
            return Some(self.options_page(ctx, player, class_id));
        }
        if let Some(data) = self.get_class_change_data(class_data_index).cloned() {
            if !Self::pay_and_reward(ctx, player, &data) {
                return None;
            }
        }
        Self::apply_class(ctx, player, class_id);
        ctx.play_sound_named(player, "ItemSound.quest_fanfare_2");
        Some("test_server_helper021.html".to_owned())
    }
}

impl Script for ClassMaster {
    fn name(&self) -> &'static str {
        "ClassMaster"
    }

    fn path(&self) -> &'static str {
        "ai/others/ClassMaster"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&CLASS_MASTERS);
        s.add_talk_id(&CLASS_MASTERS);
        s.add_first_talk_id(&CLASS_MASTERS);
        s.set_on_enter_world();
    }

    /// Java `load()` after the file: the class masters spawn when enabled.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        if !self.config.is_enabled {
            return;
        }
        if self.config.spawn_class_masters {
            for (npc_id, x, y, z, heading) in SPAWNS {
                ctx.add_spawn(npc_id, x, y, z, heading, false, 0, 0);
            }
        }
        tracing::info!(
            "ClassMaster: Loaded {} class change options.",
            self.config.class_change_data.len()
        );
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("test_server_helper001.html".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_value: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if !self.config.is_enabled {
            return None;
        }
        let player = player?;
        let mut st = event_value.split_whitespace();
        // NOTE(java-quirk): `StringTokenizer.nextToken()` of an empty event throws.
        let event = st.next()?;
        let mut htmltext = None;
        match event {
            "buyitems" => {
                // NOTE(java-quirk): Java's `npc.getId()` throws without an NPC.
                let npc = npc?;
                htmltext = Some(if npc.id == CLASS_MASTERS[0] {
                    "test_server_helper001a.html"
                } else {
                    "test_server_helper001b.html"
                });
            }
            "setnoble" => {
                if ctx.world().is_noble(player) {
                    htmltext = Some("test_server_helper025b.html");
                } else if level_of(ctx, player) < 75 {
                    htmltext = Some("test_server_helper025a.html");
                } else {
                    ctx.world_mut().set_noble(player, true);
                    ctx.world_mut().broadcast_user_info(player);
                    // TODO: SetOneTimeQuestFlag(talker, 10385, 1);
                    htmltext = Some("test_server_helper025.html");
                }
            }
            "firstclass" => {
                htmltext = Self::get_first_occupation_change_html(ctx, player);
            }
            "secondclass" => {
                htmltext = Self::get_second_occupation_change_html(ctx, player);
            }
            "thirdclass" => {
                if in_category(ctx, player, CategoryType::ThirdClassGroup) && level_of(ctx, player) > 75 {
                    if self.change_to_next_class(ctx, player) {
                        ctx.play_sound_named(player, "ItemSound.quest_fanfare_2");
                        ctx.world_mut().broadcast_user_info(player);
                        htmltext = Some("test_server_helper021.html");
                    }
                } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
                    htmltext = Some("test_server_helper011.html");
                } else {
                    htmltext = Some("test_server_helper024.html");
                }
            }
            "setclass" => return self.set_class(ctx, player, &mut st),
            "clanlevel" => {
                htmltext = Some(if ctx.world().is_clan_leader(player) {
                    "test_server_helper022.html"
                } else {
                    "pl014.html"
                });
            }
            "learnskills" => {
                ctx.world_mut()
                    .give_available_skills_now(player, true, true, true);
            }
            "clanlevelup" => {
                let clan = ctx.world().clan_of(player).map(|c| (c.id, c.level));
                match clan {
                    Some((clan_id, level)) if ctx.world().is_clan_leader(player) => {
                        if level >= 10 {
                            htmltext = Some("test_server_helper022a.html");
                        } else {
                            ctx.world_mut().change_clan_level(clan_id, level + 1);
                            ctx.world_mut().broadcast_clan_status(clan_id);
                            htmltext = Some("test_server_helper022c.html");
                        }
                    }
                    _ => htmltext = Some("pl014.html"),
                }
            }
            "test_server_helper001.html" => {
                // NOTE(java-quirk): Java's `npc.getId()` throws without an NPC.
                if CLASS_MASTERS.contains(&npc?.id) {
                    htmltext = Some("test_server_helper001.html");
                }
            }
            _ => {}
        }
        htmltext.map(str::to_owned)
    }

    /// Java `onPlayerLogin` (`ON_PLAYER_LOGIN`).
    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        self.show_popup_window(ctx, player);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use l2_world::subclass::categories::ItemHolder;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to, player_data};

    const DIR: &str = "data/scripts/ai/others/ClassMaster";
    const NPCS: &str = r#"<npc id="31756" type="Folk" level="70" name="Mr. Cat"/>
        <npc id="31757" type="Folk" level="70" name="Miss Queen"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6622" type="EtcItem" name="Giant's Codex"><set name="is_stackable" val="true"/></item>"#;

    /// The shipped options, enabled: free for the first class, a million adena for the second.
    fn config() -> ClassMasterConfig {
        ClassMasterConfig {
            is_enabled: true,
            spawn_class_masters: false,
            show_popup_window: false,
            class_change_data: vec![
                ClassChangeData {
                    name: "Free".to_owned(),
                    applied_categories: vec![CategoryType::FirstClassGroup],
                    reward_noblesse: false,
                    reward_hero: false,
                    items_required: vec![],
                    items_rewarded: vec![],
                },
                ClassChangeData {
                    name: "1,000,000 Adena".to_owned(),
                    applied_categories: vec![CategoryType::SecondClassGroup],
                    reward_noblesse: false,
                    reward_hero: false,
                    items_required: vec![ItemHolder {
                        id: 57,
                        count: 1_000_000,
                    }],
                    items_rewarded: vec![],
                },
            ],
        }
    }

    fn setup(config: ClassMasterConfig) -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ClassMaster::new(config));
        let link = |e: &str| format!(r#"<a action="bypass -h Script ClassMaster {e}">{e}</a>"#);
        let mut pages: Vec<(String, String)> = vec![(
            format!("{DIR}/test_server_helper001.html"),
            format!(
                "<html><body>helper001 {}{}{}{}{}</body></html>",
                link("firstclass"),
                link("secondclass"),
                link("thirdclass"),
                link("setnoble"),
                link("buyitems")
            ),
        )];
        pages.push((
            format!("{DIR}/test_server_helper026a.html"),
            format!(
                "<html><body>helper026a {}{}</body></html>",
                link("setclass 1"),
                link("setclass 2")
            ),
        ));
        pages.push((
            format!("{DIR}/test_server_helper012a.html"),
            format!("<html><body>helper012a {}</body></html>", link("setclass 2")),
        ));
        pages.push((
            format!("{DIR}/cc_options.html"),
            "<html><body>cc_options %name% <table>%options%</table></body></html>".to_owned(),
        ));
        for page in [
            "test_server_helper021.html",
            "test_server_helper024.html",
            "test_server_helper025.html",
            "test_server_helper025a.html",
            "test_server_helper001a.html",
        ] {
            pages.push((
                format!("{DIR}/{page}"),
                format!("<html><body>{page}</body></html>"),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let cat = t.spawn(31756, 20);
        (t, cat)
    }

    fn enter(t: &mut ScriptTest, id: i32, level: i32, class_id: i32, items: Vec<l2_world::ItemData>) {
        let mut data = player_data(id, level, items);
        data.class_id = class_id;
        data.base_class = class_id;
        t.enter_with(data);
        t.take();
    }

    fn page(t: &mut ScriptTest, player: i32, event: &str) -> String {
        t.bypass(player, &format!("Script ClassMaster {event}"));
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn a_free_first_class_change_at_mr_cat() {
        let (mut t, cat) = setup(config());
        enter(&mut t, 1, 20, 0, vec![]);
        t.talk(1, cat);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("helper001"));
        assert!(page(&mut t, 1, "firstclass").contains("helper026a"));
        // The free option has no price and no reward: no options page.
        t.bypass(1, "Script ClassMaster setclass 1");
        let out = t.take();
        assert!(
            pages_to(&out, 1)
                .pop()
                .unwrap()
                .contains("test_server_helper021.html")
        );
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        let p = t.world.player(Oid(1)).unwrap();
        assert_eq!((p.data.class_id, p.data.base_class), (1, 1));
    }

    #[test]
    fn the_second_class_costs_a_million_adena() {
        let (mut t, cat) = setup(config());
        enter(&mut t, 1, 40, 1, vec![item(100, 57, 999_999)]);
        t.talk(1, cat);
        t.take();
        assert!(page(&mut t, 1, "secondclass").contains("helper012a"));
        // A price: the options page first, with the option's link.
        let options = page(&mut t, 1, "setclass 2");
        // (`%name%` is the class list's name; the test world has no class list.)
        assert!(options.contains("cc_options"));
        assert!(options.contains("bypass -h Script ClassMaster setclass 2 1"));
        assert!(options.contains("1000000"));
        // Not enough adena: nothing changes.
        t.bypass(1, "Script ClassMaster setclass 2 1");
        assert!(pages_to(&t.take(), 1).is_empty());
        assert_eq!(t.world.player(Oid(1)).unwrap().data.class_id, 1);

        enter(&mut t, 2, 40, 1, vec![item(101, 57, 1_500_000)]);
        t.talk(2, cat);
        t.take();
        page(&mut t, 2, "secondclass");
        page(&mut t, 2, "setclass 2");
        assert!(page(&mut t, 2, "setclass 2 1").contains("test_server_helper021.html"));
        assert_eq!(t.world.player(Oid(2)).unwrap().data.class_id, 2);
        assert_eq!(t.count(2, 57), 500_000);
    }

    #[test]
    fn a_class_outside_the_tree_and_the_disabled_master() {
        let (mut t, cat) = setup(config());
        enter(&mut t, 1, 20, 0, vec![]);
        t.talk(1, cat);
        t.take();
        page(&mut t, 1, "firstclass");
        // A mystic class is not a child of the fighter.
        t.bypass(1, "Script ClassMaster setclass 11");
        assert!(pages_to(&t.take(), 1).is_empty());
        assert_eq!(t.world.player(Oid(1)).unwrap().data.class_id, 0);

        let (mut t, cat) = setup(ClassMasterConfig::default());
        enter(&mut t, 1, 20, 0, vec![]);
        t.talk(1, cat);
        t.take();
        // Disabled: every event answers nothing.
        assert!(page(&mut t, 1, "firstclass").is_empty());
    }

    #[test]
    fn noblesse_and_the_third_class_pages() {
        let (mut t, cat) = setup(config());
        enter(&mut t, 1, 74, 2, vec![]);
        t.talk(1, cat);
        t.take();
        assert!(page(&mut t, 1, "setnoble").contains("test_server_helper025a.html"));
        enter(&mut t, 2, 76, 2, vec![]);
        t.talk(2, cat);
        t.take();
        assert!(page(&mut t, 2, "setnoble").contains("test_server_helper025.html"));
        assert!(t.world.is_noble(Oid(2)));
        t.talk(2, cat);
        t.take();
        // Level 76 Gladiator: Duelist (no option applies to the third class group here).
        assert!(page(&mut t, 2, "thirdclass").contains("test_server_helper021.html"));
        assert_eq!(t.world.player(Oid(2)).unwrap().data.class_id, 88);
        t.talk(1, cat);
        t.take();
        assert!(page(&mut t, 1, "thirdclass").contains("test_server_helper024.html"));
        t.talk(1, cat);
        t.take();
        assert!(page(&mut t, 1, "buyitems").contains("test_server_helper001a.html"));
    }
}
