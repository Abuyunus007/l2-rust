//! Quest 313: Collect Spores (Herbiel in Gludio, Spore Fungus).
// Java: dist/game/data/scripts/quests/Q00313_CollectSpores/Q00313_CollectSpores.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`,
// "Quest patterns"): `on_kill` gives the spore sacs with `give_item_randomly` and moves to cond 2
// at the limit; the NPC pays and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HERBIEL: i32 = 30150;

// Item
const SPORE_SAC: i32 = 1118;

// Misc
const MIN_LEVEL: i32 = 8;
const REQUIRED_SAC_COUNT: i64 = 10;

// Monster
const SPORE_FUNGUS: i32 = 20509;

/// Java `Q00313_CollectSpores`.
pub struct Q00313CollectSpores;

impl Script for Q00313CollectSpores {
    fn name(&self) -> &'static str {
        "Q00313_CollectSpores"
    }

    fn path(&self) -> &'static str {
        "quests/Q00313_CollectSpores"
    }

    fn id(&self) -> i32 {
        313
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HERBIEL]);
        s.add_talk_id(&[HERBIEL]);
        s.add_kill_id(&[SPORE_FUNGUS]);
        s.register_quest_items(&[SPORE_SAC]);
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
            "30150-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30150-04.htm" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
            && ctx.give_item_randomly(killer, Some(npc), SPORE_SAC, 1, 1, REQUIRED_SAC_COUNT, 0.4, true)
        {
            qs.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30150-03.htm"
            } else {
                "30150-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match qs.get_cond(ctx) {
                1 => {
                    if ctx.get_quest_items_count(player, SPORE_SAC) < REQUIRED_SAC_COUNT {
                        htmltext = "30150-06.html".to_owned();
                    }
                }
                2 => {
                    if ctx.get_quest_items_count(player, SPORE_SAC) >= REQUIRED_SAC_COUNT {
                        ctx.give_adena(player, 3500, true);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30150-07.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00313_CollectSpores";

    const NPCS: &str = r#"
        <npc id="30150" type="Folk" level="1" name="Herbiel"/>
        <npc id="20509" type="Monster" level="2" name="Spore Fungus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1118" type="EtcItem" name="Spore Sac"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00313CollectSpores);
        let pages = Pages::of(&Q00313CollectSpores)
            .page("30150-03.htm", &["30150-04.htm"])
            .page("30150-04.htm", &["30150-05.htm"])
            .plain(&["30150-02.htm", "30150-05.htm", "30150-06.html", "30150-07.html"])
            .npcs(&[HERBIEL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let herbiel = t.spawn(HERBIEL, 20);
        (t, herbiel)
    }

    /// Kills Spore Fungi until the quest is at cond 2; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let fungus = t.spawn(SPORE_FUNGUS, 60);
            t.kill(1, fungus);
            if t.count(1, SPORE_SAC) >= REQUIRED_SAC_COUNT {
                return kills;
            }
        }
        panic!("no 10 sacs after 200 kills");
    }

    #[test]
    fn a_level_seven_character_is_sent_away() {
        let (mut t, herbiel) = setup();
        t.enter(1, 7, vec![]);
        assert!(t.quest_page(1, herbiel).contains("30150-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_sacs_then_adena() {
        let (mut t, herbiel) = setup();
        t.enter(1, 8, vec![]);
        assert!(t.quest_page(1, herbiel).contains("30150-03.htm"));
        assert!(t.event(1, NAME, "30150-04.htm").contains("30150-04.htm"));
        assert!(t.event(1, NAME, "30150-05.htm").contains("30150-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Too few sacs: the NPC sends the player back to the hunt.
        assert!(t.quest_page(1, herbiel).contains("30150-06.html"));

        // A kill out of range of the killer gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                SPORE_FUNGUS,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, SPORE_SAC), 0);

        // The tenth sac sets cond 2 (`setCond(2)`: no sound of its own).
        let kills = hunt(&mut t);
        assert!(kills >= 10, "{kills}");
        assert_eq!(t.count(1, SPORE_SAC), REQUIRED_SAC_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: 3500 adena, the sacs go with the quest (`exitQuest` takes the quest
        // items), the quest ends repeatable.
        t.take();
        t.talk(1, herbiel);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30150-07.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), 3500);
        assert_eq!(t.count(1, SPORE_SAC), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
