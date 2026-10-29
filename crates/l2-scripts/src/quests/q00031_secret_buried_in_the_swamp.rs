//! Quest 31: Secret Buried in the Swamp (Abercrombie in the Gludin Village, four forgotten monuments and a dwarf corpse).
// Java: dist/game/data/scripts/quests/Q00031_SecretBuriedInTheSwamp/Q00031_SecretBuriedInTheSwamp.java
//
// Pattern: an ordered visit to several NPCs (`docs/design/scripts.md`, "Quest patterns"): the
// dwarf corpse gives Krorin's journal (cond 2), Abercrombie takes it (cond 3), the four
// monuments are visited in order (cond 4 to 7, the monument's index + 3 is the cond it needs),
// and Abercrombie pays the reward. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ABERCROMBIE: i32 = 31555;
const FORGOTTEN_MONUMENT_1: i32 = 31661;
const FORGOTTEN_MONUMENT_2: i32 = 31662;
const FORGOTTEN_MONUMENT_3: i32 = 31663;
const FORGOTTEN_MONUMENT_4: i32 = 31664;
const CORPSE_OF_DWARF: i32 = 31665;

// Items
const KRORINS_JOURNAL: i32 = 7252;

// Misc
const MIN_LEVEL: i32 = 66;

/// Java `MONUMENTS`: the monuments in the order they are visited.
const MONUMENTS: [i32; 4] = [
    FORGOTTEN_MONUMENT_1,
    FORGOTTEN_MONUMENT_2,
    FORGOTTEN_MONUMENT_3,
    FORGOTTEN_MONUMENT_4,
];

/// Java `Q00031_SecretBuriedInTheSwamp`.
pub struct Q00031SecretBuriedInTheSwamp;

impl Script for Q00031SecretBuriedInTheSwamp {
    fn name(&self) -> &'static str {
        "Q00031_SecretBuriedInTheSwamp"
    }

    fn path(&self) -> &'static str {
        "quests/Q00031_SecretBuriedInTheSwamp"
    }

    fn id(&self) -> i32 {
        31
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ABERCROMBIE]);
        s.add_talk_id(&[ABERCROMBIE, CORPSE_OF_DWARF]);
        s.add_talk_id(&MONUMENTS);
        s.register_quest_items(&[KRORINS_JOURNAL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "31555-02.html" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    return Some(event.to_owned());
                }
            }
            "31665-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    ctx.give_items(player, KRORINS_JOURNAL, 1);
                    return Some(event.to_owned());
                }
            }
            "31555-05.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, KRORINS_JOURNAL) {
                    ctx.take_items(player, KRORINS_JOURNAL, -1);
                    qs.set_cond(ctx, 3, true);
                    return Some(event.to_owned());
                }
            }
            "31661-02.html" | "31662-02.html" | "31663-02.html" | "31664-02.html" => {
                // Java: `npc.getId()`; an event with no NPC cannot match a monument.
                if let Some(npc) = npc
                    && let Some(index) = MONUMENTS.iter().position(|&m| m == npc.id)
                    && qs.is_cond(ctx, index as i32 + 3)
                {
                    qs.set_cond(ctx, qs.get_cond(ctx) + 1, true);
                    return Some(event.to_owned());
                }
            }
            "31555-08.html" => {
                if qs.is_cond(ctx, 7) {
                    ctx.add_exp_and_sp(player, 490_000, 45_880);
                    ctx.give_adena(player, 120_000, true);
                    qs.exit_quest(ctx, false, true);
                    return Some(event.to_owned());
                }
            }
            _ => {}
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ABERCROMBIE => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31555-01.htm"
                    } else {
                        "31555-03.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "31555-02.html".to_owned(),
                    2 => {
                        if ctx.has_quest_items(player, KRORINS_JOURNAL) {
                            htmltext = "31555-04.html".to_owned();
                        }
                    }
                    3 => htmltext = "31555-06.html".to_owned(),
                    7 => htmltext = "31555-07.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            CORPSE_OF_DWARF => match qs.get_cond(ctx) {
                1 => htmltext = "31665-01.html".to_owned(),
                2 => htmltext = "31665-03.html".to_owned(),
                _ => {}
            },
            FORGOTTEN_MONUMENT_1 | FORGOTTEN_MONUMENT_2 | FORGOTTEN_MONUMENT_3 | FORGOTTEN_MONUMENT_4 => {
                let index = MONUMENTS.iter().position(|&m| m == npc.id).unwrap_or_default();
                let loc = index as i32 + 3;
                if qs.is_cond(ctx, loc) {
                    htmltext = format!("{}-01.html", npc.id);
                } else if qs.is_cond(ctx, loc + 1) {
                    htmltext = format!("{}-03.html", npc.id);
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00031_SecretBuriedInTheSwamp";

    const NPCS: &str = r#"
        <npc id="31555" type="Folk" level="70" name="Abercrombie"/>
        <npc id="31661" type="Folk" level="70" name="Forgotten Monument 1"/>
        <npc id="31662" type="Folk" level="70" name="Forgotten Monument 2"/>
        <npc id="31663" type="Folk" level="70" name="Forgotten Monument 3"/>
        <npc id="31664" type="Folk" level="70" name="Forgotten Monument 4"/>
        <npc id="31665" type="Folk" level="70" name="Corpse of Dwarf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7252" type="EtcItem" name="Krorin's Journal"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        abercrombie: Oid,
        corpse: Oid,
        monuments: [Oid; 4],
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00031SecretBuriedInTheSwamp);
        let pages = Pages::of(&Q00031SecretBuriedInTheSwamp)
            .page("31555-01.htm", &["31555-02.html"])
            .page("31555-04.html", &["31555-05.html"])
            .page("31555-07.html", &["31555-08.html"])
            .page("31665-01.html", &["31665-02.html"])
            .page("31661-01.html", &["31661-02.html"])
            .page("31662-01.html", &["31662-02.html"])
            .page("31663-01.html", &["31663-02.html"])
            .page("31664-01.html", &["31664-02.html"])
            .plain(&[
                "31555-02.html",
                "31555-03.htm",
                "31555-05.html",
                "31555-06.html",
                "31555-08.html",
                "31665-02.html",
                "31665-03.html",
                "31661-02.html",
                "31661-03.html",
                "31662-02.html",
                "31662-03.html",
                "31663-02.html",
                "31663-03.html",
                "31664-02.html",
                "31664-03.html",
            ])
            .npcs(&[
                ABERCROMBIE,
                CORPSE_OF_DWARF,
                FORGOTTEN_MONUMENT_1,
                FORGOTTEN_MONUMENT_2,
                FORGOTTEN_MONUMENT_3,
                FORGOTTEN_MONUMENT_4,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            abercrombie: t.spawn(ABERCROMBIE, 20),
            corpse: t.spawn(CORPSE_OF_DWARF, 40),
            monuments: [
                t.spawn(MONUMENTS[0], 60),
                t.spawn(MONUMENTS[1], 80),
                t.spawn(MONUMENTS[2], 100),
                t.spawn(MONUMENTS[3], 120),
            ],
        };
        (t, npcs)
    }

    #[test]
    fn abercrombie_wants_level_66() {
        let (mut t, npcs) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_journal_the_four_monuments_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 66, vec![]);
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-01.htm"));
        assert!(t.event(1, NAME, "31555-02.html").contains("31555-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The dwarf corpse gives the journal (cond 2).
        assert!(t.quest_page(1, npcs.corpse).contains("31665-01.html"));
        assert!(t.event(1, NAME, "31665-02.html").contains("31665-02.html"));
        assert_eq!(t.count(1, KRORINS_JOURNAL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.corpse).contains("31665-03.html"));

        // Abercrombie takes the journal (cond 3).
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-04.html"));
        assert!(t.event(1, NAME, "31555-05.html").contains("31555-05.html"));
        assert_eq!(t.count(1, KRORINS_JOURNAL), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-06.html"));

        // A monument out of turn says nothing (the second one at cond 3).
        assert!(!t.quest_page(1, npcs.monuments[1]).contains("31662-01.html"));

        // The monuments in order: each one moves the cond on (4, 5, 6, 7).
        for (i, monument) in npcs.monuments.into_iter().enumerate() {
            let id = MONUMENTS[i];
            assert!(t.quest_page(1, monument).contains(&format!("{id}-01.html")));
            assert!(
                t.event(1, NAME, &format!("{id}-02.html"))
                    .contains(&format!("{id}-02.html"))
            );
            assert_eq!(
                t.take_writes(),
                [rows::update(NAME, "cond", &(i as i32 + 4).to_string())]
            );
        }
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-07.html"));
        assert!(t.take_writes().is_empty());

        // Abercrombie: the exp and the adena, `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "31555-08.html").contains("31555-08.html"));
        assert_eq!(t.count(1, 57), 120_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.abercrombie)
                .contains("This quest has already been completed.")
        );
    }
}
