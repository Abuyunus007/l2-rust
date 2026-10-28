//! Quest 275: Dark Winged Spies (Neruga Chief Tantus, Darkwing Bats and the Varangka's Tracker).
// Java: dist/game/data/scripts/quests/Q00275_DarkWingedSpies/Q00275_DarkWingedSpies.java
//
// Pattern: a quest that spawns a monster (`docs/design/scripts.md`, "Quest patterns"): a kill
// can call a tracker to the killer (`add_spawn_at(id, killer, false, 0)`: Java's
// `addSpawn(npcId, killer)`), which attacks when it sees a player (`on_creature_see` +
// `add_attack_desire`) and pays five fangs when killed. Orcs only (`race::ORC`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const NERUGA_CHIEF_TANTUS: i32 = 30567;

// Items
const DARKWING_BAT_FANG: i32 = 1478;
const VARANGKAS_PARASITE: i32 = 1479;

// Monsters
const DARKWING_BAT: i32 = 20316;
const VARANGKAS_TRACKER: i32 = 27043;

// Misc
const MIN_LEVEL: i32 = 11;
const FANG_PRICE: i64 = 60;
const MAX_BAT_FANG_COUNT: i64 = 70;

/// Java `Q00275_DarkWingedSpies`.
pub struct Q00275DarkWingedSpies;

impl Script for Q00275DarkWingedSpies {
    fn name(&self) -> &'static str {
        "Q00275_DarkWingedSpies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00275_DarkWingedSpies"
    }

    fn id(&self) -> i32 {
        275
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NERUGA_CHIEF_TANTUS]);
        s.add_talk_id(&[NERUGA_CHIEF_TANTUS]);
        s.add_kill_id(&[DARKWING_BAT, VARANGKAS_TRACKER]);
        s.add_creature_see_id(&[VARANGKAS_TRACKER]);
        s.register_quest_items(&[DARKWING_BAT_FANG, VARANGKAS_PARASITE]);
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
            && event == "30567-03.htm"
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
        {
            let count = ctx.get_quest_items_count(killer, DARKWING_BAT_FANG);

            match npc.id {
                DARKWING_BAT => {
                    if ctx.give_item_randomly(
                        killer,
                        None,
                        DARKWING_BAT_FANG,
                        1,
                        1,
                        MAX_BAT_FANG_COUNT,
                        1.0,
                        true,
                    ) {
                        qs.set_cond(ctx, 2, false);
                    } else if count > 10 && count < 66 && ctx.get_random(100) < 10 {
                        ctx.add_spawn_at(VARANGKAS_TRACKER, killer, false, 0);
                        ctx.give_items(killer, VARANGKAS_PARASITE, 1);
                    }
                }
                VARANGKAS_TRACKER => {
                    if count < 66 && ctx.has_quest_items(killer, VARANGKAS_PARASITE) {
                        if ctx.give_item_randomly(
                            killer,
                            None,
                            DARKWING_BAT_FANG,
                            5,
                            5,
                            MAX_BAT_FANG_COUNT,
                            1.0,
                            true,
                        ) {
                            qs.set_cond(ctx, 2, false);
                        }
                        ctx.take_items(killer, VARANGKAS_PARASITE, -1);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() {
            // `setRunning()`, `addDamageHate(creature, 0, 1)`, `setIntentionAttack(creature)`.
            ctx.add_attack_desire(npc, creature, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(talker) == race::ORC {
                    if ctx.player_level(talker) >= MIN_LEVEL {
                        "30567-02.htm"
                    } else {
                        "30567-01.htm"
                    }
                } else {
                    "30567-00.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30567-05.html".to_owned(),
                2 => {
                    let count = ctx.get_quest_items_count(talker, DARKWING_BAT_FANG);
                    if count >= MAX_BAT_FANG_COUNT {
                        ctx.give_adena(talker, count * FANG_PRICE, true);
                        qs.exit_quest(ctx, true, true);
                        // NOTE(java-quirk): the reward shows the "keep hunting" page 05; the
                        // reward page 04 is never used.
                        htmltext = "30567-05.html".to_owned();
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
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00275_DarkWingedSpies";

    const NPCS: &str = r#"
        <npc id="30567" type="Folk" level="70" name="Neruga Chief Tantus"/>
        <npc id="20316" type="Monster" level="12" name="Darkwing Bat"/>
        <npc id="27043" type="Monster" level="12" name="Varangka's Tracker"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1478" type="EtcItem" name="Darkwing Bat Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1479" type="EtcItem" name="Varangka's Parasite"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00275DarkWingedSpies);
        let pages = Pages::of(&Q00275DarkWingedSpies)
            .page("30567-02.htm", &["30567-03.htm"])
            .plain(&[
                "30567-00.htm",
                "30567-01.htm",
                "30567-03.htm",
                "30567-04.html",
                "30567-05.html",
            ])
            .npcs(&[NERUGA_CHIEF_TANTUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tantus = t.spawn(NERUGA_CHIEF_TANTUS, 20);
        (t, tantus)
    }

    #[test]
    fn only_orcs() {
        let (mut t, tantus) = setup();
        t.enter_as(1, 11, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, tantus).contains("30567-00.htm"));
        t.enter_as(2, 10, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, tantus).contains("30567-01.htm"));
    }

    #[test]
    fn bats_call_trackers_and_seventy_fangs_pay() {
        let (mut t, tantus) = setup();
        t.enter_as(1, 11, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, tantus).contains("30567-02.htm"));
        t.event(1, NAME, "30567-03.htm");
        t.take_writes();

        let mut trackers = 0;
        while t.count(1, DARKWING_BAT_FANG) < MAX_BAT_FANG_COUNT {
            let bat = t.spawn(DARKWING_BAT, 60);
            t.kill(1, bat);
            // A tracker came to the killer with its parasite: kill it for five fangs.
            if let Some(&tracker) = t
                .npcs_of(VARANGKAS_TRACKER)
                .iter()
                .find(|id| !t.world.is_dead(**id))
            {
                trackers += 1;
                assert_eq!(t.count(1, VARANGKAS_PARASITE), 1);
                let before = t.count(1, DARKWING_BAT_FANG);
                t.kill(1, tracker);
                assert_eq!(t.count(1, VARANGKAS_PARASITE), 0);
                assert_eq!(
                    t.count(1, DARKWING_BAT_FANG),
                    (before + 5).min(MAX_BAT_FANG_COUNT)
                );
            }
        }
        assert!(trackers > 0, "seed 7 should call at least one tracker");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, tantus).contains("30567-05.html"));
        assert_eq!(t.count(1, 57), MAX_BAT_FANG_COUNT * FANG_PRICE);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_tracker_attacks_the_player_it_sees() {
        let (mut t, _tantus) = setup();
        t.enter_as(1, 11, race::ORC, player_class::ORC_FIGHTER, vec![]);
        let tracker = t.spawn(VARANGKAS_TRACKER, 60);
        let npc = ScriptNpc {
            object_id: tracker,
            id: VARANGKAS_TRACKER,
        };
        // The hook itself (the world fires it once a second from `CreatureSeeTaskManager`).
        t.world.run_as_script(NAME, |ctx| {
            Q00275DarkWingedSpies.on_creature_see(ctx, npc, Oid(1));
        });
        assert!(t.world.aggro_list(tracker).iter().any(|a| a.attacker == Oid(1)));
    }
}
