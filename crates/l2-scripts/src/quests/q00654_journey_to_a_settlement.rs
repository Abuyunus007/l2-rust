//! Quest 654: Journey to a Settlement (the Nameless Spirit at the Imperial Tomb, the Frintezza scroll).
// Java: dist/game/data/scripts/quests/Q00654_JourneyToASettlement/Q00654_JourneyToASettlement.java
//
// Pattern: a memo-state quest (`set_memo_state(1)` then `2`) that the start NPC drives in two
// talks, then a drop from the Canyon Antelopes (`get_random_party_member_state` with a chance
// per monster) that sets cond 3. The spirit only takes the player who finished Q00119.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NAMELESS_SPIRIT: i32 = 31453;

// Items
const ANTELOPE_SKIN: i32 = 8072;
const FRINTEZZAS_SCROLL: i32 = 8073;

// Misc
const MIN_LEVEL: i32 = 74;

/// Java `MOBS_SKIN`: the Canyon Antelope and its slave with their drop chance.
const MOBS_SKIN: [(i32, f64); 2] = [(21294, 0.840), (21295, 0.893)];

/// Java `Q00654_JourneyToASettlement`.
pub struct Q00654JourneyToASettlement;

impl Script for Q00654JourneyToASettlement {
    fn name(&self) -> &'static str {
        "Q00654_JourneyToASettlement"
    }

    fn path(&self) -> &'static str {
        "quests/Q00654_JourneyToASettlement"
    }

    fn id(&self) -> i32 {
        654
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NAMELESS_SPIRIT]);
        s.add_talk_id(&[NAMELESS_SPIRIT]);
        s.add_kill_id(&[21294, 21295]);
        s.register_quest_items(&[ANTELOPE_SKIN]);
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
            "31453-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "31453-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31453-07.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, ANTELOPE_SKIN) {
                    ctx.give_items(player, FRINTEZZAS_SCROLL, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, 2, 3, Some(npc)) else {
            return;
        };
        let chance = MOBS_SKIN
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0.0, |(_, chance)| *chance);
        if ctx.give_item_randomly(qs.player(), Some(npc), ANTELOPE_SKIN, 1, 1, 1, chance, true) {
            qs.set_cond(ctx, 3, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            let last_imperial_prince_done = ctx
                .player_quest_state(player, "Q00119_LastImperialPrince")
                .is_some_and(|q| q.state() == COMPLETED);
            htmltext = if ctx.player_level(player) >= MIN_LEVEL && last_imperial_prince_done {
                "31453-01.htm"
            } else {
                "31453-04.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if qs.is_memo_state(ctx, 1) {
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                htmltext = "31453-03.html".to_owned();
            } else if qs.is_memo_state(ctx, 2) {
                htmltext = if ctx.has_quest_items(player, ANTELOPE_SKIN) {
                    "31453-06.html"
                } else {
                    "31453-05.html"
                }
                .to_owned();
            }
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

    const NAME: &str = "Q00654_JourneyToASettlement";
    const LAST_IMPERIAL_PRINCE: &str = "Q00119_LastImperialPrince";

    /// Stand-in for Q00119 (Last Imperial Prince): the world only keeps the rows of a known script.
    struct LastImperialPrinceStandIn;

    impl Script for LastImperialPrinceStandIn {
        fn name(&self) -> &'static str {
            LAST_IMPERIAL_PRINCE
        }

        fn path(&self) -> &'static str {
            "quests/Q00119_LastImperialPrince"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    const NPCS: &str = r#"
        <npc id="31453" type="Folk" level="1" name="Nameless Spirit"/>
        <npc id="21294" type="Monster" level="76" name="Canyon Antelope"/>
        <npc id="21295" type="Monster" level="76" name="Canyon Antelope Slave"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8072" type="EtcItem" name="Skin of the Antelope"><set name="is_questitem" val="true"/></item>
        <item id="8073" type="EtcItem" name="Frintezza's Magic Force Field Removal Scroll"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00654JourneyToASettlement);
        registry.add(LastImperialPrinceStandIn);
        let pages = Pages::of(&Q00654JourneyToASettlement)
            .page("31453-01.htm", &["31453-02.htm"])
            .page("31453-02.htm", &["31453-03.html"])
            .plain(&["31453-03.html", "31453-04.htm", "31453-05.html", "31453-07.html"])
            .page("31453-06.html", &["31453-07.html"])
            .npcs(&[NAMELESS_SPIRIT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let spirit = t.spawn(NAMELESS_SPIRIT, 20);
        (t, spirit)
    }

    /// Java `Q00119_LastImperialPrince` completed by the player.
    fn complete_last_imperial_prince(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(LAST_IMPERIAL_PRINCE, Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_spirit_needs_level_74_and_the_last_imperial_prince() {
        let (mut t, spirit) = setup();
        t.enter(1, 74, vec![]);
        // Not completed yet: refused.
        assert!(t.quest_page(1, spirit).contains("31453-04.htm"));
        assert!(t.take_writes().is_empty());

        complete_last_imperial_prince(&mut t);
        assert!(t.quest_page(1, spirit).contains("31453-01.htm"));
    }

    #[test]
    fn a_level_73_character_is_refused_even_after_the_prince() {
        let (mut t, spirit) = setup();
        t.enter(1, 73, vec![]);
        complete_last_imperial_prince(&mut t);
        assert!(t.quest_page(1, spirit).contains("31453-04.htm"));
    }

    #[test]
    fn the_talk_starts_the_memo_and_the_skin_ends_it() {
        let (mut t, spirit) = setup();
        t.enter(1, 74, vec![]);
        complete_last_imperial_prince(&mut t);

        assert!(t.quest_page(1, spirit).contains("31453-01.htm"));
        assert!(t.event(1, NAME, "31453-02.htm").contains("31453-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // The second talk through the spirit's own link sets memo 2 and cond 2.
        assert!(t.event(1, NAME, "31453-03.html").contains("31453-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
            ]
        );

        // Without a skin: 05.
        assert!(t.quest_page(1, spirit).contains("31453-05.html"));
        assert!(t.take_writes().is_empty());

        // The antelopes give the skin: each kill rolls for it, the first skin sets cond 3.
        let mut kills = 0;
        while t.count(1, ANTELOPE_SKIN) < 1 {
            let mob = t.spawn(21294, 60);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "the antelopes never dropped the skin");
        }
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));

        // With the skin: 06, then the scroll and the end of the quest.
        assert!(t.quest_page(1, spirit).contains("31453-06.html"));
        assert!(t.event(1, NAME, "31453-07.html").contains("31453-07.html"));
        assert_eq!(t.count(1, FRINTEZZAS_SCROLL), 1);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
