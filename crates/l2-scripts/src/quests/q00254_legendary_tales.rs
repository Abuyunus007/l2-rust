//! Quest 254: Legendary Tales (Gilmore, the Watcher of Antharas, sends the characters of level 80
//! and up to the seven raid bosses of the Antharas lair for their Large Dragon Skulls, and trades
//! seven skulls for a Vesper weapon).
// Java: dist/game/data/scripts/quests/Q00254_LegendaryTales/Q00254_LegendaryTales.java
//
// Pattern: a kill count where each boss counts once (a bit mask of the bosses in the `raids`
// variable), the party kill through `execute_for_each_player` and a reward choice (the Vesper
// events).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GILMORE: i32 = 30754;

// Monsters: the seven raid bosses, the bit of each one in the `raids` variable is `1 << index`
const BOSSES: [i32; 7] = [25718, 25719, 25720, 25721, 25722, 25723, 25724];

// Items
const LARGE_DRAGON_SKULL: i32 = 17249;

// The Vesper weapons (the reward choice)
const VESPERS: [i32; 11] = [
    13467, 13466, 13465, 13464, 13463, 13457, 13458, 13459, 13460, 13461, 13462,
];

// Misc
const MIN_LEVEL: i32 = 80;
const SKULLS_NEEDED: i64 = 7;

/// Java `Bosses.getMask()` of a boss id, `None` for another NPC.
fn boss_mask(npc_id: i32) -> Option<i32> {
    BOSSES
        .iter()
        .position(|boss| *boss == npc_id)
        .map(|index| 1 << index)
}

/// Java `checkMask(qs, boss)`: the boss is in the `raids` variable.
fn check_mask(ctx: &ScriptCtx<'_>, qs: l2_world::script::Qs, mask: i32) -> bool {
    (qs.get_int(ctx, "raids") & mask) == mask
}

/// Java `Q00254_LegendaryTales`.
pub struct Q00254LegendaryTales;

impl Script for Q00254LegendaryTales {
    fn name(&self) -> &'static str {
        "Q00254_LegendaryTales"
    }

    fn path(&self) -> &'static str {
        "quests/Q00254_LegendaryTales"
    }

    fn id(&self) -> i32 {
        254
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GILMORE]);
        s.add_talk_id(&[GILMORE]);
        s.add_kill_id(&BOSSES);
        s.register_quest_items(&[LARGE_DRAGON_SKULL]);
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
        let mut htmltext = ctx.get_no_quest_msg(player);
        match event {
            "30754-02.html" | "30754-03.html" | "30754-04.htm" | "30754-08.html" | "30754-15.html"
            | "30754-20.html" | "30754-21.html" => htmltext = event.to_owned(),
            "30754-05.html" => {
                qs.start_quest(ctx);
                htmltext = event.to_owned();
            }
            // NOTE(java-quirk): the boss and Vesper links are `bypass -h <id>` (no `Script`
            // prefix), which no bypass handler routes to a script (`RequestBypassToServer`
            // reports them as not handled). The arms are kept as Java has them.
            "25718" | "25719" | "25720" | "25721" | "25722" | "25723" | "25724" => {
                let index = BOSSES
                    .iter()
                    .position(|boss| boss.to_string() == event)
                    .unwrap_or_default();
                let mask = 1 << index;
                let (done, not_done) = match index {
                    0 => ("30754-22.html", "30754-16.html"),
                    1 => ("30754-23.html", "30754-17.html"),
                    2 => ("30754-24.html", "30754-18.html"),
                    3 => ("30754-25.html", "30754-19.html"),
                    4 => ("30754-26.html", "30754-16.html"),
                    5 => ("30754-27.html", "30754-17.html"),
                    _ => ("30754-28.html", "30754-18.html"),
                };
                htmltext = if check_mask(ctx, qs, mask) { done } else { not_done }.to_owned();
            }
            _ => {
                if let Ok(vesper) = event.parse::<i32>()
                    && VESPERS.contains(&vesper)
                    && qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, LARGE_DRAGON_SKULL) >= SKULLS_NEEDED
                {
                    htmltext = "30754-09.html".to_owned();
                    ctx.reward_items(player, vesper, 1);
                    qs.exit_quest(ctx, false, true);
                }
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Every party member in the same place as the boss gets its own chance of the skull.
        ctx.execute_for_each_player(killer, npc, false, true, false);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if qs.is_cond(ctx, 1) {
            let raids = qs.get_int(ctx, "raids");
            let Some(mask) = boss_mask(npc.id) else {
                return;
            };
            if !check_mask(ctx, qs, mask) {
                qs.set_int(ctx, "raids", raids | mask);
                ctx.give_items(player, LARGE_DRAGON_SKULL, 1);
                if ctx.get_quest_items_count(player, LARGE_DRAGON_SKULL) < SKULLS_NEEDED {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                } else {
                    qs.set_cond(ctx, 2, true);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30754-00.htm"
                } else {
                    "30754-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                let count = ctx.get_quest_items_count(player, LARGE_DRAGON_SKULL);
                if qs.is_cond(ctx, 1) {
                    htmltext = if count > 0 {
                        "30754-14.htm"
                    } else {
                        "30754-06.html"
                    }
                    .to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = if count < SKULLS_NEEDED {
                        "30754-12.htm"
                    } else {
                        "30754-07.html"
                    }
                    .to_owned();
                }
            }
            COMPLETED => htmltext = "30754-29.html".to_owned(),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00254_LegendaryTales";

    const NPCS: &str = r#"
        <npc id="30754" type="Folk" level="80" name="Gilmore"/>
        <npc id="25718" type="RaidBoss" level="80" name="Emerald Horn"/>
        <npc id="25719" type="RaidBoss" level="80" name="Dust Rider"/>
        <npc id="25720" type="RaidBoss" level="80" name="Bleeding Fly"/>
        <npc id="25721" type="RaidBoss" level="80" name="Black Dagger Wing"/>
        <npc id="25722" type="RaidBoss" level="80" name="Shadow Summoner"/>
        <npc id="25723" type="RaidBoss" level="80" name="Spike Slasher"/>
        <npc id="25724" type="RaidBoss" level="80" name="Muscle Bomber"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17249" type="EtcItem" name="Large Dragon Skull"><set name="is_questitem" val="true"/></item>
        <item id="13467" type="Weapon" name="Vesper Thrower"><set name="weapon_type" val="dagger"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00254LegendaryTales);
        let pages = Pages::of(&Q00254LegendaryTales)
            .page("30754-01.htm", &["30754-02.html"])
            .page("30754-02.html", &["30754-03.html"])
            .page("30754-03.html", &["30754-04.htm"])
            .page("30754-04.htm", &["30754-05.html"])
            .plain(&["30754-00.htm", "30754-05.html", "30754-06.html", "30754-07.html"])
            .plain(&["30754-09.html", "30754-12.htm", "30754-14.htm", "30754-29.html"])
            .npcs(&[GILMORE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gilmore = t.spawn(GILMORE, 20);
        (t, gilmore)
    }

    fn raids(value: &str, first: bool) -> QuestWrite {
        if first {
            rows::insert(NAME, "raids", value)
        } else {
            rows::update(NAME, "raids", value)
        }
    }

    #[test]
    fn only_characters_of_level_80_are_offered_the_quest() {
        let (mut t, gilmore) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, gilmore).contains("30754-00.htm"));
        t.enter(2, 80, vec![]);
        assert!(t.quest_page(2, gilmore).contains("30754-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_seven_bosses_bring_the_skulls_and_a_vesper_is_the_reward() {
        let (mut t, gilmore) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, gilmore).contains("30754-01.htm"));
        for event in ["30754-02.html", "30754-03.html", "30754-04.htm"] {
            assert!(t.event(1, NAME, event).contains(event));
        }
        assert!(t.event(1, NAME, "30754-05.html").contains("30754-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, gilmore).contains("30754-06.html"));

        // Each boss counts once: a second kill of the Emerald Horn gives nothing.
        let mut writes = Vec::new();
        for (i, boss) in BOSSES.iter().enumerate() {
            let mob = t.spawn(*boss, 20);
            t.kill(1, mob);
            writes.extend(t.take_writes());
            if i == 0 {
                let again = t.spawn(*boss, 20);
                t.kill(1, again);
                assert!(t.take_writes().is_empty());
            }
        }
        assert_eq!(t.count(1, LARGE_DRAGON_SKULL), 7);
        // The `raids` mask after each kill, then cond 2 with the seventh skull.
        let masks = ["1", "3", "7", "15", "31", "63", "127"];
        let mut expected: Vec<QuestWrite> = masks.iter().enumerate().map(|(i, m)| raids(m, i == 0)).collect();
        expected.push(rows::update(NAME, "cond", "2"));
        assert_eq!(writes, expected);

        assert!(t.quest_page(1, gilmore).contains("30754-07.html"));
        assert!(t.take_writes().is_empty());

        // The Vesper reward is reached through the choice event (not a `Script` bypass, see the
        // NOTE above), so the test calls the event directly.
        t.world.run_as_script(NAME, |ctx| {
            ctx.notify_event("13467", None, Some(Oid(1)));
        });
        assert_eq!(t.count(1, 13467), 1);
        assert_eq!(t.count(1, LARGE_DRAGON_SKULL), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, gilmore).contains("30754-29.html"));
    }
}
