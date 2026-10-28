//! Quest 303: Collect Arrowheads (Minia in Gludin, Tunath Orc Marksmen).
// Java: dist/game/data/scripts/quests/Q00303_CollectArrowheads/Q00303_CollectArrowheads.java
//
// Pattern: a party kill quest (`docs/design/scripts.md`, "Quest patterns"): `on_kill` picks a
// random party member at cond 1 with `get_random_party_member_cond` and counts the kill for that
// member's quest state. Repeatable: `exit_quest(ctx, true, ..)`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MINIA: i32 = 30029;

// Item
const ORCISH_ARROWHEAD: i32 = 963;

// Misc
const MIN_LEVEL: i32 = 10;
const REQUIRED_ITEM_COUNT: i64 = 10;

// Monster
const TUNATH_ORC_MARKSMAN: i32 = 20361;

/// Java `Q00303_CollectArrowheads`.
pub struct Q00303CollectArrowheads;

impl Script for Q00303CollectArrowheads {
    fn name(&self) -> &'static str {
        "Q00303_CollectArrowheads"
    }

    fn path(&self) -> &'static str {
        "quests/Q00303_CollectArrowheads"
    }

    fn id(&self) -> i32 {
        303
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MINIA]);
        s.add_talk_id(&[MINIA]);
        s.add_kill_id(&[TUNATH_ORC_MARKSMAN]);
        s.register_quest_items(&[ORCISH_ARROWHEAD]);
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
            && event == "30029-04.htm"
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let party_member = ctx.get_random_party_member_cond(player, 1);
        if let Some(party_member) = party_member {
            // The member has a state: `getRandomPartyMember(player, 1)` only picks members at
            // cond 1.
            let Some(qs) = ctx.get_quest_state(party_member, false) else {
                return;
            };
            // NOTE(java-quirk): the arrowhead goes to the killer, not to the member who was
            // drawn, while the member's quest moves to cond 2 (and the killer's count decides).
            if ctx.give_item_randomly(
                player,
                Some(npc),
                ORCISH_ARROWHEAD,
                1,
                1,
                REQUIRED_ITEM_COUNT,
                0.4,
                true,
            ) {
                qs.set_cond(ctx, 2, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30029-03.htm"
                } else {
                    "30029-02.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    if ctx.get_quest_items_count(player, ORCISH_ARROWHEAD) < REQUIRED_ITEM_COUNT {
                        htmltext = "30029-05.html".to_owned();
                    }
                }
                2 => {
                    if ctx.get_quest_items_count(player, ORCISH_ARROWHEAD) >= REQUIRED_ITEM_COUNT {
                        ctx.give_adena(player, 1000, true);
                        ctx.add_exp_and_sp(player, 2000, 0);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30029-06.html".to_owned();
                    }
                }
                _ => {}
            },
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

    const NAME: &str = "Q00303_CollectArrowheads";

    const NPCS: &str = r#"
        <npc id="30029" type="Folk" level="70" name="Minia"/>
        <npc id="20361" type="Monster" level="12" name="Tunath Orc Marksman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="963" type="EtcItem" name="Orcish Arrowhead"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00303CollectArrowheads);
        let pages = Pages::of(&Q00303CollectArrowheads)
            .page("30029-03.htm", &["30029-04.htm"])
            .plain(&["30029-02.htm", "30029-04.htm", "30029-05.html", "30029-06.html"])
            .npcs(&[MINIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let minia = t.spawn(MINIA, 20);
        (t, minia)
    }

    fn start(t: &mut ScriptTest, player: i32, minia: Oid) {
        assert!(t.quest_page(player, minia).contains("30029-03.htm"));
        assert!(t.event(player, NAME, "30029-04.htm").contains("30029-04.htm"));
    }

    #[test]
    fn alone_ten_arrowheads_for_adena_and_exp() {
        let (mut t, minia) = setup();
        t.enter(1, 10, vec![]);
        start(&mut t, 1, minia);
        t.take_writes();
        assert!(t.quest_page(1, minia).contains("30029-05.html"));
        while t.count(1, ORCISH_ARROWHEAD) < 10 {
            let orc = t.spawn(TUNATH_ORC_MARKSMAN, 60);
            t.kill(1, orc);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, minia).contains("30029-06.html"));
        assert_eq!((t.count(1, 57), t.count(1, ORCISH_ARROWHEAD)), (1000, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, minia).contains("30029-03.htm"));
    }

    #[test]
    fn in_a_party_the_killer_gets_the_arrowheads() {
        let (mut t, minia) = setup();
        t.enter(1, 10, vec![]);
        t.enter(2, 10, vec![]);
        // Only player 2 is on the quest; player 1 kills.
        start(&mut t, 2, minia);
        t.party(1, 2);
        t.take_writes();
        while t.count(1, ORCISH_ARROWHEAD) < 10 {
            let orc = t.spawn(TUNATH_ORC_MARKSMAN, 60);
            t.kill(1, orc);
        }
        assert_eq!(t.count(2, ORCISH_ARROWHEAD), 0);
        // Player 2's quest is at cond 2, but without the arrowheads Minia has nothing to say.
        assert_eq!(t.take_writes(), [rows::of(2, rows::update(NAME, "cond", "2"))]);
        let page = t.quest_page(2, minia);
        assert!(!page.contains("30029-06.html"), "{page}");
        assert!(t.take_writes().is_empty());
    }
}
