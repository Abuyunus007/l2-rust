//! Quest 10291: Fire Dragon Destroyer (Klein in the Valakas lair, Valakas).
// Java: dist/game/data/scripts/quests/Q10291_FireDragonDestroyer/Q10291_FireDragonDestroyer.java
//
// Pattern: a party kill (`docs/design/scripts.md`, "Kills, drops and parties"), with the reward
// check of the whole command channel (or party) on each kill. The NPC is the start of the quest
// and gives the reward at the end (one time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEIN: i32 = 31540;

// Monster
const VALAKAS: i32 = 29028;

// Items
const FLOATING_STONE: i32 = 7267;
const POOR_NECKLACE: i32 = 15524;
const VALOR_NECKLACE: i32 = 15525;

// Reward
const VALAKAS_SLAYER_CIRCLET: i32 = 8567;

/// Java `Q10291_FireDragonDestroyer`.
pub struct Q10291FireDragonDestroyer;

impl Script for Q10291FireDragonDestroyer {
    fn name(&self) -> &'static str {
        "Q10291_FireDragonDestroyer"
    }

    fn path(&self) -> &'static str {
        "quests/Q10291_FireDragonDestroyer"
    }

    fn id(&self) -> i32 {
        10291
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEIN]);
        s.add_talk_id(&[KLEIN]);
        s.add_kill_id(&[VALAKAS]);
        s.register_quest_items(&[POOR_NECKLACE, VALOR_NECKLACE]);
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
        if event == "31540-05.htm" {
            qs.start_quest(ctx);
            ctx.give_items(player, POOR_NECKLACE, 1);
        }
        // NOTE(java-quirk): every event is shown back as a page, also one this quest does not
        // know.
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if !ctx.is_in_party(killer) {
            return;
        }
        // Rewards go only to command channel, not to a single party or player.
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
                // Java: `getLevel() < 83` (no constant in Java).
                htmltext = if ctx.player_level(player) < 83 {
                    "31540-00.htm"
                } else if ctx.has_quest_items(player, FLOATING_STONE) {
                    "31540-02.htm"
                } else {
                    "31540-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, POOR_NECKLACE) {
                        htmltext = "31540-06.html".to_owned();
                    } else {
                        ctx.give_items(player, POOR_NECKLACE, 1);
                        htmltext = "31540-07.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, VALOR_NECKLACE) {
                    htmltext = "31540-08.html".to_owned();
                    ctx.give_adena(player, 126_549, true);
                    ctx.add_exp_and_sp(player, 717_291, 77_397);
                    ctx.give_items(player, VALAKAS_SLAYER_CIRCLET, 1);
                    qs.exit_quest(ctx, false, true);
                }
            }
            COMPLETED => htmltext = "31540-09.html".to_owned(),
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
            && ctx.has_quest_items(killer, POOR_NECKLACE)
        {
            ctx.take_items(killer, POOR_NECKLACE, -1);
            ctx.give_items(killer, VALOR_NECKLACE, 1);
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

    const NAME: &str = "Q10291_FireDragonDestroyer";

    const NPCS: &str = r#"
        <npc id="31540" type="Folk" level="80" name="Klein"/>
        <npc id="29028" type="RaidBoss" level="80" name="Valakas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7267" type="EtcItem" name="Floating Stone"><set name="is_stackable" val="true"/></item>
        <item id="15524" type="EtcItem" name="Poor Necklace"><set name="is_questitem" val="true"/></item>
        <item id="15525" type="EtcItem" name="Valor Necklace"><set name="is_questitem" val="true"/></item>
        <item id="8567" type="Armor" name="Valakas Slayer Circlet"><set name="bodypart" val="head"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10291FireDragonDestroyer);
        let pages = Pages::of(&Q10291FireDragonDestroyer)
            .page("31540-02.htm", &["31540-03.html"])
            .page("31540-03.html", &["31540-04.htm"])
            .page("31540-04.htm", &["31540-05.htm"])
            .plain(&[
                "31540-00.htm",
                "31540-01.htm",
                "31540-05.htm",
                "31540-06.html",
                "31540-07.html",
                "31540-08.html",
                "31540-09.html",
            ])
            .npcs(&[KLEIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klein = t.spawn(KLEIN, 20);
        (t, klein)
    }

    #[test]
    fn a_character_under_83_is_sent_away() {
        let (mut t, klein) = setup();
        t.enter(1, 82, vec![item(100, FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_floating_stone_decides_the_first_page() {
        let (mut t, klein) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, klein).contains("31540-01.htm"));
        t.enter(2, 83, vec![item(101, FLOATING_STONE, 1)]);
        assert!(t.quest_page(2, klein).contains("31540-02.htm"));
    }

    #[test]
    fn start_the_party_kill_then_the_circlet() {
        let (mut t, klein) = setup();
        t.enter(1, 83, vec![item(100, FLOATING_STONE, 1)]);
        t.enter(2, 60, vec![]);
        assert!(t.quest_page(1, klein).contains("31540-02.htm"));
        assert!(t.event(1, NAME, "31540-03.html").contains("31540-03.html"));
        assert!(t.event(1, NAME, "31540-04.htm").contains("31540-04.htm"));
        assert!(t.event(1, NAME, "31540-05.htm").contains("31540-05.htm"));
        assert_eq!(t.count(1, POOR_NECKLACE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-06.html"));

        // Member 2 is in the party; a kill far away gives no necklace.
        t.party(1, 2);
        let far = t
            .world
            .spawn_npc_at(
                VALAKAS,
                crate::testing::X + 9000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, VALOR_NECKLACE), 0);
        assert_eq!(t.count(1, POOR_NECKLACE), 1);

        // Valakas near the group: the killer's necklace turns, the killer's quest moves to cond 2.
        let valakas = t.spawn(VALAKAS, 60);
        t.kill(1, valakas);
        assert_eq!(t.count(1, VALOR_NECKLACE), 1);
        assert_eq!(t.count(1, POOR_NECKLACE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The circlet, the adena and the quest's end.
        assert!(t.quest_page(1, klein).contains("31540-08.html"));
        assert_eq!(t.count(1, 57), 126_549);
        assert_eq!(t.count(1, VALAKAS_SLAYER_CIRCLET), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-09.html"));
    }
}
