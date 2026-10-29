//! Quest 17: Light and Darkness (the Hierarch and four saint altars, for a level 61 character).
// Java: dist/game/data/scripts/quests/Q00017_LightAndDarkness/Q00017_LightAndDarkness.java
//
// Pattern: an ordered visit to four NPCs with a blood per altar (`docs/design/scripts.md`,
// "Quest patterns"): the Hierarch gives four blood, each altar takes one when it is the next
// altar in order (cond = altar index), and the Hierarch pays the exp at cond 5. The start needs
// Sweet Whispers (Q00015) completed. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIERARCH: i32 = 31517;
const SAINT_ALTAR_1: i32 = 31508;
const SAINT_ALTAR_2: i32 = 31509;
const SAINT_ALTAR_3: i32 = 31510;
const SAINT_ALTAR_4: i32 = 31511;

// Item
const BLOOD_OF_SAINT: i32 = 7168;

// Misc
const MIN_LEVEL: i32 = 61;
/// Java `Q00015_SweetWhispers.class.getSimpleName()`: the quest that must be completed first.
const SWEET_WHISPERS: &str = "Q00015_SweetWhispers";

/// Java `Q00017_LightAndDarkness`.
pub struct Q00017LightAndDarkness;

impl Script for Q00017LightAndDarkness {
    fn name(&self) -> &'static str {
        "Q00017_LightAndDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00017_LightAndDarkness"
    }

    fn id(&self) -> i32 {
        17
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIERARCH]);
        s.add_talk_id(&[
            HIERARCH,
            SAINT_ALTAR_1,
            SAINT_ALTAR_2,
            SAINT_ALTAR_3,
            SAINT_ALTAR_4,
        ]);
        s.register_quest_items(&[BLOOD_OF_SAINT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` returns the event itself (not null),
        // so the event's page is shown anyway.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        let mut htmltext = event.to_owned();
        match event {
            "31517-02.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    ctx.give_items(player, BLOOD_OF_SAINT, 4);
                } else {
                    htmltext = "31517-02a.html".to_owned();
                }
            }
            "31508-02.html" | "31509-02.html" | "31510-02.html" | "31511-02.html" => {
                let cond = qs.get_cond(ctx);
                let npc_id = event.replace("-02.html", "").parse::<i32>().unwrap_or_default();
                if cond == npc_id - 31507 && ctx.has_quest_items(player, BLOOD_OF_SAINT) {
                    htmltext = format!("{npc_id}-01.html");
                    ctx.take_items(player, BLOOD_OF_SAINT, 1);
                    qs.set_cond(ctx, cond + 1, true);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                // Java: `player.getQuestState(Q00015...)` completed.
                let sweet_whispers_done = ctx
                    .player_quest_state(player, SWEET_WHISPERS)
                    .is_some_and(|q| q.is_completed());
                htmltext = if sweet_whispers_done {
                    "31517-00.htm"
                } else {
                    "31517-06.html"
                }
                .to_owned();
            }
            STARTED => {
                let blood = ctx.get_quest_items_count(player, BLOOD_OF_SAINT);
                match npc.id {
                    HIERARCH => {
                        if qs.get_cond(ctx) < 5 {
                            htmltext = if blood >= 5 {
                                "31517-05.html"
                            } else {
                                "31517-04.html"
                            }
                            .to_owned();
                        } else {
                            ctx.add_exp_and_sp(player, 697_040, 54_887);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "31517-03.html".to_owned();
                        }
                    }
                    SAINT_ALTAR_1 | SAINT_ALTAR_2 | SAINT_ALTAR_3 | SAINT_ALTAR_4 => {
                        let npc_id = npc.id;
                        let index = npc_id - 31507;
                        if index == qs.get_cond(ctx) {
                            htmltext = if blood > 0 {
                                format!("{npc_id}-00.html")
                            } else {
                                format!("{npc_id}-02.html")
                            };
                        } else if qs.get_cond(ctx) > index {
                            htmltext = format!("{npc_id}-03.html");
                        }
                    }
                    _ => {}
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
    use l2_world::{ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00017_LightAndDarkness";

    const NPCS: &str = r#"
        <npc id="31517" type="Folk" level="70" name="Hierarch"/>
        <npc id="31508" type="Folk" level="70" name="Saint Altar 1"/>
        <npc id="31509" type="Folk" level="70" name="Saint Altar 2"/>
        <npc id="31510" type="Folk" level="70" name="Saint Altar 3"/>
        <npc id="31511" type="Folk" level="70" name="Saint Altar 4"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7168" type="EtcItem" name="Blood of Saint"><set name="is_questitem" val="true"/></item>"#;

    const ALTARS: [i32; 4] = [31508, 31509, 31510, 31511];

    struct Npcs {
        hierarch: Oid,
        altars: [Oid; 4],
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SweetWhispers);
        registry.add(Q00017LightAndDarkness);
        let mut pages = Pages::of(&Q00017LightAndDarkness)
            .page("31517-00.htm", &["31517-01.htm"])
            .page("31517-01.htm", &["31517-02.html"])
            .plain(&[
                "31517-02.html",
                "31517-02a.html",
                "31517-03.html",
                "31517-04.html",
                "31517-05.html",
                "31517-06.html",
            ]);
        for altar in ALTARS {
            pages = pages
                .page(&format!("{altar}-00.html"), &[&format!("{altar}-02.html")])
                .plain(&[
                    &format!("{altar}-01.html"),
                    &format!("{altar}-02.html"),
                    &format!("{altar}-03.html"),
                ]);
        }
        let pages = pages.npcs(&[HIERARCH, 31508, 31509, 31510, 31511]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            hierarch: t.spawn(HIERARCH, 20),
            altars: [
                t.spawn(ALTARS[0], 40),
                t.spawn(ALTARS[1], 60),
                t.spawn(ALTARS[2], 80),
                t.spawn(ALTARS[3], 100),
            ],
        };
        (t, npcs)
    }

    /// Java `Q00015_SweetWhispers` as far as this quest needs it: a script of that name with
    /// no NPCs, so the Hierarch's page lists only Light and Darkness.
    struct SweetWhispers;

    impl Script for SweetWhispers {
        fn name(&self) -> &'static str {
            "Q00015_SweetWhispers"
        }
        fn path(&self) -> &'static str {
            "quests/Q00015_SweetWhispers"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// Java: Sweet Whispers completed (the player's state of that quest), made through its
    /// own exit path (`exitQuest(false)`), since the quest itself is not in this test world.
    fn complete_sweet_whispers(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script("Q00015_SweetWhispers", |ctx| {
            if let Some(qs) = ctx.get_quest_state(ObjectId(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn the_hierarch_sends_the_player_to_sweet_whispers_first() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![]);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_four_saints_in_order_then_the_exp() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![]);
        complete_sweet_whispers(&mut t, 1);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-00.htm"));
        assert!(t.event(1, NAME, "31517-01.htm").contains("31517-01.htm"));
        assert!(t.event(1, NAME, "31517-02.html").contains("31517-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, BLOOD_OF_SAINT), 4);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-04.html"));

        // Each altar takes one blood, in order; a passed altar says "already done".
        for (i, altar) in npcs.altars.into_iter().enumerate() {
            let id = ALTARS[i];
            assert!(t.quest_page(1, altar).contains(&format!("{id}-00.html")));
            assert!(
                t.event(1, NAME, &format!("{id}-02.html"))
                    .contains(&format!("{id}-01.html"))
            );
            assert_eq!(t.count(1, BLOOD_OF_SAINT), 3 - i as i64);
            assert_eq!(
                t.take_writes(),
                [rows::update(NAME, "cond", &(i as i32 + 2).to_string())]
            );
            assert!(t.quest_page(1, altar).contains(&format!("{id}-03.html")));
        }

        // The Hierarch at cond 5: the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.hierarch)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_character_below_level_61_is_refused_at_the_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![]);
        complete_sweet_whispers(&mut t, 1);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-00.htm"));
        assert!(t.event(1, NAME, "31517-01.htm").contains("31517-01.htm"));
        assert!(t.event(1, NAME, "31517-02.html").contains("31517-02a.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, BLOOD_OF_SAINT), 0);
    }
}
