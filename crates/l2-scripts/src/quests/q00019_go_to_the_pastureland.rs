//! Quest 19: Go to the Pastureland (Vladimir in the Town of Dion sends a level 82 character to Tunatun).
// Java: dist/game/data/scripts/quests/Q00019_GoToThePastureland/Q00019_GoToThePastureland.java
//
// Pattern: a single errand with a choice of reward (`docs/design/scripts.md`, "Quest patterns"):
// Vladimir gives the veal on the start event, and Tunatun pays for the young wild beast meat
// when the character has it, else for the veal. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const VLADIMIR: i32 = 31302;
const TUNATUN: i32 = 31537;

// Items
const VEAL: i32 = 15532;
const YOUNG_WILD_BEAST_MEAT: i32 = 7547;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00019_GoToThePastureland`.
pub struct Q00019GoToThePastureland;

impl Script for Q00019GoToThePastureland {
    fn name(&self) -> &'static str {
        "Q00019_GoToThePastureland"
    }

    fn path(&self) -> &'static str {
        "quests/Q00019_GoToThePastureland"
    }

    fn id(&self) -> i32 {
        19
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VLADIMIR]);
        s.add_talk_id(&[VLADIMIR, TUNATUN]);
        s.register_quest_items(&[VEAL, YOUNG_WILD_BEAST_MEAT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` answers with the no-quest message
        // (a page), not with the event.
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("31302-02.htm") {
            qs.start_quest(ctx);
            ctx.give_items(player, VEAL, 1);
        } else if event.eq_ignore_ascii_case("31537-02.html") {
            if ctx.has_quest_items(player, YOUNG_WILD_BEAST_MEAT) {
                ctx.give_adena(player, 50_000, true);
                ctx.add_exp_and_sp(player, 136_766, 12_688);
                qs.exit_quest(ctx, false, true);
                htmltext = "31537-02.html".to_owned();
            } else if ctx.has_quest_items(player, VEAL) {
                ctx.give_adena(player, 147_200, true);
                ctx.add_exp_and_sp(player, 385_040, 75_250);
                qs.exit_quest(ctx, false, true);
                htmltext = "31537-02.html".to_owned();
            } else {
                htmltext = "31537-03.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == VLADIMIR {
            match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31302-01.htm"
                    } else {
                        "31302-03.html"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "31302-04.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            }
        } else if npc.id == TUNATUN && qs.is_cond(ctx, 1) {
            htmltext = "31537-01.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00019_GoToThePastureland";

    const NPCS: &str = r#"
        <npc id="31302" type="Folk" level="70" name="Vladimir"/>
        <npc id="31537" type="Folk" level="70" name="Tunatun"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15532" type="EtcItem" name="Veal"><set name="is_questitem" val="true"/></item>
        <item id="7547" type="EtcItem" name="Young Wild Beast Meat"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        vladimir: Oid,
        tunatun: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00019GoToThePastureland);
        let pages = Pages::of(&Q00019GoToThePastureland)
            .page("31302-01.htm", &["31302-02.htm"])
            .page("31537-01.html", &["31537-02.html"])
            .plain(&[
                "31302-02.htm",
                "31302-03.html",
                "31302-04.html",
                "31537-02.html",
                "31537-03.html",
            ])
            .npcs(&[VLADIMIR, TUNATUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            vladimir: t.spawn(VLADIMIR, 20),
            tunatun: t.spawn(TUNATUN, 40),
        };
        (t, npcs)
    }

    #[test]
    fn vladimir_wants_level_82() {
        let (mut t, npcs) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_veal_then_the_adena_for_the_veal() {
        let (mut t, npcs) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-01.htm"));
        assert!(t.event(1, NAME, "31302-02.htm").contains("31302-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, VEAL), 1);
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-04.html"));

        // Tunatun pays for the veal (no meat): `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.tunatun).contains("31537-01.html"));
        assert!(t.event(1, NAME, "31537-02.html").contains("31537-02.html"));
        assert_eq!(t.count(1, 57), 147_200);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.vladimir)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_meat_is_paid_before_the_veal() {
        let (mut t, npcs) = setup();
        t.enter(1, 82, vec![item(100, YOUNG_WILD_BEAST_MEAT, 1)]);
        t.quest_page(1, npcs.vladimir);
        t.event(1, NAME, "31302-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.tunatun).contains("31537-01.html"));
        assert!(t.event(1, NAME, "31537-02.html").contains("31537-02.html"));
        assert_eq!(t.count(1, 57), 50_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
