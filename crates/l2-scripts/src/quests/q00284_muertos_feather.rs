//! Quest 284: Muertos Feather (Trevor in the Hunters Village buys the Muertos feathers).
// Java: dist/game/data/scripts/quests/Q00284_MuertosFeather/Q00284_MuertosFeather.java
//
// Pattern: a party drop quest without a limit (`docs/design/scripts.md`, "Quest patterns"): a
// random party member gets a Muertos feather from a Muertos monster with a drop chance; Trevor pays
// 45 adena per feather and the quest repeats. The quest has no condition: it is started until it
// is left.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TREVOR: i32 = 32166;

// Item
const MUERTOS_FEATHER: i32 = 9748;

/// Java `MOB_DROP_CHANCE`: the Muertos monsters and their drop chance.
const MOB_DROP_CHANCE: [(i32, f64); 6] = [
    (22239, 0.500), // Muertos Guard
    (22240, 0.533), // Muertos Scout
    (22242, 0.566), // Muertos Warrior
    (22243, 0.600), // Muertos Captain
    (22245, 0.633), // Muertos Lieutenant
    (22246, 0.633), // Muertos Commander
];

// Misc
const MIN_LEVEL: i32 = 11;

/// Java `Q00284_MuertosFeather`.
pub struct Q00284MuertosFeather;

impl Script for Q00284MuertosFeather {
    fn name(&self) -> &'static str {
        "Q00284_MuertosFeather"
    }

    fn path(&self) -> &'static str {
        "quests/Q00284_MuertosFeather"
    }

    fn id(&self) -> i32 {
        284
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TREVOR]);
        s.add_talk_id(&[TREVOR]);
        let ids: Vec<i32> = MOB_DROP_CHANCE.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[MUERTOS_FEATHER]);
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
        let mut html = None;
        match event {
            "32166-03.htm" => {
                qs.start_quest(ctx);
                html = Some(event.to_owned());
            }
            "32166-06.html" => html = Some(event.to_owned()),
            "32166-08.html" => {
                if ctx.has_quest_items(player, MUERTOS_FEATHER) {
                    let count = ctx.get_quest_items_count(player, MUERTOS_FEATHER);
                    ctx.give_adena(player, count * 45, true);
                    ctx.take_items(player, MUERTOS_FEATHER, -1);
                    html = Some(event.to_owned());
                } else {
                    html = Some("32166-07.html".to_owned());
                }
            }
            "32166-09.html" => {
                qs.exit_quest(ctx, true, true);
                html = Some(event.to_owned());
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        let chance = MOB_DROP_CHANCE
            .iter()
            .find(|&&(id, _)| id == npc.id)
            .map_or(0.0, |&(_, chance)| chance);
        // Limit 0: no limit on the feathers.
        ctx.give_item_randomly(member.player(), Some(npc), MUERTOS_FEATHER, 1, 1, 0, chance, true);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut html = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            html = if ctx.player_level(player) >= MIN_LEVEL {
                "32166-01.htm"
            } else {
                "32166-02.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            html = if ctx.has_quest_items(player, MUERTOS_FEATHER) {
                "32166-05.html"
            } else {
                "32166-04.html"
            }
            .to_owned();
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00284_MuertosFeather";

    const NPCS: &str = r#"
        <npc id="32166" type="Folk" level="70" name="Trevor"/>
        <npc id="22239" type="Monster" level="30" name="Muertos Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9748" type="EtcItem" name="Muertos Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00284MuertosFeather);
        let pages = Pages::of(&Q00284MuertosFeather)
            .page("32166-01.htm", &["32166-03.htm"])
            .plain(&[
                "32166-02.htm",
                "32166-03.htm",
                "32166-04.html",
                "32166-06.html",
                "32166-07.html",
                "32166-09.html",
            ])
            .page("32166-05.html", &["32166-08.html"])
            .page("32166-08.html", &["32166-06.html", "32166-09.html"])
            .npcs(&[TREVOR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let trevor = t.spawn(TREVOR, 20);
        (t, trevor)
    }

    #[test]
    fn a_level_ten_character_is_sent_away() {
        let (mut t, trevor) = setup();
        t.enter(1, 10, vec![]);
        assert!(t.quest_page(1, trevor).contains("32166-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn feathers_are_bought_for_45_adena_each_then_the_quest_repeats() {
        let (mut t, trevor) = setup();
        t.enter(1, 11, vec![]);
        assert!(t.quest_page(1, trevor).contains("32166-01.htm"));
        assert!(t.event(1, NAME, "32166-03.htm").contains("32166-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, trevor).contains("32166-04.html"));

        // Feathers drop with a chance per kill and no limit; no condition is set.
        for _ in 0..200 {
            if t.count(1, MUERTOS_FEATHER) >= 3 {
                break;
            }
            let guard = t.spawn(22239, 60);
            t.kill(1, guard);
        }
        assert!(t.count(1, MUERTOS_FEATHER) >= 3);
        assert!(t.take_writes().is_empty());

        // Trevor buys them: count * 45 adena, all feathers taken.
        let feathers = t.count(1, MUERTOS_FEATHER);
        assert!(t.quest_page(1, trevor).contains("32166-05.html"));
        assert!(t.event(1, NAME, "32166-08.html").contains("32166-08.html"));
        assert_eq!(t.count(1, MUERTOS_FEATHER), 0);
        assert_eq!(t.count(1, 57), feathers * 45);
        assert!(t.take_writes().is_empty());

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32166-09.html").contains("32166-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
