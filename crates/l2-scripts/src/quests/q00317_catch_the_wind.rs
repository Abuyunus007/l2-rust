//! Quest 317: Catch the Wind (Rizraell in the Elven village, Lireins).
// Java: dist/game/data/scripts/quests/Q00317_CatchTheWind/Q00317_CatchTheWind.java
//
// Pattern: a repeatable quest (`docs/design/scripts.md`, "Quest patterns"): no limit on the
// drops, the NPC buys what the player brings and the player chooses to go on ("09") or to quit
// ("08": `exit_quest(ctx, true, true)`, the quest can be taken again). The kill goes to a random
// started party member, the killer three times as likely (`random_party_member_state`).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RIZRAELL: i32 = 30361;

// Item
const WIND_SHARD: i32 = 1078;

// Misc
const MIN_LEVEL: i32 = 18;
const DROP_CHANCE: f64 = 0.5;

// Monsters
const MONSTERS: [i32; 2] = [
    20036, // Lirein
    20044, // Lirein Elder
];

/// Java `Q00317_CatchTheWind`.
pub struct Q00317CatchTheWind;

impl Script for Q00317CatchTheWind {
    fn name(&self) -> &'static str {
        "Q00317_CatchTheWind"
    }

    fn path(&self) -> &'static str {
        "quests/Q00317_CatchTheWind"
    }

    fn id(&self) -> i32 {
        317
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RIZRAELL]);
        s.add_talk_id(&[RIZRAELL]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[WIND_SHARD]);
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
            "30361-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30361-08.html" | "30361-09.html" => {
                let shard_count = ctx.get_quest_items_count(player, WIND_SHARD);
                if shard_count > 0 {
                    let bonus = if shard_count >= 10 { 2988 } else { 0 };
                    ctx.give_adena(player, shard_count * 40 + bonus, true);
                    ctx.take_items(player, WIND_SHARD, -1);
                }
                if event == "30361-08.html" {
                    qs.exit_quest(ctx, true, true);
                }
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if let Some(qs) = qs {
            // No limit (0): a shard on every successful roll.
            ctx.give_item_randomly(qs.player(), Some(npc), WIND_SHARD, 1, 1, 0, DROP_CHANCE, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30361-03.htm"
            } else {
                "30361-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = if ctx.has_quest_items(player, WIND_SHARD) {
                "30361-07.html"
            } else {
                "30361-05.html"
            }
            .to_owned();
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

    const NAME: &str = "Q00317_CatchTheWind";

    const NPCS: &str = r#"
        <npc id="30361" type="Folk" level="70" name="Rizraell"/>
        <npc id="20036" type="Monster" level="19" name="Lirein"/>
        <npc id="20044" type="Monster" level="20" name="Lirein Elder"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1078" type="EtcItem" name="Wind Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00317CatchTheWind);
        let pages = Pages::of(&Q00317CatchTheWind)
            .page("30361-03.htm", &["30361-04.htm"])
            .page("30361-05.html", &["30361-09.html", "30361-08.html"])
            .page("30361-07.html", &["30361-08.html", "30361-09.html"])
            .plain(&["30361-02.htm", "30361-04.htm", "30361-08.html", "30361-09.html"])
            .npcs(&[RIZRAELL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rizraell = t.spawn(RIZRAELL, 20);
        (t, rizraell)
    }

    fn hunt(t: &mut ScriptTest, shards: i64) {
        while t.count(1, WIND_SHARD) < shards {
            let lirein = t.spawn(MONSTERS[0], 60);
            t.kill(1, lirein);
        }
    }

    #[test]
    fn sell_go_on_then_quit() {
        let (mut t, rizraell) = setup();
        t.enter(1, 18, vec![]);
        assert!(t.quest_page(1, rizraell).contains("30361-03.htm"));
        assert!(t.event(1, NAME, "30361-04.htm").contains("30361-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // A second "accept" does nothing: the state is not CREATED any more.
        assert!(t.quest_page(1, rizraell).contains("30361-05.html"));

        // Ten or more shards: 40 each and 2988 on top; the quest goes on ("09").
        hunt(&mut t, 10);
        let shards = t.count(1, WIND_SHARD);
        assert!(t.quest_page(1, rizraell).contains("30361-07.html"));
        assert!(t.event(1, NAME, "30361-09.html").contains("30361-09.html"));
        assert_eq!(t.count(1, 57), shards * 40 + 2988);
        assert_eq!(t.count(1, WIND_SHARD), 0);
        assert!(t.take_writes().is_empty());

        // Fewer than ten: no bonus; "08" ends the quest so it can be taken again.
        let adena = t.count(1, 57);
        hunt(&mut t, 1);
        assert!(t.quest_page(1, rizraell).contains("30361-07.html"));
        assert!(t.event(1, NAME, "30361-08.html").contains("30361-08.html"));
        assert_eq!(t.count(1, 57), adena + 40);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, rizraell).contains("30361-03.htm"));
    }

    #[test]
    fn a_kill_goes_to_a_started_party_member() {
        let (mut t, rizraell) = setup();
        t.enter(1, 18, vec![]);
        t.enter(2, 18, vec![]);
        // Only player 2 is on the quest: every shard is player 2's.
        assert!(t.quest_page(2, rizraell).contains("30361-03.htm"));
        t.event(2, NAME, "30361-04.htm");
        t.party(1, 2);
        for _ in 0..10 {
            let lirein = t.spawn(MONSTERS[1], 60);
            t.kill(1, lirein);
        }
        assert_eq!(t.count(1, WIND_SHARD), 0);
        assert!(t.count(2, WIND_SHARD) > 0);
    }
}
