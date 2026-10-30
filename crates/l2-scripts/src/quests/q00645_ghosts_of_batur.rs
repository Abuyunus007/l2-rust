//! Quest 645: Ghosts of Batur (Karuda in the Ruins of Batur; the contaminated Morek and Batur
//! warriors drop the cursed burial items, 500 of them end the hunt).
// Java: dist/game/data/scripts/quests/Q00645_GhostsOfBatur/Q00645_GhostsOfBatur.java
//
// Pattern: a party kill count (the drop goes to a random member at cond 1), a per-monster drop
// chance per mille, the cond 2 at the limit, and the old reward for the old grave goods.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KARUDA: i32 = 32017;

// Monsters
const CONTAMINATED_MOREK_WARRIOR: i32 = 22703;
const CONTAMINATED_BATUR_WARRIOR: i32 = 22704;
const CONTAMINATED_BATUR_COMMANDER: i32 = 22705;

// Items
const CURSED_GRAVE_GOODS: i32 = 8089; // Old item
const CURSED_BURIAL_ITEMS: i32 = 14861; // New item

// Misc
const MIN_LEVEL: i32 = 80;
/// Java `CHANCES`, per mille, indexed by `npc.getId() - CONTAMINATED_MOREK_WARRIOR`.
const CHANCES: [i32; 3] = [516, 664, 686];

/// Java `Q00645_GhostsOfBatur`.
pub struct Q00645GhostsOfBatur;

impl Script for Q00645GhostsOfBatur {
    fn name(&self) -> &'static str {
        "Q00645_GhostsOfBatur"
    }

    fn path(&self) -> &'static str {
        "quests/Q00645_GhostsOfBatur"
    }

    fn id(&self) -> i32 {
        645
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KARUDA]);
        s.add_talk_id(&[KARUDA]);
        s.add_kill_id(&[
            CONTAMINATED_MOREK_WARRIOR,
            CONTAMINATED_BATUR_WARRIOR,
            CONTAMINATED_BATUR_COMMANDER,
        ]);
        s.register_quest_items(&[CURSED_GRAVE_GOODS]);
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
        // Java: every event needs the level, not only the start.
        if ctx.player_level(player) >= MIN_LEVEL {
            match event {
                "32017-03.htm" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                "32017-06.html" | "32017-08.html" => htmltext = Some(event.to_owned()),
                "32017-09.html" => {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(player) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
            return;
        }
        let Some(&chance) =
            CHANCES.get(usize::try_from(npc.id - CONTAMINATED_MOREK_WARRIOR).unwrap_or(usize::MAX))
        else {
            return;
        };
        if ctx.get_random(1000) >= chance {
            return;
        }
        let qs = ctx.get_quest_state(player, false);
        ctx.give_items(killer, CURSED_BURIAL_ITEMS, 1);
        // NOTE(java-quirk): Java calls `qs.isCond(1)` on the member's state without a check; a
        // member without one would throw there, after the item is given. Here the kill stops.
        let Some(qs) = qs else {
            return;
        };
        if qs.is_cond(ctx, 1) && ctx.get_quest_items_count(killer, CURSED_BURIAL_ITEMS) >= 500 {
            qs.set_cond(ctx, 2, true);
        } else {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32017-01.htm".to_owned()
                } else {
                    "32017-02.html".to_owned()
                };
            }
            STARTED => {
                // Support for old quest reward.
                let count = ctx.get_quest_items_count(player, CURSED_GRAVE_GOODS);
                if count > 0 && count < 180 {
                    ctx.give_adena(player, 56_000 + count * 64, false);
                    ctx.add_exp_and_sp(player, 138_000, 7997);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "32017-07.html".to_owned();
                } else {
                    htmltext = if ctx.has_quest_items(player, CURSED_BURIAL_ITEMS) {
                        "32017-04.html".to_owned()
                    } else {
                        "32017-05.html".to_owned()
                    };
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

    const NAME: &str = "Q00645_GhostsOfBatur";

    const NPCS: &str = r#"
        <npc id="32017" type="Folk" level="80" name="Karuda"/>
        <npc id="22703" type="Monster" level="80" name="Contaminated Morek Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8089" type="EtcItem" name="Cursed Grave Goods"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14861" type="EtcItem" name="Cursed Burial Items"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00645GhostsOfBatur);
        let pages = Pages::of(&Q00645GhostsOfBatur)
            .page("32017-01.htm", &["32017-03.htm"])
            .page("32017-03.htm", &["32017-06.html", "32017-08.html"])
            .page("32017-04.html", &["32017-08.html", "32017-09.html"])
            .page(
                "32017-05.html",
                &["32017-06.html", "32017-08.html", "32017-09.html"],
            )
            .plain(&[
                "32017-02.html",
                "32017-06.html",
                "32017-07.html",
                "32017-08.html",
                "32017-09.html",
            ])
            .npcs(&[KARUDA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let karuda = t.spawn(KARUDA, 20);
        (t, karuda)
    }

    /// A contaminated Morek Warrior dies next to the player and the kill delay passes.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(22703, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_80_is_sent_away() {
        let (mut t, karuda) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, karuda).contains("32017-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_burial_items_end_the_hunt_at_500() {
        let (mut t, karuda) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, karuda).contains("32017-01.htm"));
        assert!(t.event(1, NAME, "32017-03.htm").contains("32017-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, karuda).contains("32017-05.html"));

        // Almost 500 items already: the next drop makes cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), CURSED_BURIAL_ITEMS, 499);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, CURSED_BURIAL_ITEMS) < 500 {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 100, "no burial items dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The burial items are the proof: the page offers the exit.
        assert!(t.quest_page(1, karuda).contains("32017-04.html"));
        assert!(t.event(1, NAME, "32017-09.html").contains("32017-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_old_grave_goods_are_paid_out_and_the_quest_ends() {
        let (mut t, karuda) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, karuda);
        t.event(1, NAME, "32017-03.htm");
        t.take_writes();
        // 10 old goods: 56000 + 10 * 64 adena, 138000 exp, 7997 sp, and the quest ends.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), CURSED_GRAVE_GOODS, 10);
        });
        t.take();
        assert!(t.quest_page(1, karuda).contains("32017-07.html"));
        assert_eq!(t.count(1, 57), 56_000 + 10 * 64);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn some_burial_items_show_the_page_with_the_exit() {
        let (mut t, karuda) = setup();
        t.enter(1, 80, vec![item(100, 14861, 3)]);
        t.quest_page(1, karuda);
        t.event(1, NAME, "32017-03.htm");
        t.take_writes();
        // Any burial item (not 500 of them) shows page 04 (`hasQuestItems`).
        assert!(t.quest_page(1, karuda).contains("32017-04.html"));
        assert!(t.take_writes().is_empty());
    }
}
