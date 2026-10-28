//! The 2nd class transfer of dark elves (Brecson, Medown, Angus, Andromeda, Oltran, Xairakin,
//! Samael, Tifaren, Drizzit, Helminter).
// Java: dist/game/data/scripts/village_master/DarkElfChange2/DarkElfChange2.java

use l2_util::ObjectId;
use l2_world::subclass::{ClassId, Race};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, class_of, is_numeric};

// NPCs
const NPCS: [i32; 10] = [
    30195, // Brecson
    30699, // Medown
    30474, // Angus
    31324, // Andromeda
    30862, // Oltran
    30910, // Xairakin
    31285, // Samael
    31334, // Tifaren
    31974, // Drizzit
    32096, // Helminter
];

// Items
const MARK_OF_CHALLENGER: i32 = 2627;
const MARK_OF_DUTY: i32 = 2633;
const MARK_OF_SEEKER: i32 = 2673;
const MARK_OF_SCHOLAR: i32 = 2674;
const MARK_OF_PILGRIM: i32 = 2721;
const MARK_OF_DUELIST: i32 = 2762;
const MARK_OF_SEARCHER: i32 = 2809;
const MARK_OF_REFORMER: i32 = 2821;
const MARK_OF_MAGUS: i32 = 2840;
const MARK_OF_FATE: i32 = 3172;
const MARK_OF_SAGITTARIUS: i32 = 3293;
const MARK_OF_WITCHCRAFT: i32 = 3307;
const MARK_OF_SUMMONER: i32 = 3336;

/// `{ new class, required class, page < 40 without the marks, < 40 with them, >= 40 without them,
/// >= 40 with them, the three marks }`.
const CLASSES: [[i32; 9]; 7] = [
    [
        33,
        32,
        26,
        27,
        28,
        29,
        MARK_OF_DUTY,
        MARK_OF_FATE,
        MARK_OF_WITCHCRAFT,
    ], // SK
    [
        34,
        32,
        30,
        31,
        32,
        33,
        MARK_OF_CHALLENGER,
        MARK_OF_FATE,
        MARK_OF_DUELIST,
    ], // BD
    [
        43,
        42,
        34,
        35,
        36,
        37,
        MARK_OF_PILGRIM,
        MARK_OF_FATE,
        MARK_OF_REFORMER,
    ], // SE
    [
        36,
        35,
        38,
        39,
        40,
        41,
        MARK_OF_SEEKER,
        MARK_OF_FATE,
        MARK_OF_SEARCHER,
    ], // AW
    [
        37,
        35,
        42,
        43,
        44,
        45,
        MARK_OF_SEEKER,
        MARK_OF_FATE,
        MARK_OF_SAGITTARIUS,
    ], // PR
    [
        40,
        39,
        46,
        47,
        48,
        49,
        MARK_OF_SCHOLAR,
        MARK_OF_FATE,
        MARK_OF_MAGUS,
    ], // SH
    [
        41,
        39,
        50,
        51,
        52,
        53,
        MARK_OF_SCHOLAR,
        MARK_OF_FATE,
        MARK_OF_SUMMONER,
    ], // PS
];

/// `PlayerClass.PALUS_KNIGHT`, `SHILLIEN_ORACLE`, `ASSASSIN`, `DARK_WIZARD`.
const PALUS_KNIGHT: i32 = 32;
const SHILLIEN_ORACLE: i32 = 42;
const ASSASSIN: i32 = 35;
const DARK_WIZARD: i32 = 39;

/// Java `village_master.DarkElfChange2.DarkElfChange2`.
pub struct DarkElfChange2;

impl Script for DarkElfChange2 {
    fn name(&self) -> &'static str {
        "DarkElfChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/DarkElfChange2"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if is_numeric(event) {
            // NOTE(java-quirk): an index past `CLASSES` (or past `int`) throws in Java; the script
            // error page goes to a GM only, so a player sees nothing.
            let class = CLASSES.get(event.parse::<usize>().ok()?)?;
            let player = player?;
            let cid = class_of(ctx, player);
            let race = u16::try_from(cid).ok().and_then(|c| ClassId(c).race());
            if race == Some(Race::DarkElf) && cid == class[1] {
                let suffix;
                let item1 = ctx.has_quest_items(player, class[6]);
                let item2 = ctx.has_quest_items(player, class[7]);
                let item3 = ctx.has_quest_items(player, class[8]);
                if ctx.player_level(player) < 40 {
                    suffix = if !item1 || !item2 || !item3 {
                        class[2]
                    } else {
                        class[3]
                    };
                } else {
                    if !item1 || !item2 || !item3 {
                        suffix = class[4];
                    } else {
                        suffix = class[5];
                        ctx.take_items(player, class[6], -1);
                        ctx.take_items(player, class[7], -1);
                        ctx.take_items(player, class[8], -1);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestFanfare2);
                        ctx.world_mut().set_player_class(player, class[0]);
                        ctx.world_mut().set_base_class(player, class[0]);
                        ctx.give_items(player, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, 15);
                        ctx.world_mut().broadcast_user_info(player);
                    }
                }
                return Some(format!("30474-{suffix}.html"));
            }
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        if ctx.world().is_subclass_active(player) {
            return Some(htmltext);
        }
        let cid = class_of(ctx, player);
        let class = u16::try_from(cid).ok().map(ClassId);
        if class.and_then(ClassId::race) == Some(Race::DarkElf) {
            htmltext = match cid {
                PALUS_KNIGHT => "30474-01.html",
                SHILLIEN_ORACLE => "30474-08.html",
                ASSASSIN => "30474-12.html",
                DARK_WIZARD => "30474-19.html",
                _ => {
                    let level = class.map_or(0, ClassId::class_level);
                    if level == 0 {
                        // first occupation not made yet
                        "30474-55.html"
                    } else if level >= 2 {
                        // second/third occupation change already made
                        "30474-54.html"
                    } else {
                        "30474-56.html"
                    }
                }
            }
            .to_owned();
        } else {
            htmltext = "30474-56.html".to_owned(); // other races
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/DarkElfChange2";

    fn setup() -> Vm {
        let mut vm = Vm::new(
            DarkElfChange2,
            DIR,
            "DarkElfChange2",
            &[(31324, "VillageMasterDElf")],
        );
        vm.page("30474-19.html", &["30474-20.html", "5", "6"]);
        for n in [20, 46, 47, 48, 49, 54, 55, 56] {
            vm.page(&format!("30474-{n:02}.html"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_dark_wizard_becomes_a_spellhowler() {
        let mut vm = setup();
        let marks = |base: i32| {
            vec![
                item(base, MARK_OF_SCHOLAR, 1),
                item(base + 1, MARK_OF_FATE, 1),
                item(base + 2, MARK_OF_MAGUS, 1),
            ]
        };
        vm.enter(1, 39, DARK_WIZARD, marks(100));
        assert!(vm.talk(1, 31324).contains("30474-19.html"));
        assert!(vm.click(1, "30474-20.html").contains("30474-20.html"));
        assert!(vm.talk(1, 31324).contains("30474-19.html"));
        assert!(vm.click(1, "5").contains("30474-47.html"));
        vm.enter(2, 40, DARK_WIZARD, vec![item(110, MARK_OF_SCHOLAR, 1)]);
        assert!(vm.talk(2, 31324).contains("30474-19.html"));
        assert!(vm.click(2, "5").contains("30474-48.html"));

        vm.enter(3, 40, DARK_WIZARD, marks(120));
        assert!(vm.talk(3, 31324).contains("30474-19.html"));
        assert!(vm.click(3, "5").contains("30474-49.html"));
        vm.assert_class(3, 40);
        for mark in [MARK_OF_SCHOLAR, MARK_OF_FATE, MARK_OF_MAGUS] {
            assert_eq!(vm.t.count(3, mark), 0);
        }
        assert_eq!(vm.t.count(3, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);
        assert!(vm.talk(3, 31324).contains("30474-54.html"));
        // A dark fighter has no 1st class yet; a human is another race.
        vm.enter(4, 40, 31, vec![]);
        assert!(vm.talk(4, 31324).contains("30474-55.html"));
        vm.enter(5, 40, 1, vec![]);
        assert!(vm.talk(5, 31324).contains("30474-56.html"));
    }
}
