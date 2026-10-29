//! Quest 12: Secret Meeting With Varka Silenos (Cadmon, Helmut and Naran in the Varka Silenos Village).
// Java: dist/game/data/scripts/quests/Q00012_SecretMeetingWithVarkaSilenos/Q00012_SecretMeetingWithVarkaSilenos.java
//
// Pattern: the Varka twin of Q00011 (`docs/design/scripts.md`, "Quest patterns"): Cadmon starts
// it, Helmut gives the box (cond 2), and Naran takes the box for the reward. One-time:
// `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CADMON: i32 = 31296;
const HELMUT: i32 = 31258;
const NARAN: i32 = 31378;

// Item
const BOX: i32 = 7232;

// Misc
const MIN_LEVEL: i32 = 74;

/// Java `Q00012_SecretMeetingWithVarkaSilenos`.
pub struct Q00012SecretMeetingWithVarkaSilenos;

impl Script for Q00012SecretMeetingWithVarkaSilenos {
    fn name(&self) -> &'static str {
        "Q00012_SecretMeetingWithVarkaSilenos"
    }

    fn path(&self) -> &'static str {
        "quests/Q00012_SecretMeetingWithVarkaSilenos"
    }

    fn id(&self) -> i32 {
        12
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CADMON]);
        s.add_talk_id(&[CADMON, HELMUT, NARAN]);
        s.register_quest_items(&[BOX]);
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
            "31296-03.html" => qs.start_quest(ctx),
            "31258-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    ctx.give_items(player, BOX, 1);
                }
            }
            "31378-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, BOX) {
                    ctx.add_exp_and_sp(player, 233_125, 18_142);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = "31378-03.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc_id == CADMON {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31296-01.htm"
                    } else {
                        "31296-02.html"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                let cond = qs.get_cond(ctx);
                if npc_id == CADMON && cond == 1 {
                    htmltext = "31296-04.html".to_owned();
                } else if npc_id == HELMUT {
                    if cond == 1 {
                        htmltext = "31258-01.html".to_owned();
                    } else if cond == 2 {
                        htmltext = "31258-03.html".to_owned();
                    }
                } else if npc_id == NARAN && cond == 2 {
                    htmltext = "31378-01.html".to_owned();
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

    const NAME: &str = "Q00012_SecretMeetingWithVarkaSilenos";

    const NPCS: &str = r#"
        <npc id="31296" type="Folk" level="70" name="Cadmon"/>
        <npc id="31258" type="Folk" level="70" name="Helmut"/>
        <npc id="31378" type="Folk" level="70" name="Naran"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7232" type="EtcItem" name="Box"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        cadmon: Oid,
        helmut: Oid,
        naran: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00012SecretMeetingWithVarkaSilenos);
        let pages = Pages::of(&Q00012SecretMeetingWithVarkaSilenos)
            .page("31296-01.htm", &["31296-03.html"])
            .page("31258-01.html", &["31258-02.html"])
            .page("31378-01.html", &["31378-02.html"])
            .plain(&[
                "31296-02.html",
                "31296-03.html",
                "31296-04.html",
                "31258-02.html",
                "31258-03.html",
                "31378-02.html",
                "31378-03.html",
            ])
            .npcs(&[CADMON, HELMUT, NARAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            cadmon: t.spawn(CADMON, 20),
            helmut: t.spawn(HELMUT, 40),
            naran: t.spawn(NARAN, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_character_of_level_74_is_sent_to_the_meeting() {
        let (mut t, npcs) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, npcs.cadmon).contains("31296-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_box_from_helmut_to_naran() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.cadmon).contains("31296-01.htm"));
        assert!(t.event(1, NAME, "31296-03.html").contains("31296-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.cadmon).contains("31296-04.html"));

        // Naran has nothing before the box.
        assert!(!t.quest_page(1, npcs.naran).contains("31378-01.html"));

        // Helmut gives the box (cond 2).
        assert!(t.quest_page(1, npcs.helmut).contains("31258-01.html"));
        assert!(t.event(1, NAME, "31258-02.html").contains("31258-02.html"));
        assert_eq!(t.count(1, BOX), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.helmut).contains("31258-03.html"));

        // Naran takes the box: the experience and `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.naran).contains("31378-01.html"));
        assert!(t.event(1, NAME, "31378-02.html").contains("31378-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.cadmon)
                .contains("This quest has already been completed.")
        );
    }
}
