//! Quest 257: The Guard Is Busy (Gilbert in the Gludio guard post sends the characters of level 6
//! and up to kill the orcs and werewolves of the Gludio area for their amulets, necklaces and
//! fangs).
// Java: dist/game/data/scripts/quests/Q00257_TheGuardIsBusy/Q00257_TheGuardIsBusy.java
//
// Pattern: a repeatable kill count with a random drop table per monster (`MobDrop`: a roll per
// entry until one drops, `docs/design/scripts.md`, "Kills, drops and parties"), a trade of the
// items at the end, the rookie shots and the newbie guide's memo.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GILBERT: i32 = 30039;

// Items
const GLUDIO_LORDS_MARK: i32 = 1084;
const ORC_AMULET: i32 = 752;
const ORC_NECKLACE: i32 = 1085;
const WEREWOLF_FANG: i32 = 1086;
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);

// Misc
const MIN_LEVEL: i32 = 6;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00257_TheGuardIsBusy.MobDrop`: a drop when `getRandom(random) < chance`.
#[derive(Clone, Copy)]
struct MobDrop {
    random: i32,
    chance: i32,
    id: i32,
    count: i64,
}

const fn mob_drop(random: i32, chance: i32, id: i32, count: i64) -> MobDrop {
    MobDrop {
        random,
        chance,
        id,
        count,
    }
}

/// Java `MONSTERS`: the drops of each monster, in order (the first one that drops is given).
const MONSTERS: [(i32, &[MobDrop]); 9] = [
    // Orc Archer
    (
        20006,
        &[mob_drop(10, 2, ORC_AMULET, 2), mob_drop(10, 10, ORC_AMULET, 1)],
    ),
    // Orc Fighter
    (20093, &[mob_drop(100, 85, ORC_NECKLACE, 1)]),
    // Orc Fighter Sub Leader
    (20096, &[mob_drop(100, 95, ORC_NECKLACE, 1)]),
    // Orc Fighter Leader
    (20098, &[mob_drop(100, 100, ORC_NECKLACE, 1)]),
    // Orc
    (20130, &[mob_drop(10, 7, ORC_AMULET, 1)]),
    // Orc Grunt
    (20131, &[mob_drop(10, 9, ORC_AMULET, 1)]),
    // Werewolf
    (20132, &[mob_drop(10, 7, WEREWOLF_FANG, 1)]),
    // Werewolf Chieftain
    (20342, &[mob_drop(0, 1, WEREWOLF_FANG, 1)]),
    // Werewolf Hunter
    (20343, &[mob_drop(100, 85, WEREWOLF_FANG, 1)]),
];

/// Java `Q00257_TheGuardIsBusy`.
pub struct Q00257TheGuardIsBusy;

impl Script for Q00257TheGuardIsBusy {
    fn name(&self) -> &'static str {
        "Q00257_TheGuardIsBusy"
    }

    fn path(&self) -> &'static str {
        "quests/Q00257_TheGuardIsBusy"
    }

    fn id(&self) -> i32 {
        257
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GILBERT]);
        s.add_talk_id(&[GILBERT]);
        for (mob, _) in MONSTERS {
            s.add_kill_id(&[mob]);
        }
        s.register_quest_items(&[ORC_AMULET, GLUDIO_LORDS_MARK, ORC_NECKLACE, WEREWOLF_FANG]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30039-03.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, GLUDIO_LORDS_MARK, 1);
                Some(event.to_owned())
            }
            "30039-05.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            "30039-06.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_none() {
            return;
        }
        let Some((_, drops)) = MONSTERS.iter().find(|(mob, _)| *mob == npc.id) else {
            return;
        };
        for drop in drops.iter() {
            if ctx.get_random(drop.random) < drop.chance {
                ctx.give_items(killer, drop.id, drop.count);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                break;
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30039-02.htm"
                } else {
                    "30039-01.html"
                }
                .to_owned();
            }
            STARTED => {
                let registered = [ORC_AMULET, ORC_NECKLACE, WEREWOLF_FANG];
                if ctx.has_at_least_one_quest_item(player, &registered) {
                    let amulets = ctx.get_quest_items_count(player, ORC_AMULET);
                    let common = ctx.get_quest_items_count_of(player, &[ORC_NECKLACE, WEREWOLF_FANG]);
                    let bonus = if amulets + common >= 10 { 1000 } else { 0 };
                    ctx.give_adena(player, amulets * 10 + common * 20 + bonus, true);
                    ctx.take_items_many(player, -1, &registered);

                    if ctx.player_level(player) < 25 && ctx.get_one_time_quest_flag(player, 57) == 0 {
                        let (item, count) = if ctx.is_mage_class(player) {
                            ctx.play_sound_named(player, "tutorial_voice_027");
                            SPIRITSHOTS_NO_GRADE_FOR_ROOKIES
                        } else {
                            ctx.play_sound_named(player, "tutorial_voice_026");
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES
                        };
                        ctx.give_items(player, item, count);
                        ctx.set_one_time_quest_flag(player, 57, 1);
                    }

                    // Newbie Guide (only when `ai/others/NewbieGuide` is installed, as in Java).
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10000) / 1000 != 1 {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                    }

                    htmltext = "30039-07.html".to_owned();
                } else {
                    htmltext = "30039-04.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::player_class;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00257_TheGuardIsBusy";

    const NPCS: &str = r#"
        <npc id="30039" type="Folk" level="70" name="Gilbert"/>
        <npc id="20093" type="Monster" level="10" name="Orc Fighter"/>
        <npc id="20130" type="Monster" level="6" name="Orc"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1084" type="EtcItem" name="Gludio Lord's Mark"><set name="is_questitem" val="true"/></item>
        <item id="752" type="EtcItem" name="Orc Amulet"><set name="is_questitem" val="true"/></item>
        <item id="1085" type="EtcItem" name="Orc Necklace"><set name="is_questitem" val="true"/></item>
        <item id="1086" type="EtcItem" name="Werewolf Fang"><set name="is_questitem" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="5790" type="EtcItem" name="Spiritshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00057_NewbieShots`, the quest the one-time flag 57 reads: a stand-in with its id.
    struct NewbieShots;

    impl Script for NewbieShots {
        fn name(&self) -> &'static str {
            "Q00057_NewbieShots"
        }
        fn path(&self) -> &'static str {
            "quests/Dummy/Q00057_NewbieShots"
        }
        fn id(&self) -> i32 {
            57
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieShots);
        registry.add(Q00257TheGuardIsBusy);
        let pages = Pages::of(&Q00257TheGuardIsBusy)
            .page("30039-02.htm", &["30039-03.htm"])
            .plain(&["30039-01.html", "30039-03.htm", "30039-04.html", "30039-06.html"])
            .page("30039-07.html", &["30039-05.html", "30039-06.html"])
            .plain(&["30039-05.html"])
            .npcs(&[GILBERT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gilbert = t.spawn(GILBERT, 20);
        (t, gilbert)
    }

    #[test]
    fn a_character_below_level_6_is_sent_away() {
        let (mut t, gilbert) = setup();
        t.enter_as(1, 5, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, gilbert).contains("30039-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_necklaces_are_traded_and_the_quest_repeats() {
        let (mut t, gilbert) = setup();
        t.enter_as(1, 6, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, gilbert).contains("30039-02.htm"));
        assert!(t.event(1, NAME, "30039-03.htm").contains("30039-03.htm"));
        assert_eq!(t.count(1, GLUDIO_LORDS_MARK), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, gilbert).contains("30039-04.html"));

        // Orc Fighters drop necklaces at 85%: three of them, then Gilbert pays 3 x 20 adena.
        for _ in 0..200 {
            if t.count(1, ORC_NECKLACE) >= 3 {
                break;
            }
            let fighter = t.spawn(20093, 20);
            t.kill(1, fighter);
        }
        assert_eq!(t.count(1, ORC_NECKLACE), 3);
        assert!(t.quest_page(1, gilbert).contains("30039-07.html"));
        assert_eq!(t.count(1, 57), 60);
        assert_eq!(t.count(1, ORC_NECKLACE), 0);
        // The mark stays with the player.
        assert_eq!(t.count(1, GLUDIO_LORDS_MARK), 1);
        // The rookie soulshots (a fighter, level below 25, the flag not set yet).
        assert_eq!(t.count(1, 5789), 6000);
        // The flag of the payment is written as the state of Q00057.
        assert_eq!(
            t.take_writes(),
            [rows::insert("Q00057_NewbieShots", "<state>", "Completed")]
        );

        // Quit: `exitQuest(true, true)` deletes the quest; the mark is removed with the items.
        assert!(t.event(1, NAME, "30039-05.html").contains("30039-05.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, gilbert).contains("30039-02.htm"));
    }
}
