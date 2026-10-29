//! Quest 173: To the Isle of Souls (Galladucci and Gentler in the Kamael village).
// Java: dist/game/data/scripts/quests/Q00173_ToTheIsleOfSouls/Q00173_ToTheIsleOfSouls.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GALLADUCCI: i32 = 30097;
const GENTLER: i32 = 30094;

// Items
const GALLADUCCIS_ORDER: i32 = 7563;
const MAGIC_SWORD_HILT: i32 = 7568;
const MARK_OF_TRAVELER: i32 = 7570;
const SCROLL_OF_ESCAPE_KAMAEL_VILLAGE: i32 = 9716;

/// Java `Q00172_NewHorizons.class.getSimpleName()`: the quest this one follows.
const NEW_HORIZONS: &str = "Q00172_NewHorizons";

/// Java `Q00173_ToTheIsleOfSouls`.
pub struct Q00173ToTheIsleOfSouls;

impl Script for Q00173ToTheIsleOfSouls {
    fn name(&self) -> &'static str {
        "Q00173_ToTheIsleOfSouls"
    }

    fn path(&self) -> &'static str {
        "quests/Q00173_ToTheIsleOfSouls"
    }

    fn id(&self) -> i32 {
        173
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GALLADUCCI]);
        s.add_talk_id(&[GALLADUCCI, GENTLER]);
        s.register_quest_items(&[GALLADUCCIS_ORDER, MAGIC_SWORD_HILT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let st = ctx.get_quest_state(player, false)?;

        match event {
            "30097-03.htm" => {
                st.start_quest(ctx);
                ctx.give_items(player, GALLADUCCIS_ORDER, 1);
                Some(event.to_owned())
            }
            "30097-06.html" => {
                ctx.give_items(player, SCROLL_OF_ESCAPE_KAMAEL_VILLAGE, 1);
                ctx.take_items(player, MARK_OF_TRAVELER, 1);
                st.exit_quest(ctx, false, true);
                Some(event.to_owned())
            }
            "30094-02.html" => {
                st.set_cond(ctx, 2, true);
                ctx.take_items(player, GALLADUCCIS_ORDER, -1);
                ctx.give_items(player, MAGIC_SWORD_HILT, 1);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GALLADUCCI => match st.state(ctx) {
                CREATED => {
                    // `player.getQuestState(Q00172_NewHorizons)`: the other quest's state, read only.
                    let previous_done = ctx
                        .player_quest_state(player, NEW_HORIZONS)
                        .is_some_and(|qs| qs.is_completed());
                    htmltext = if previous_done
                        && ctx.player_race(player) == race::KAMAEL
                        && ctx.has_quest_items(player, MARK_OF_TRAVELER)
                    {
                        "30097-01.htm"
                    } else {
                        "30097-02.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext = if st.is_cond(ctx, 1) {
                        "30097-04.html"
                    } else {
                        "30097-05.html"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            GENTLER => {
                if st.is_started(ctx) {
                    htmltext = if st.is_cond(ctx, 1) {
                        "30094-01.html"
                    } else {
                        "30094-03.html"
                    }
                    .to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00173_ToTheIsleOfSouls";

    const NPCS: &str = r#"
        <npc id="30097" type="Folk" level="70" name="Galladucci"/>
        <npc id="30094" type="Folk" level="70" name="Gentler"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7563" type="EtcItem" name="Galladucci's Order"><set name="is_questitem" val="true"/></item>
        <item id="7568" type="EtcItem" name="Magic Sword Hilt"><set name="is_questitem" val="true"/></item>
        <item id="7570" type="EtcItem" name="Mark of Traveler"><set name="is_questitem" val="true"/></item>
        <item id="9716" type="EtcItem" name="Scroll of Escape: Kamael Village"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00172_NewHorizons` as far as this quest reads it: a script of that name.
    struct NewHorizons;

    impl Script for NewHorizons {
        fn name(&self) -> &'static str {
            NEW_HORIZONS
        }
        fn path(&self) -> &'static str {
            "quests/Q00172_NewHorizons"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewHorizons);
        registry.add(Q00173ToTheIsleOfSouls);
        let pages = Pages::of(&Q00173ToTheIsleOfSouls)
            .page("30097-01.htm", &["30097-03.htm"])
            .plain(&["30097-02.htm", "30097-03.htm", "30097-04.html"])
            .page("30097-05.html", &["30097-06.html"])
            .plain(&["30097-06.html"])
            .page("30094-01.html", &["30094-02.html"])
            .plain(&["30094-02.html", "30094-03.html"])
            .npcs(&[GALLADUCCI, GENTLER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let galladucci = t.spawn(GALLADUCCI, 20);
        let gentler = t.spawn(GENTLER, 40);
        (t, galladucci, gentler)
    }

    /// Java's `getQuestState(..)` of the previous quest: completed or not.
    fn finish_new_horizons(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(NEW_HORIZONS, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
    }

    #[test]
    fn the_kamael_who_finished_new_horizons_with_the_mark_is_offered_the_quest() {
        let (mut t, galladucci, _) = setup();
        t.enter_as(1, 10, race::KAMAEL, 123, vec![item(100, MARK_OF_TRAVELER, 1)]);
        assert!(t.quest_page(1, galladucci).contains("30097-02.htm"));

        finish_new_horizons(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, galladucci).contains("30097-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_order_then_the_hilt_then_the_scroll() {
        let (mut t, galladucci, gentler) = setup();
        t.enter_as(1, 10, race::KAMAEL, 123, vec![item(100, MARK_OF_TRAVELER, 1)]);
        finish_new_horizons(&mut t, 1);
        t.take_writes();

        assert!(t.quest_page(1, galladucci).contains("30097-01.htm"));
        assert!(t.event(1, NAME, "30097-03.htm").contains("30097-03.htm"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert_eq!(t.count(1, GALLADUCCIS_ORDER), 1);
        assert!(t.quest_page(1, galladucci).contains("30097-04.html"));

        // Gentler takes the order and gives the hilt; cond 2 (`setCond(2, true)`).
        assert!(t.quest_page(1, gentler).contains("30094-01.html"));
        assert!(t.event(1, NAME, "30094-02.html").contains("30094-02.html"));
        assert_eq!((t.count(1, GALLADUCCIS_ORDER), t.count(1, MAGIC_SWORD_HILT)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, gentler).contains("30094-03.html"));

        // Galladucci: the scroll, the mark goes, `exitQuest(false, true)`.
        assert!(t.quest_page(1, galladucci).contains("30097-05.html"));
        assert!(t.event(1, NAME, "30097-06.html").contains("30097-06.html"));
        assert_eq!((t.count(1, SCROLL_OF_ESCAPE_KAMAEL_VILLAGE), t.count(1, MARK_OF_TRAVELER)), (1, 0));
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, galladucci).contains("already been completed"));
    }
}
