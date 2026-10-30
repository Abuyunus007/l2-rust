//! Quest 10290: Land Dragon Conqueror (Theodric in the Antharas lair, Antharas).
// Java: dist/game/data/scripts/quests/Q10290_LandDragonConqueror/Q10290_LandDragonConqueror.java
//
// Pattern: a party kill (`docs/design/scripts.md`, "Kills, drops and parties"), with the reward
// check of the whole command channel (or party) on each kill. The NPC is the start of the quest
// and gives the reward at the end (one time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const THEODRIC: i32 = 30755;

// Monster
const ANTHARAS: i32 = 29068;

// Items
const PORTAL_STONE: i32 = 3865;
const SHABBY_NECKLACE: i32 = 15522;
const MIRACLE_NECKLACE: i32 = 15523;

// Reward
const ANTHARAS_SLAYER_CIRCLET: i32 = 8568;

// Misc
const MIN_LEVEL: i32 = 83;

/// Java `Q10290_LandDragonConqueror`.
pub struct Q10290LandDragonConqueror;

impl Script for Q10290LandDragonConqueror {
    fn name(&self) -> &'static str {
        "Q10290_LandDragonConqueror"
    }

    fn path(&self) -> &'static str {
        "quests/Q10290_LandDragonConqueror"
    }

    fn id(&self) -> i32 {
        10290
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[THEODRIC]);
        s.add_talk_id(&[THEODRIC]);
        s.add_kill_id(&[ANTHARAS]);
        s.register_quest_items(&[MIRACLE_NECKLACE, SHABBY_NECKLACE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        if event == "30755-05.htm" {
            qs.start_quest(ctx);
            ctx.give_items(player, SHABBY_NECKLACE, 1);
        }
        // NOTE(java-quirk): every event is shown back as a page, also one this quest does not
        // know.
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if !ctx.is_in_party(killer) {
            return;
        }
        // Rewards go only to command channel, not to a single party or player (retail Freya AI).
        let members = match ctx.world().command_channel_of(killer) {
            Some(channel) => ctx.world().command_channel_members(channel),
            None => ctx.world().party_members_of(killer),
        };
        for member in members {
            reward_check(ctx, npc, killer, member);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30755-00.htm"
                } else if ctx.has_quest_items(player, PORTAL_STONE) {
                    "30755-02.htm"
                } else {
                    "30755-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, SHABBY_NECKLACE) {
                        htmltext = "30755-06.html".to_owned();
                    } else {
                        ctx.give_items(player, SHABBY_NECKLACE, 1);
                        htmltext = "30755-07.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MIRACLE_NECKLACE) {
                    htmltext = "30755-08.html".to_owned();
                    ctx.give_adena(player, 131_236, true);
                    ctx.add_exp_and_sp(player, 702_557, 76_334);
                    ctx.give_items(player, ANTHARAS_SLAYER_CIRCLET, 1);
                    qs.exit_quest(ctx, false, true);
                }
            }
            COMPLETED => htmltext = "30755-09.html".to_owned(),
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `rewardCheck` (the lambda of `onKill`) for one member of the killer's group.
fn reward_check(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, member: ObjectId) {
    if ctx.check_if_in_range(8000, npc.object_id, member, false) {
        let qs = ctx.get_quest_state(member, false);
        // NOTE(java-quirk): Java tests, takes and gives the items of the killer (`player`), but
        // moves the quest state of the member (`p`).
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.has_quest_items(killer, SHABBY_NECKLACE)
        {
            ctx.take_items(killer, SHABBY_NECKLACE, -1);
            ctx.give_items(killer, MIRACLE_NECKLACE, 1);
            qs.set_cond(ctx, 2, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q10290_LandDragonConqueror";

    const NPCS: &str = r#"
        <npc id="30755" type="Folk" level="80" name="Theodric"/>
        <npc id="29068" type="RaidBoss" level="80" name="Antharas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3865" type="EtcItem" name="Portal Stone"><set name="is_stackable" val="true"/></item>
        <item id="15522" type="EtcItem" name="Shabby Necklace"><set name="is_questitem" val="true"/></item>
        <item id="15523" type="EtcItem" name="Miracle Necklace"><set name="is_questitem" val="true"/></item>
        <item id="8568" type="Armor" name="Antharas Slayer Circlet"><set name="bodypart" val="head"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10290LandDragonConqueror);
        let pages = Pages::of(&Q10290LandDragonConqueror)
            .page("30755-02.htm", &["30755-03.html"])
            .page("30755-03.html", &["30755-04.htm"])
            .page("30755-04.htm", &["30755-05.htm"])
            .plain(&[
                "30755-00.htm",
                "30755-01.htm",
                "30755-05.htm",
                "30755-06.html",
                "30755-07.html",
                "30755-08.html",
                "30755-09.html",
            ])
            .npcs(&[THEODRIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let theodric = t.spawn(THEODRIC, 20);
        (t, theodric)
    }

    #[test]
    fn a_character_under_83_is_sent_away() {
        let (mut t, theodric) = setup();
        t.enter(1, 82, vec![item(100, PORTAL_STONE, 1)]);
        assert!(t.quest_page(1, theodric).contains("30755-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_portal_stone_decides_the_first_page() {
        let (mut t, theodric) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, theodric).contains("30755-01.htm"));
        t.enter(2, 83, vec![item(101, PORTAL_STONE, 1)]);
        assert!(t.quest_page(2, theodric).contains("30755-02.htm"));
    }

    #[test]
    fn start_the_party_kill_then_the_circlet() {
        let (mut t, theodric) = setup();
        t.enter(1, 83, vec![item(100, PORTAL_STONE, 1)]);
        t.enter(2, 60, vec![]);
        assert!(t.quest_page(1, theodric).contains("30755-02.htm"));
        assert!(t.event(1, NAME, "30755-03.html").contains("30755-03.html"));
        assert!(t.event(1, NAME, "30755-04.htm").contains("30755-04.htm"));
        assert!(t.event(1, NAME, "30755-05.htm").contains("30755-05.htm"));
        assert_eq!(t.count(1, SHABBY_NECKLACE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-06.html"));

        // Member 2 is in the party; the killer is out of range of nothing: a kill far away
        // gives no necklace.
        t.party(1, 2);
        let far = t
            .world
            .spawn_npc_at(
                ANTHARAS,
                crate::testing::X + 9000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, MIRACLE_NECKLACE), 0);
        assert_eq!(t.count(1, SHABBY_NECKLACE), 1);

        // Antharas near the group: the killer's necklace turns, the killer's quest moves to cond 2.
        let antharas = t.spawn(ANTHARAS, 60);
        t.kill(1, antharas);
        assert_eq!(t.count(1, MIRACLE_NECKLACE), 1);
        assert_eq!(t.count(1, SHABBY_NECKLACE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The circlet, the adena and the quest's end.
        assert!(t.quest_page(1, theodric).contains("30755-08.html"));
        assert_eq!(t.count(1, 57), 131_236);
        assert_eq!(t.count(1, ANTHARAS_SLAYER_CIRCLET), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-09.html"));
    }
}
