//! Quest 276: Totem of the Hestui (Tanapi in the Orc Village sends the orcs after the kasha bears and their totem).
// Java: dist/game/data/scripts/quests/Q00276_TotemOfTheHestui/Q00276_TotemOfTheHestui.java
//
// Pattern: a spawn chain (`docs/design/scripts.md`, "Quest patterns"): a kasha bear drops parasites,
// and with a chance that grows with the parasites in the bag a totem spawns at the killer (the
// parasites go). Killing the totem gives the crystal and cond 2; Tanapi pays two rewards and ends
// the quest. Killing the bears is the chance roll; the Newbie Guide's mission hears about it.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TANAPI: i32 = 30571;

// Items
const KASHA_PARASITE: i32 = 1480;
const KASHA_CRYSTAL: i32 = 1481;

// Monsters
const KASHA_BEAR: i32 = 20479;
const KASHA_BEAR_TOTEM: i32 = 27044;

// Rewards
const REWARDS: [i32; 2] = [29, 1500];

/// Java `SPAWN_CHANCES`: `(parasites needed, chance out of 100)`, tried in order.
const SPAWN_CHANCES: [(i64, i32); 5] = [(79, 100), (69, 20), (59, 15), (49, 10), (39, 2)];

// Misc
const MIN_LEVEL: i32 = 15;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00276_TotemOfTheHestui`.
pub struct Q00276TotemOfTheHestui;

impl Script for Q00276TotemOfTheHestui {
    fn name(&self) -> &'static str {
        "Q00276_TotemOfTheHestui"
    }

    fn path(&self) -> &'static str {
        "quests/Q00276_TotemOfTheHestui"
    }

    fn id(&self) -> i32 {
        276
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TANAPI]);
        s.add_talk_id(&[TANAPI]);
        s.add_kill_id(&[KASHA_BEAR, KASHA_BEAR_TOTEM]);
        s.register_quest_items(&[KASHA_PARASITE, KASHA_CRYSTAL]);
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
        if event == "30571-03.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        let Some(qs) = qs else {
            return;
        };
        if !qs.is_cond(ctx, 1) || !ctx.check_if_in_range(ctx.alt_party_range(), killer, npc.object_id, true) {
            return;
        }
        match npc.id {
            KASHA_BEAR => {
                let chance1 = ctx.get_quest_items_count(killer, KASHA_PARASITE);
                let chance2 = ctx.get_random(100);
                let mut chance3 = true;
                for (needed, chance) in SPAWN_CHANCES {
                    if chance1 >= needed && chance2 <= chance {
                        ctx.add_spawn_at(KASHA_BEAR_TOTEM, killer, false, 0);
                        ctx.take_items(killer, KASHA_PARASITE, -1);
                        chance3 = false;
                        break;
                    }
                }
                if chance3 {
                    // Java's 6-argument overload: one parasite, no limit, always dropped.
                    ctx.give_item_randomly(killer, None, KASHA_PARASITE, 1, 1, 0, 1.0, true);
                }
            }
            KASHA_BEAR_TOTEM => {
                if ctx.give_item_randomly(killer, None, KASHA_CRYSTAL, 1, 1, 1, 1.0, true) {
                    // `setCond(2)`: no sound of its own.
                    qs.set_cond(ctx, 2, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ORC {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30571-02.htm"
                    } else {
                        "30571-01.htm"
                    }
                } else {
                    "30571-00.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30571-04.html".to_owned(),
                2 => {
                    if ctx.has_quest_items(player, KASHA_CRYSTAL) {
                        // Newbie Guide: only when `ai/others/NewbieGuide` is installed.
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 100_000_000) / 10_000_000
                                != 1
                            {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 10_000_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                            guide.set_state(ctx, COMPLETED);
                        }

                        for reward in REWARDS {
                            ctx.reward_items(player, reward, 1);
                        }
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30571-05.html".to_owned();
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
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00276_TotemOfTheHestui";

    const NPCS: &str = r#"
        <npc id="30571" type="Folk" level="70" name="Tanapi"/>
        <npc id="20479" type="Monster" level="30" name="Kasha Bear"/>
        <npc id="27044" type="Monster" level="30" name="Kasha Bear Totem"/>"#;

    const ITEMS: &str = r#"
        <item id="29" type="EtcItem" name="Reward A"/>
        <item id="1500" type="EtcItem" name="Reward B"/>
        <item id="1480" type="EtcItem" name="Kasha Parasite"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1481" type="EtcItem" name="Kasha Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup(with_guide: bool) -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        if with_guide {
            registry.add(NewbieGuide);
        }
        registry.add(Q00276TotemOfTheHestui);
        let pages = Pages::of(&Q00276TotemOfTheHestui)
            .page("30571-02.htm", &["30571-03.htm"])
            .plain(&[
                "30571-00.htm",
                "30571-01.htm",
                "30571-03.htm",
                "30571-04.html",
                "30571-05.html",
            ])
            .npcs(&[TANAPI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tanapi = t.spawn(TANAPI, 20);
        (t, tanapi)
    }

    #[test]
    fn only_orcs_of_level_fifteen_can_take_it() {
        let (mut t, tanapi) = setup(false);
        t.enter_as(1, 15, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, tanapi).contains("30571-00.htm"));
        t.enter_as(2, 14, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, tanapi).contains("30571-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_totem_from_the_bears_then_the_crystal_then_the_rewards() {
        let (mut t, tanapi) = setup(true);
        t.enter_as(1, 15, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, tanapi).contains("30571-02.htm"));
        assert!(t.event(1, NAME, "30571-03.htm").contains("30571-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, tanapi).contains("30571-04.html"));

        // Bears drop parasites; with enough of them a totem spawns at the killer and the
        // parasites go.
        let mut kills = 0;
        while t.npcs_of(KASHA_BEAR_TOTEM).is_empty() {
            kills += 1;
            assert!(kills < 400, "no totem after {kills} bears");
            let bear = t.spawn(KASHA_BEAR, 60);
            t.kill(1, bear);
        }
        assert!(t.count(1, KASHA_PARASITE) < 79);
        assert!(t.take_writes().is_empty());

        // The totem gives the crystal and cond 2 (`setCond(2)`).
        let totem = t.npcs_of(KASHA_BEAR_TOTEM)[0];
        t.kill(1, totem);
        assert_eq!(t.count(1, KASHA_CRYSTAL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Tanapi: the Newbie Guide's mission, the two rewards, `exitQuest(true, true)`.
        assert!(t.quest_page(1, tanapi).contains("30571-05.html"));
        assert_eq!((t.count(1, 29), t.count(1, 1500)), (1, 1));
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "100000"),
            ]
        );
        assert!(writes.contains(&rows::delete(NAME, true)));
    }
}
