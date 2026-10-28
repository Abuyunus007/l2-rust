//! The 1st class transfer of dark elves (Xenos, Tobias, Tronix, Devon).
// Java: dist/game/data/scripts/village_master/DarkElfChange1/DarkElfChange1.java

use l2_util::ObjectId;
use l2_world::subclass::{ClassId, Race};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, class_of, is_numeric};

// NPCs
const NPCS: [i32; 4] = [
    30290, // Xenos
    30297, // Tobias
    30462, // Tronix
    32160, // Devon
];

// Items
const GAZE_OF_ABYSS: i32 = 1244;
const IRON_HEART: i32 = 1252;
const JEWEL_OF_DARKNESS: i32 = 1261;
const ORB_OF_ABYSS: i32 = 1270;

/// `{ new class, required class, page < 20 without the item, < 20 with it, >= 20 without it,
/// >= 20 with it, the item }`.
const CLASSES: [[i32; 7]; 4] = [
    [32, 31, 15, 16, 17, 18, GAZE_OF_ABYSS],     // PK
    [35, 31, 19, 20, 21, 22, IRON_HEART],        // AS
    [39, 38, 23, 24, 25, 26, JEWEL_OF_DARKNESS], // DW
    [42, 38, 27, 28, 29, 30, ORB_OF_ABYSS],      // SO
];

/// `PlayerClass.DARK_FIGHTER`, `DARK_MAGE`.
const DARK_FIGHTER: i32 = 31;
const DARK_MAGE: i32 = 38;

/// Java `village_master.DarkElfChange1.DarkElfChange1`.
pub struct DarkElfChange1;

impl Script for DarkElfChange1 {
    fn name(&self) -> &'static str {
        "DarkElfChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/DarkElfChange1"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
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
                let item = ctx.has_quest_items(player, class[6]);
                if ctx.player_level(player) < 20 {
                    suffix = if !item { class[2] } else { class[3] };
                } else {
                    if !item {
                        suffix = class[4];
                    } else {
                        suffix = class[5];
                        ctx.give_items(player, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, 15);
                        ctx.take_items(player, class[6], -1);
                        ctx.world_mut().set_player_class(player, class[0]);
                        ctx.world_mut().set_base_class(player, class[0]);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestFanfare2);
                        ctx.world_mut().broadcast_user_info(player);
                    }
                }
                return Some(format!("{}-{suffix}.html", npc?.id));
            }
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        if ctx.world().is_subclass_active(player) {
            return Some(htmltext);
        }
        let cid = class_of(ctx, player);
        let class = u16::try_from(cid).ok().map(ClassId);
        if class.and_then(ClassId::race) == Some(Race::DarkElf) {
            match cid {
                DARK_FIGHTER => htmltext = format!("{}-01.html", npc.id),
                DARK_MAGE => htmltext = format!("{}-08.html", npc.id),
                _ => {
                    let level = class.map_or(0, ClassId::class_level);
                    if level == 1 {
                        // first occupation change already made
                        return Some(format!("{}-32.html", npc.id));
                    } else if level >= 2 {
                        // second/third occupation change already made
                        return Some(format!("{}-31.html", npc.id));
                    }
                }
            }
        } else {
            htmltext = format!("{}-33.html", npc.id); // other races
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::names_to;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/DarkElfChange1";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            DarkElfChange1,
            DIR,
            "DarkElfChange1",
            &[(30290, "VillageMasterDElf")],
        );
        vm.page("30290-01.html", &["30290-02.html", "0", "1"]);
        vm.page("30290-08.html", &["2", "3"]);
        for n in [2, 15, 16, 17, 18, 21, 28, 29, 31, 32, 33] {
            vm.page(&format!("30290-{n:02}.html"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_dark_fighter_becomes_a_palus_knight() {
        let mut vm = setup();
        vm.enter(1, 19, DARK_FIGHTER, vec![item(100, GAZE_OF_ABYSS, 1)]);
        assert!(vm.talk(1, 30290).contains("30290-01.html"));
        // Any other event is shown as a page.
        assert!(vm.click(1, "30290-02.html").contains("30290-02.html"));
        assert!(vm.talk(1, 30290).contains("30290-01.html"));
        assert!(vm.click(1, "0").contains("30290-16.html"));

        vm.enter(2, 20, DARK_FIGHTER, vec![item(101, GAZE_OF_ABYSS, 1)]);
        assert!(vm.talk(2, 30290).contains("30290-01.html"));
        vm.t.bypass(2, "Script DarkElfChange1 0");
        let out = vm.t.take();
        let names = names_to(&out, 2);
        // The class change effect, the fanfare and the new user info.
        assert!(names.contains(&"MAGIC_SKILL_USE".to_owned()));
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        assert!(names.contains(&"USER_INFO".to_owned()));
        assert!(
            crate::testing::pages_to(&out, 2)
                .pop()
                .unwrap()
                .contains("30290-18.html")
        );
        vm.assert_class(2, 32);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);
        assert_eq!(vm.t.count(2, GAZE_OF_ABYSS), 0);
        // Done: the first occupation change is made.
        assert!(vm.talk(2, 30290).contains("30290-32.html"));
    }

    #[test]
    fn a_dark_mage_without_the_orb_and_other_races() {
        let mut vm = setup();
        vm.enter(1, 20, DARK_MAGE, vec![]);
        assert!(vm.talk(1, 30290).contains("30290-08.html"));
        assert!(vm.click(1, "3").contains("30290-29.html"));
        vm.enter(2, 20, 0, vec![]);
        assert!(vm.talk(2, 30290).contains("30290-33.html"));
        // A dark elf of the 2nd class.
        vm.enter(3, 40, 33, vec![]);
        assert!(vm.talk(3, 30290).contains("30290-31.html"));
    }
}
