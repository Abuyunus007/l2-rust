//! Quest 286: Fabulous Feathers (Erinu in the Hunters Village sends the player after the Shady Muertos).
// Java: dist/game/data/scripts/quests/Q00286_FabulousFeathers/Q00286_FabulousFeathers.java
//
// Pattern: a party kill quest (`docs/design/scripts.md`, "Quest patterns"): a Shady Muertos
// drops a commander's feather for a random party member in cond 1, the 80th feather (limit)
// sets that member to cond 2; the NPC pays adena and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ERINU: i32 = 32164;

// Item
const COMMANDERS_FEATHER: i32 = 9746;
const COMMANDERS_FEATHER_COUNT: i64 = 80;

/// Java `MOB_DROP_CHANCES`: the Shady Muertos and their drop chance.
const MOB_DROP_CHANCES: [(i32, f64); 5] = [
    (22251, 0.748), // Shady Muertos Captain
    (22253, 0.772), // Shady Muertos Warrior
    (22254, 0.772), // Shady Muertos Archer
    (22255, 0.796), // Shady Muertos Commander
    (22256, 0.952), // Shady Muertos Wizard
];

// Misc
const MIN_LEVEL: i32 = 17;

/// Java `Q00286_FabulousFeathers`.
pub struct Q00286FabulousFeathers;

impl Script for Q00286FabulousFeathers {
    fn name(&self) -> &'static str {
        "Q00286_FabulousFeathers"
    }

    fn path(&self) -> &'static str {
        "quests/Q00286_FabulousFeathers"
    }

    fn id(&self) -> i32 {
        286
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ERINU]);
        s.add_talk_id(&[ERINU]);
        let ids: Vec<i32> = MOB_DROP_CHANCES.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[COMMANDERS_FEATHER]);
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
            "32164-03.htm" => {
                qs.start_quest(ctx);
                html = Some(event.to_owned());
            }
            "32164-06.html" => {
                if qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, COMMANDERS_FEATHER) >= COMMANDERS_FEATHER_COUNT
                {
                    ctx.take_items(player, COMMANDERS_FEATHER, COMMANDERS_FEATHER_COUNT);
                    ctx.give_adena(player, 4160, true);
                    qs.exit_quest(ctx, true, true);
                    html = Some(event.to_owned());
                } else {
                    html = Some("32164-07.html".to_owned());
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        let chance = MOB_DROP_CHANCES
            .iter()
            .find(|&&(id, _)| id == npc.id)
            .map_or(0.0, |&(_, chance)| chance);
        if ctx.give_item_randomly(
            member.player(),
            Some(npc),
            COMMANDERS_FEATHER,
            1,
            1,
            COMMANDERS_FEATHER_COUNT,
            chance,
            true,
        ) {
            // `setCond(2)`: no sound of its own.
            member.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut html = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            html = if ctx.player_level(player) >= MIN_LEVEL {
                "32164-01.htm"
            } else {
                "32164-02.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            html = if qs.is_cond(ctx, 2)
                && ctx.get_quest_items_count(player, COMMANDERS_FEATHER) >= COMMANDERS_FEATHER_COUNT
            {
                "32164-04.html"
            } else {
                "32164-05.html"
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

    const NAME: &str = "Q00286_FabulousFeathers";

    const NPCS: &str = r#"
        <npc id="32164" type="Folk" level="70" name="Erinu"/>
        <npc id="22251" type="Monster" level="35" name="Shady Muertos Captain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9746" type="EtcItem" name="Commander's Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00286FabulousFeathers);
        let pages = Pages::of(&Q00286FabulousFeathers)
            .page("32164-01.htm", &["32164-03.htm"])
            .plain(&["32164-02.htm", "32164-03.htm", "32164-05.html", "32164-07.html"])
            .page("32164-04.html", &["32164-06.html"])
            .plain(&["32164-06.html"])
            .npcs(&[ERINU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let erinu = t.spawn(ERINU, 20);
        (t, erinu)
    }

    /// Kills Shady Muertos Captains until `feathers` feathers are in the inventory; returns the kills.
    fn hunt(t: &mut ScriptTest, feathers: i64) -> usize {
        for kills in 1..1000 {
            let captain = t.spawn(22251, 60);
            t.kill(1, captain);
            if t.count(1, COMMANDERS_FEATHER) >= feathers {
                return kills;
            }
        }
        panic!("no {feathers} feathers after 1000 kills");
    }

    #[test]
    fn a_level_sixteen_character_is_sent_away() {
        let (mut t, erinu) = setup();
        t.enter(1, 16, vec![]);
        assert!(t.quest_page(1, erinu).contains("32164-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn eighty_feathers_then_adena() {
        let (mut t, erinu) = setup();
        t.enter(1, 17, vec![]);
        assert!(t.quest_page(1, erinu).contains("32164-01.htm"));
        assert!(t.event(1, NAME, "32164-03.htm").contains("32164-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, erinu).contains("32164-05.html"));

        // The feathers come with a chance per kill; the 80th sets cond 2 for the member.
        let kills = hunt(&mut t, COMMANDERS_FEATHER_COUNT);
        assert!(kills >= 80, "{kills}");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Erinu takes the 80 feathers and pays 4160 adena; `exitQuest(true, true)`.
        assert!(t.quest_page(1, erinu).contains("32164-04.html"));
        assert!(t.event(1, NAME, "32164-06.html").contains("32164-06.html"));
        assert_eq!(t.count(1, COMMANDERS_FEATHER), 0);
        assert_eq!(t.count(1, 57), 4160);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
