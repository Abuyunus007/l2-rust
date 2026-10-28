//! Quest 166: Mass of Darkness (Undrias sends a Dark Elf to three priests for the offerings).
// Java: dist/game/data/scripts/quests/Q00166_MassOfDarkness/Q00166_MassOfDarkness.java
//
// Pattern: a delivery chain over several NPCs. Each of the three priests hands out one offering
// once; when the three are in hand the quest moves to cond 2, and Undrias gives the reward and
// ends the quest (one-time). The Newbie Guide's memo (`NRmemo`) counts the delivery, like
// Q00001. Dark Elves only (race gate).

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const UNDRIAS: i32 = 30130;
const IRIA: i32 = 30135;
const DORANKUS: i32 = 30139;
const TRUDY: i32 = 30143;

// Items
const UNDRIAS_LETTER: i32 = 1088;
const CEREMONIAL_DAGGER: i32 = 1089;
const DREVIANT_WINE: i32 = 1090;
const GARMIELS_SCRIPTURE: i32 = 1091;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `NPC_ITEMS`: the offering each priest gives.
fn offering_of(npc_id: i32) -> Option<i32> {
    match npc_id {
        IRIA => Some(CEREMONIAL_DAGGER),
        DORANKUS => Some(DREVIANT_WINE),
        TRUDY => Some(GARMIELS_SCRIPTURE),
        _ => None,
    }
}

/// Java `Q00166_MassOfDarkness`.
pub struct Q00166MassOfDarkness;

impl Script for Q00166MassOfDarkness {
    fn name(&self) -> &'static str {
        "Q00166_MassOfDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00166_MassOfDarkness"
    }

    fn id(&self) -> i32 {
        166
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[UNDRIAS]);
        s.add_talk_id(&[UNDRIAS, IRIA, DORANKUS, TRUDY]);
        s.register_quest_items(&[
            UNDRIAS_LETTER,
            CEREMONIAL_DAGGER,
            DREVIANT_WINE,
            GARMIELS_SCRIPTURE,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30130-03.htm"
        {
            qs.start_quest(ctx);
            ctx.give_items(player, UNDRIAS_LETTER, 1);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            UNDRIAS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::DARK_ELF {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30130-02.htm".to_owned()
                        } else {
                            "30130-01.htm".to_owned()
                        }
                    } else {
                        "30130-00.htm".to_owned()
                    };
                }
                STARTED => {
                    if qs.is_cond(ctx, 2)
                        && ctx.has_all_quest_items(
                            player,
                            &[
                                UNDRIAS_LETTER,
                                CEREMONIAL_DAGGER,
                                DREVIANT_WINE,
                                GARMIELS_SCRIPTURE,
                            ],
                        )
                    {
                        // Newbie Guide: the delivery is counted in its memo (the script must
                        // exist; Java skips the block when it is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10 != 1 {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                        }

                        ctx.add_exp_and_sp(player, 5_672, 466);
                        ctx.give_adena(player, 2_966, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30130-05.html".to_owned();
                    } else {
                        htmltext = "30130-04.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            IRIA | DORANKUS | TRUDY => {
                if qs.is_started(ctx) {
                    let item_id = offering_of(npc.id).unwrap_or_default();
                    if qs.is_cond(ctx, 1) && !ctx.has_quest_items(player, item_id) {
                        ctx.give_items(player, item_id, 1);
                        if ctx.has_all_quest_items(
                            player,
                            &[CEREMONIAL_DAGGER, DREVIANT_WINE, GARMIELS_SCRIPTURE],
                        ) {
                            qs.set_cond(ctx, 2, true);
                        }
                        htmltext = format!("{}-01.html", npc.id);
                    } else {
                        htmltext = format!("{}-02.html", npc.id);
                    }
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00166_MassOfDarkness";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30130" type="Folk" level="1" name="Undrias"/>
        <npc id="30135" type="Folk" level="1" name="Iria"/>
        <npc id="30139" type="Folk" level="1" name="Dorankus"/>
        <npc id="30143" type="Folk" level="1" name="Trudy"/>"#;

    const ITEMS: &str = r#"
        <item id="1088" type="EtcItem" name="Undrias' Letter"><set name="is_questitem" val="true"/></item>
        <item id="1089" type="EtcItem" name="Ceremonial Dagger"><set name="is_questitem" val="true"/></item>
        <item id="1090" type="EtcItem" name="Dreviant Wine"><set name="is_questitem" val="true"/></item>
        <item id="1091" type="EtcItem" name="Garmiel's Scripture"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// The Newbie Guide, read only here: a script of that name (its memo rows are the test's).
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            NEWBIE_GUIDE
        }

        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        undrias: Oid,
        iria: Oid,
        dorankus: Oid,
        trudy: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00166MassOfDarkness);
        let pages = Pages::of(&Q00166MassOfDarkness)
            .page("30130-02.htm", &["30130-03.htm"])
            .plain(&[
                "30130-00.htm",
                "30130-01.htm",
                "30130-03.htm",
                "30130-04.html",
                "30130-05.html",
                "30135-01.html",
                "30135-02.html",
                "30139-01.html",
                "30139-02.html",
                "30143-01.html",
                "30143-02.html",
            ])
            .npcs(&[UNDRIAS, IRIA, DORANKUS, TRUDY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            undrias: t.spawn(UNDRIAS, 20),
            iria: t.spawn(IRIA, 40),
            dorankus: t.spawn(DORANKUS, 60),
            trudy: t.spawn(TRUDY, 80),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_dark_elf_of_level_2_starts_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.undrias).contains("30130-00.htm"));
        t.enter_as(2, 1, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.undrias).contains("30130-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_offerings_then_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.undrias).contains("30130-02.htm"));
        assert!(t.event(1, NAME, "30130-03.htm").contains("30130-03.htm"));
        assert_eq!(t.count(1, UNDRIAS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Iria gives the dagger; the second talk has nothing to give.
        assert!(t.quest_page(1, npcs.iria).contains("30135-01.html"));
        assert!(t.quest_page(1, npcs.iria).contains("30135-02.html"));
        assert_eq!(t.count(1, CEREMONIAL_DAGGER), 1);
        assert!(t.take_writes().is_empty());

        // Dorankus, then Trudy: the last offering sets cond 2.
        assert!(t.quest_page(1, npcs.dorankus).contains("30139-01.html"));
        assert_eq!(t.count(1, DREVIANT_WINE), 1);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, npcs.trudy).contains("30143-01.html"));
        assert_eq!(t.count(1, GARMIELS_SCRIPTURE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Undrias: the Newbie Guide's memo, 5672 exp and 466 sp, 2966 adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.undrias).contains("30130-05.html"));
        assert_eq!(t.count(1, ADENA), 2_966);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NEWBIE_GUIDE, "NRmemo", "41")), "{writes:?}");
        assert!(writes.contains(&rows::insert(NEWBIE_GUIDE, "NRmemoState41", "1")), "{writes:?}");
        assert!(writes.contains(&rows::delete(NAME, false)), "{writes:?}");
        assert!(writes.contains(&rows::update(NAME, "<state>", "Completed")), "{writes:?}");
    }
}
