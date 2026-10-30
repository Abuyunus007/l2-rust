//! Quest 619: Relics of the Old Empire (the Ghost of an Adventurer in the Town of Aden; the undead
//! and the Imperial Tomb monsters drop broken relic parts, 1000 of them buy a random recipe).
// Java: dist/game/data/scripts/quests/Q00619_RelicsOfTheOldEmpire/Q00619_RelicsOfTheOldEmpire.java
//
// Pattern: a kill drop per monster (a chance, a double-item roll in per cent, and a 1/30 pass to
// the Sepulcher on some monsters). The killer gets the items (Java's `player` in `onKill` is the
// killer), the party member at cond 3 is the one that counts (`getRandomPartyMemberState`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GHOST_OF_ADVENTURER: i32 = 31538;

// Items
const ENTRANCE_PASS_TO_THE_SEPULCHER: i32 = 7075;
const BROKEN_RELIC_PART: i32 = 7254;

// Misc
const MIN_LEVEL: i32 = 74;
const REQUIRED_RELIC_COUNT: i64 = 1000;

/// Java `RECIPES`: the reward, one of them at random.
const RECIPES: [i32; 10] = [6881, 6883, 6885, 6887, 6891, 6893, 6895, 6897, 6899, 7580];

/// Java `ARCHON_OF_HALISHA`.
const ARCHON_OF_HALISHA: [i32; 8] = [18212, 18213, 18214, 18215, 18216, 18217, 18218, 18219];

/// Java `DropInfo`: the drop chance, the double-item chance (per cent, 0 for none) and whether the
/// Sepulcher pass can drop too.
#[derive(Clone, Copy)]
struct DropInfo {
    chance: f64,
    double_chance: i32,
    entrance_pass: bool,
}

const fn drop(chance: f64, double_chance: i32, entrance_pass: bool) -> DropInfo {
    DropInfo {
        chance,
        double_chance,
        entrance_pass,
    }
}

/// Java `MOBS`. Where Java puts a monster twice (18183) the last put is kept.
const MOBS: &[(i32, DropInfo)] = &[
    (21396, drop(0.51, 0, true)),   // carrion_scarab
    (21397, drop(0.50, 0, true)),   // carrion_scarab_a
    (21398, drop(0.95, 0, true)),   // soldier_scarab
    (21399, drop(0.84, 0, true)),   // soldier_scarab_a
    (21400, drop(0.76, 0, true)),   // hexa_beetle
    (21401, drop(0.67, 0, true)),   // hexa_beetle_a
    (21402, drop(0.69, 0, true)),   // katraxith
    (21403, drop(0.80, 0, true)),   // katraxith_a
    (21404, drop(0.90, 0, true)),   // tera_beetle
    (21405, drop(0.64, 0, true)),   // tera_beetle_a
    (21406, drop(0.87, 0, true)),   // imperial_knight
    (21407, drop(0.56, 0, true)),   // imperial_knight_a
    (21408, drop(0.82, 0, true)),   // imperial_guard
    (21409, drop(0.92, 0, true)),   // imperial_guard_a
    (21410, drop(0.81, 0, true)),   // guardian_scarab
    (21411, drop(0.66, 0, true)),   // guardian_scarab_a
    (21412, drop(1.00, 6, true)),   // ustralith
    (21413, drop(0.81, 0, true)),   // ustralith_a
    (21414, drop(0.79, 0, true)),   // imperial_assassin
    (21415, drop(0.80, 0, true)),   // imperial_assassin_a
    (21416, drop(0.82, 0, true)),   // imperial_warlord
    (21417, drop(1.00, 27, true)),  // imperial_warlord_a
    (21418, drop(0.66, 0, true)),   // imperial_highguard
    (21419, drop(0.67, 0, true)),   // imperial_highguard_a
    (21420, drop(0.82, 0, true)),   // ashuras
    (21421, drop(0.77, 0, true)),   // ashuras_a
    (21422, drop(0.88, 0, true)),   // imperial_dancer
    (21423, drop(0.94, 0, true)),   // imperial_dancer_a
    (21424, drop(1.00, 19, true)),  // ashikenas
    (21425, drop(1.00, 21, true)),  // ashikenas_a
    (21426, drop(1.00, 8, true)),   // abraxian
    (21427, drop(0.74, 0, true)),   // abraxian_a
    (21428, drop(0.76, 0, true)),   // hasturan
    (21429, drop(0.80, 0, true)),   // hasturan_a
    (21430, drop(1.00, 10, true)),  // ahrimanes
    (21431, drop(0.94, 0, true)),   // ahrimanes_a
    (21432, drop(1.00, 34, true)),  // chakram_beetle
    (21433, drop(1.00, 34, true)),  // jamadhr_beetle
    (21434, drop(1.00, 90, true)),  // priest_of_blood
    (21435, drop(1.00, 60, true)),  // sacrifice_guide
    (21436, drop(1.00, 66, true)),  // sacrifice_bearer
    (21437, drop(0.69, 0, true)),   // sacrifice_scarab
    (21798, drop(0.33, 0, true)),   // guard_skeleton_2d
    (21799, drop(0.61, 0, true)),   // guard_skeleton_3d
    (21800, drop(0.31, 0, true)),   // guard_undead
    (18120, drop(1.00, 28, false)), // r11_roomboss_strong
    (18121, drop(1.00, 21, false)), // r11_roomboss_weak
    (18122, drop(0.93, 0, false)),  // r11_roomboss_teleport
    (18123, drop(1.00, 28, false)), // r12_roomboss_strong
    (18124, drop(1.00, 21, false)), // r12_roomboss_weak
    (18125, drop(0.93, 0, false)),  // r12_roomboss_teleport
    (18126, drop(1.00, 28, false)), // r13_roomboss_strong
    (18127, drop(1.00, 21, false)), // r13_roomboss_weak
    (18128, drop(0.93, 0, false)),  // r13_roomboss_teleport
    (18129, drop(1.00, 28, false)), // r14_roomboss_strong
    (18130, drop(1.00, 21, false)), // r14_roomboss_weak
    (18131, drop(0.93, 0, false)),  // r14_roomboss_teleport
    (18132, drop(1.00, 30, false)), // r1_beatle_healer
    (18133, drop(1.00, 20, false)), // r1_scorpion_warrior
    (18134, drop(0.90, 0, false)),  // r1_warrior_longatk1_h
    (18135, drop(1.00, 20, false)), // r1_warrior_longatk2
    (18136, drop(1.00, 20, false)), // r1_warrior_selfbuff
    (18137, drop(0.89, 0, false)),  // r1_wizard_h
    (18138, drop(1.00, 19, false)), // r1_wizard_clanbuff
    (18139, drop(1.00, 17, false)), // r1_wizard_debuff
    (18140, drop(1.00, 19, false)), // r1_wizard_selfbuff
    (18141, drop(0.76, 0, false)),  // r21_scarab_roombosss
    (18142, drop(0.76, 0, false)),  // r22_scarab_roombosss
    (18143, drop(0.76, 0, false)),  // r23_scarab_roombosss
    (18144, drop(0.76, 0, false)),  // r24_scarab_roombosss
    (18145, drop(0.65, 0, false)),  // r2_wizard_clanbuff
    (18146, drop(0.66, 0, false)),  // r2_warrior_longatk2
    (18147, drop(0.62, 0, false)),  // r2_wizard
    (18148, drop(0.72, 0, false)),  // r2_warrior
    (18149, drop(0.63, 0, false)),  // r2_bomb
    (18166, drop(0.92, 0, false)),  // r3_warrior
    (18167, drop(0.92, 0, false)),  // r3_warrior_longatk1_h
    (18168, drop(0.93, 0, false)),  // r3_warrior_longatk2
    (18169, drop(0.90, 0, false)),  // r3_warrior_selfbuff
    (18170, drop(0.90, 0, false)),  // r3_wizard_h
    (18171, drop(0.94, 0, false)),  // r3_wizard_clanbuff
    (18172, drop(0.89, 0, false)),  // r3_wizard_selfbuff
    (18173, drop(0.99, 0, false)),  // r41_roomboss_strong
    (18174, drop(1.00, 22, false)), // r41_roomboss_weak
    (18175, drop(0.93, 0, false)),  // r41_roomboss_teleport
    (18176, drop(0.99, 0, false)),  // r42_roomboss_strong
    (18177, drop(1.00, 22, false)), // r42_roomboss_weak
    (18178, drop(0.93, 0, false)),  // r42_roomboss_teleport
    (18179, drop(0.99, 0, false)),  // r43_roomboss_strong
    (18180, drop(1.00, 22, false)), // r43_roomboss_weak
    (18181, drop(0.93, 0, false)),  // r43_roomboss_teleport
    (18183, drop(0.99, 0, false)),  // r44_roomboss_strong (the second put of 18183 in Java)
    (18184, drop(0.93, 0, false)),  // r44_roomboss_teleport
    (18185, drop(1.00, 23, false)), // r4_healer_srddmagic
    (18186, drop(1.00, 24, false)), // r4_hearler_srdebuff
    (18187, drop(1.00, 20, false)), // r4_warrior
    (18188, drop(0.90, 0, false)),  // r4_warrior_longatk1_h
    (18189, drop(1.00, 20, false)), // r4_warrior_longatk2
    (18190, drop(1.00, 20, false)), // r4_warrior_selfbuff
    (18191, drop(0.89, 0, false)),  // r4_wizard_h
    (18192, drop(1.00, 19, false)), // r4_wizard_clanbuff
    (18193, drop(1.00, 17, false)), // r4_wizard_debuff
    (18194, drop(1.00, 19, false)), // r4_wizard_selfbuff
    (18195, drop(0.91, 0, false)),  // r4_bomb
    (18220, drop(1.00, 24, false)), // r5_healer1
    (18221, drop(1.00, 27, false)), // r5_healer2
    (18222, drop(1.00, 21, false)), // r5_warrior
    (18223, drop(0.90, 0, false)),  // r5_warrior_longatk1_h
    (18224, drop(1.00, 22, false)), // r5_warrior_longatk2
    (18225, drop(1.00, 21, false)), // r5_warrior_sbuff
    (18226, drop(0.89, 0, false)),  // r5_wizard_h
    (18227, drop(1.00, 53, false)), // r5_wizard_clanbuff
    (18228, drop(1.00, 15, false)), // r5_wizard_debuff
    (18229, drop(1.00, 19, false)), // r5_wizard_slefbuff
    (18230, drop(0.49, 0, false)),  // r5_bomb
];

/// Java `Q00619_RelicsOfTheOldEmpire`.
pub struct Q00619RelicsOfTheOldEmpire;

impl Script for Q00619RelicsOfTheOldEmpire {
    fn name(&self) -> &'static str {
        "Q00619_RelicsOfTheOldEmpire"
    }

    fn path(&self) -> &'static str {
        "quests/Q00619_RelicsOfTheOldEmpire"
    }

    fn id(&self) -> i32 {
        619
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GHOST_OF_ADVENTURER]);
        s.add_talk_id(&[GHOST_OF_ADVENTURER]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.add_kill_id(&ARCHON_OF_HALISHA);
        s.register_quest_items(&[BROKEN_RELIC_PART]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31538-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31538-05.html" => htmltext = Some(event.to_owned()),
            "31538-06.html" => {
                if ctx.get_quest_items_count(player, BROKEN_RELIC_PART) >= REQUIRED_RELIC_COUNT {
                    if let Some(recipe) = ctx.get_random_entry(&RECIPES) {
                        ctx.reward_items(player, recipe, 1);
                    }
                    ctx.take_items(player, BROKEN_RELIC_PART, REQUIRED_RELIC_COUNT);
                    htmltext = Some(event.to_owned());
                }
            }
            "31538-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(player, -1, 3, npc)`; the items go to the killer.
        if ctx.random_party_member_state(killer, -1, 3, Some(npc)).is_none() {
            return;
        }
        if ARCHON_OF_HALISHA.contains(&npc.id) {
            let item_count = if ctx.get_random(100) < 79 { 4 } else { 3 };
            ctx.give_item_randomly(
                killer,
                Some(npc),
                BROKEN_RELIC_PART,
                item_count,
                item_count,
                0,
                1.0,
                true,
            );
            return;
        }
        let Some(&(_, info)) = MOBS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        let item_count = if info.double_chance > 0 {
            if ctx.get_random(100) < info.double_chance {
                2
            } else {
                1
            }
        } else {
            1
        };
        ctx.give_item_randomly(
            killer,
            Some(npc),
            BROKEN_RELIC_PART,
            item_count,
            item_count,
            0,
            info.chance,
            true,
        );
        if info.entrance_pass {
            ctx.give_item_randomly(
                killer,
                Some(npc),
                ENTRANCE_PASS_TO_THE_SEPULCHER,
                1,
                1,
                0,
                1.0 / 30.0,
                false,
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31538-01.htm".to_owned()
            } else {
                "31538-03.html".to_owned()
            };
        } else if state == STARTED {
            htmltext = if ctx.get_quest_items_count(player, BROKEN_RELIC_PART) >= REQUIRED_RELIC_COUNT {
                "31538-04.html".to_owned()
            } else {
                "31538-07.html".to_owned()
            };
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00619_RelicsOfTheOldEmpire";

    const NPCS: &str = r#"
        <npc id="31538" type="Folk" level="70" name="Ghost of an Adventurer"/>
        <npc id="21412" type="Monster" level="70" name="Ustralith"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7075" type="EtcItem" name="Entrance Pass"><set name="is_stackable" val="true"/></item>
        <item id="7254" type="EtcItem" name="Broken Relic Part"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6881" type="EtcItem" name="Recipe 6881"/>
        <item id="6883" type="EtcItem" name="Recipe 6883"/>
        <item id="6885" type="EtcItem" name="Recipe 6885"/>
        <item id="6887" type="EtcItem" name="Recipe 6887"/>
        <item id="6891" type="EtcItem" name="Recipe 6891"/>
        <item id="6893" type="EtcItem" name="Recipe 6893"/>
        <item id="6895" type="EtcItem" name="Recipe 6895"/>
        <item id="6897" type="EtcItem" name="Recipe 6897"/>
        <item id="6899" type="EtcItem" name="Recipe 6899"/>
        <item id="7580" type="EtcItem" name="Recipe 7580"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00619RelicsOfTheOldEmpire);
        let pages = Pages::of(&Q00619RelicsOfTheOldEmpire)
            .page("31538-01.htm", &["31538-02.htm"])
            .page("31538-02.htm", &[])
            .plain(&["31538-03.html", "31538-05.html", "31538-06.html"])
            .page("31538-04.html", &["31538-05.html", "31538-06.html"])
            .page("31538-07.html", &["31538-05.html", "31538-08.html"])
            .plain(&["31538-08.html"])
            .npcs(&[GHOST_OF_ADVENTURER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ghost = t.spawn(GHOST_OF_ADVENTURER, 20);
        (t, ghost)
    }

    #[test]
    fn a_character_below_level_74_is_sent_away() {
        let (mut t, ghost) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, ghost).contains("31538-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn one_thousand_relic_parts_buy_a_recipe_and_the_quest_goes_on() {
        let (mut t, ghost) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, ghost).contains("31538-01.htm"));
        assert!(t.event(1, NAME, "31538-02.htm").contains("31538-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ghost).contains("31538-07.html"));

        // 999 parts by the script; an ustralith (100 % and 6 % for a second part) makes 1000+.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BROKEN_RELIC_PART, 999);
        });
        t.take();
        let mob = t.spawn(21412, 40);
        t.kill(1, mob);
        let parts = t.count(1, BROKEN_RELIC_PART);
        assert!(parts >= REQUIRED_RELIC_COUNT);
        assert!(t.take_writes().is_empty());

        // The reward: one recipe, the 1000 parts gone, the page of the rest.
        assert!(t.quest_page(1, ghost).contains("31538-04.html"));
        assert!(t.event(1, NAME, "31538-06.html").contains("31538-06.html"));
        let recipes: i64 = RECIPES.iter().map(|r| t.count(1, *r)).sum();
        assert_eq!(recipes, 1);
        assert_eq!(t.count(1, BROKEN_RELIC_PART), parts - REQUIRED_RELIC_COUNT);
        assert!(t.take_writes().is_empty());

        // Below 1000 again: the exit ends the quest.
        assert!(t.quest_page(1, ghost).contains("31538-07.html"));
        assert!(t.event(1, NAME, "31538-08.html").contains("31538-08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
