//! Quest 128: Pailaka: Song of Ice and Fire (Adler at the Pailaka entrance, Sinai, the inspector).
// Java: dist/game/data/scripts/quests/Q00128_PailakaSongOfIceAndFire/Q00128_PailakaSongOfIceAndFire.java
//
// Pattern: a chain of monster kills, each kill moves the cond and spawns the next monster at a
// fixed place (`add_spawn` into the NPC's own instance, `instance_id_of`). The last NPC closes
// the instance (`set_instance_duration`, `set_instance_empty_destroy_time`) and rewards only the
// players inside it (`instance_world(..).contains_player`). The instance entry (`-h Script
// PailakaSongOfIceAndFire enter`) belongs to the instance script (`instances/`), not here.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const ADLER1: i32 = 32497;
const ADLER2: i32 = 32510;
const SINAI: i32 = 32500;
const INSPECTOR: i32 = 32507;

// Monsters
const HILLAS: i32 = 18610;
const PAPION: i32 = 18609;
const KINSUS: i32 = 18608;
const GARGOS: i32 = 18607;
const ADIANTUM: i32 = 18620;

// Items
const SWORD: i32 = 13034;
const ENH_SWORD1: i32 = 13035;
const ENH_SWORD2: i32 = 13036;
const BOOK1: i32 = 13130;
const BOOK2: i32 = 13131;
const BOOK3: i32 = 13132;
const BOOK4: i32 = 13133;
const BOOK5: i32 = 13134;
const BOOK6: i32 = 13135;
const BOOK7: i32 = 13136;
const WATER_ESSENCE: i32 = 13038;
const FIRE_ESSENCE: i32 = 13039;
const SHIELD_POTION: i32 = 13032;
const HEAL_POTION: i32 = 13033;
const FIRE_ENHANCER: i32 = 13040;
const WATER_ENHANCER: i32 = 13041;

/// Java `REWARDS`: Pailaka Ring, Pailaka Earring, Scroll of Escape.
const REWARDS: [i32; 3] = [13294, 13293, 736];

/// Java `VITALITY_REPLENISHING` (`SkillHolder(5774, 1)`).
const VITALITY_REPLENISHING: i32 = 5774;

// Misc
const MIN_LEVEL: i32 = 36;
const MAX_LEVEL: i32 = 42;
/// Minutes the instance stays open after the reward (Java `EXIT_TIME`).
const EXIT_TIME: i32 = 5;

/// Java `Q00128_PailakaSongOfIceAndFire`.
pub struct Q00128PailakaSongOfIceAndFire;

impl Script for Q00128PailakaSongOfIceAndFire {
    fn name(&self) -> &'static str {
        "Q00128_PailakaSongOfIceAndFire"
    }

    fn path(&self) -> &'static str {
        "quests/Q00128_PailakaSongOfIceAndFire"
    }

    fn id(&self) -> i32 {
        128
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ADLER1]);
        s.add_talk_id(&[ADLER1, ADLER2, SINAI, INSPECTOR]);
        s.add_kill_id(&[HILLAS, PAPION, KINSUS, GARGOS, ADIANTUM]);
        s.register_quest_items(&[
            SWORD,
            ENH_SWORD1,
            ENH_SWORD2,
            BOOK1,
            BOOK2,
            BOOK3,
            BOOK4,
            BOOK5,
            BOOK6,
            BOOK7,
            WATER_ESSENCE,
            FIRE_ESSENCE,
            SHIELD_POTION,
            HEAL_POTION,
            FIRE_ENHANCER,
            WATER_ENHANCER,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = None;
        match event {
            "32500-02.htm" | "32500-03.htm" | "32500-04.htm" | "32500-05.htm" | "32497-02.htm"
            | "32507-07.htm" | "32497-04.htm" => htmltext = Some(event.to_owned()),
            "32497-03.htm" => {
                if !qs.is_started(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32500-06.htm" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    ctx.give_items(player, SWORD, 1);
                    ctx.give_items(player, BOOK1, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32507-04.htm" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    ctx.take_items(player, SWORD, -1);
                    ctx.take_items(player, WATER_ESSENCE, -1);
                    ctx.take_items(player, BOOK2, -1);
                    ctx.give_items(player, BOOK3, 1);
                    ctx.give_items(player, ENH_SWORD1, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32507-08.htm" => {
                if qs.is_cond(ctx, 6) {
                    qs.set_cond(ctx, 7, true);
                    ctx.take_items(player, ENH_SWORD1, -1);
                    ctx.take_items(player, BOOK5, -1);
                    ctx.take_items(player, FIRE_ESSENCE, -1);
                    ctx.give_items(player, ENH_SWORD2, 1);
                    ctx.give_items(player, BOOK6, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32510-02.htm" => {
                qs.exit_quest(ctx, false, true);
                if let Some(npc) = npc {
                    let instance_id = ctx.world().instance_id_of(npc.object_id);
                    // Java `inst.setDuration(EXIT_TIME * 60000)`, `setEmptyDestroyTime(0)`.
                    ctx.world_mut()
                        .set_instance_duration(instance_id, EXIT_TIME * 60_000);
                    ctx.world_mut()
                        .set_instance_empty_destroy_time(instance_id, 0);
                    let inside = ctx
                        .world()
                        .instance(instance_id)
                        .is_some_and(|inst| inst.contains_player(player));
                    if inside {
                        ctx.cast_skill(npc, player, SkillKey::new(VITALITY_REPLENISHING, 1));
                        ctx.add_exp_and_sp(player, 810_000, 50_000);
                        for id in REWARDS {
                            ctx.give_items(player, id, 1);
                        }
                    }
                }
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ADLER1 => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "32497-05.htm"
                    } else if ctx.player_level(player) > MAX_LEVEL {
                        "32497-06.htm"
                    } else {
                        "32497-01.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext = if qs.get_cond(ctx) > 1 {
                        "32497-00.htm"
                    } else {
                        "32497-03.htm"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = "32497-07.htm".to_owned(),
                _ => htmltext = "32497-01.htm".to_owned(),
            },
            SINAI => {
                htmltext = if qs.get_cond(ctx) > 1 {
                    "32500-00.htm"
                } else {
                    "32500-01.htm"
                }
                .to_owned();
            }
            INSPECTOR => match qs.get_cond(ctx) {
                1 => htmltext = "32507-01.htm".to_owned(),
                2 => htmltext = "32507-02.htm".to_owned(),
                3 => htmltext = "32507-03.htm".to_owned(),
                4 | 5 => htmltext = "32507-05.htm".to_owned(),
                6 => htmltext = "32507-06.htm".to_owned(),
                _ => htmltext = "32507-09.htm".to_owned(),
            },
            ADLER2 => {
                if qs.is_completed(ctx) {
                    htmltext = "32510-00.htm".to_owned();
                } else if qs.is_cond(ctx, 9) {
                    htmltext = "32510-01.htm".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        match npc.id {
            HILLAS => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, false);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    ctx.take_items(player, BOOK1, -1);
                    ctx.give_items(player, BOOK2, 1);
                    ctx.give_items(player, WATER_ESSENCE, 1);
                }
                ctx.add_spawn(PAPION, -53_903, 181_484, -4555, 30456, false, 0, instance_id);
            }
            PAPION => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, false);
                    ctx.take_items(player, BOOK3, -1);
                    ctx.give_items(player, BOOK4, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
                ctx.add_spawn(KINSUS, -61_415, 181_418, -4818, 63852, false, 0, instance_id);
            }
            KINSUS => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, false);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    ctx.take_items(player, BOOK4, -1);
                    ctx.give_items(player, BOOK5, 1);
                    ctx.give_items(player, FIRE_ESSENCE, 1);
                }
                ctx.add_spawn(GARGOS, -61_354, 183_624, -4821, 63613, false, 0, instance_id);
            }
            GARGOS => {
                if qs.is_cond(ctx, 7) {
                    qs.set_cond(ctx, 8, false);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    ctx.take_items(player, BOOK6, -1);
                    ctx.give_items(player, BOOK7, 1);
                }
                ctx.add_spawn(ADIANTUM, -53_297, 185_027, -4617, 1512, false, 0, instance_id);
            }
            ADIANTUM => {
                if qs.is_cond(ctx, 8) {
                    qs.set_cond(ctx, 9, false);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.take_items(player, BOOK7, -1);
                    ctx.add_spawn(ADLER2, -53_297, 185_027, -4617, 33486, false, 0, instance_id);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32497" type="Folk" level="40" name="Adler"/>
        <npc id="32510" type="Folk" level="40" name="Adler"/>
        <npc id="32500" type="Folk" level="40" name="Sinai"/>
        <npc id="32507" type="Folk" level="40" name="Inspector"/>
        <npc id="18610" type="Monster" level="40" name="Hillas"/>
        <npc id="18609" type="Monster" level="40" name="Papion"/>
        <npc id="18608" type="Monster" level="40" name="Kinsus"/>
        <npc id="18607" type="Monster" level="40" name="Gargos"/>
        <npc id="18620" type="Monster" level="40" name="Adiantum"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13034" type="Weapon" name="Sword"><set name="is_questitem" val="true"/></item>
        <item id="13130" type="EtcItem" name="Book 1"><set name="is_questitem" val="true"/></item>
        <item id="13131" type="EtcItem" name="Book 2"><set name="is_questitem" val="true"/></item>
        <item id="13132" type="EtcItem" name="Book 3"><set name="is_questitem" val="true"/></item>
        <item id="13133" type="EtcItem" name="Book 4"><set name="is_questitem" val="true"/></item>
        <item id="13134" type="EtcItem" name="Book 5"><set name="is_questitem" val="true"/></item>
        <item id="13135" type="EtcItem" name="Book 6"><set name="is_questitem" val="true"/></item>
        <item id="13136" type="EtcItem" name="Book 7"><set name="is_questitem" val="true"/></item>
        <item id="13038" type="EtcItem" name="Water Essence"><set name="is_questitem" val="true"/></item>
        <item id="13039" type="EtcItem" name="Fire Essence"><set name="is_questitem" val="true"/></item>
        <item id="13035" type="Weapon" name="Enhanced Sword 1"><set name="is_questitem" val="true"/></item>
        <item id="13036" type="Weapon" name="Enhanced Sword 2"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00128_PailakaSongOfIceAndFire";

    struct Npcs {
        adler1: Oid,
        sinai: Oid,
        inspector: Oid,
        adler2: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00128PailakaSongOfIceAndFire);
        let pages = Pages::of(&Q00128PailakaSongOfIceAndFire)
            .page("32497-01.htm", &["32497-02.htm"])
            .page("32497-02.htm", &["32497-03.htm", "32497-04.htm"])
            .plain(&["32497-03.htm", "32497-04.htm", "32497-05.htm", "32497-06.htm"])
            .page("32500-01.htm", &["32500-02.htm"])
            .page("32500-02.htm", &["32500-03.htm"])
            .page("32500-03.htm", &["32500-04.htm"])
            .page("32500-04.htm", &["32500-05.htm"])
            .page("32500-05.htm", &["32500-06.htm"])
            .plain(&["32500-06.htm", "32500-00.htm"])
            .page("32507-03.htm", &["32507-04.htm"])
            .plain(&["32507-01.htm", "32507-02.htm", "32507-04.htm", "32507-05.htm"])
            .page("32507-06.htm", &["32507-07.htm"])
            .page("32507-07.htm", &["32507-08.htm"])
            .plain(&["32507-08.htm", "32507-09.htm"])
            .page("32510-01.htm", &["32510-02.htm"])
            .plain(&["32510-00.htm", "32510-02.htm"])
            .npcs(&[ADLER1, SINAI, INSPECTOR, ADLER2])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            adler1: t.spawn(ADLER1, 20),
            sinai: t.spawn(SINAI, 40),
            inspector: t.spawn(INSPECTOR, 60),
            adler2: t.spawn(ADLER2, 80),
        };
        (t, npcs)
    }

    /// Kills the monster of `npc_id` that the last step spawned.
    fn kill_spawned(t: &mut ScriptTest, npc_id: i32) {
        let npc = t.npcs_of(npc_id)[0];
        t.kill(1, npc);
    }

    #[test]
    fn adler_gates_the_level_36_to_42() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, npcs.adler1).contains("32497-05.htm"));
        t.enter(2, 43, vec![]);
        assert!(t.quest_page(2, npcs.adler1).contains("32497-06.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_chain_from_adler_to_the_reward_outside_the_instance() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, npcs.adler1).contains("32497-01.htm"));
        assert!(t.event(1, NAME, "32497-02.htm").contains("32497-02.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "32497-03.htm").contains("32497-03.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Sinai hands out the sword and the first book: cond 2.
        assert!(t.quest_page(1, npcs.sinai).contains("32500-01.htm"));
        for page in ["32500-02.htm", "32500-03.htm", "32500-04.htm", "32500-05.htm"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert!(t.event(1, NAME, "32500-06.htm").contains("32500-06.htm"));
        assert_eq!((t.count(1, SWORD), t.count(1, BOOK1)), (1, 1));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Hillas dies: cond 3, Papion is spawned.
        let hillas = t.spawn(HILLAS, 20);
        t.kill(1, hillas);
        assert_eq!((t.count(1, BOOK2), t.count(1, WATER_ESSENCE)), (1, 1));
        assert_eq!(t.count(1, BOOK1), 0);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);
        assert_eq!(t.npcs_of(PAPION).len(), 1);

        // The inspector: cond 4, the enhanced sword.
        assert!(t.quest_page(1, npcs.inspector).contains("32507-03.htm"));
        assert!(t.event(1, NAME, "32507-04.htm").contains("32507-04.htm"));
        assert_eq!((t.count(1, SWORD), t.count(1, ENH_SWORD1)), (0, 1));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "4")]);

        // Papion (cond 5), Kinsus (cond 6).
        kill_spawned(&mut t, PAPION);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "5")]);
        kill_spawned(&mut t, KINSUS);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "6")]);

        // The inspector's second step: cond 7, the second enhanced sword.
        assert!(t.quest_page(1, npcs.inspector).contains("32507-06.htm"));
        assert!(t.event(1, NAME, "32507-07.htm").contains("32507-07.htm"));
        assert!(t.event(1, NAME, "32507-08.htm").contains("32507-08.htm"));
        assert_eq!((t.count(1, ENH_SWORD1), t.count(1, ENH_SWORD2)), (0, 1));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "7")]);

        // Gargos (cond 8), Adiantum (cond 9) spawns Adler's second NPC.
        kill_spawned(&mut t, GARGOS);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "8")]);
        kill_spawned(&mut t, ADIANTUM);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "9")]);

        // Adler's second NPC is brought to the player; outside the instance there is no reward,
        // only `exitQuest(false, true)`.
        t.world.run_as_script(NAME, |ctx| {
            ctx.tele_npc_to(
                ScriptNpc { object_id: npcs.adler2, id: ADLER2 },
                crate::testing::X,
                crate::testing::Y,
                crate::testing::Z,
            );
        });
        assert!(t.quest_page(1, npcs.adler2).contains("32510-01.htm"));
        assert!(t.event(1, NAME, "32510-02.htm").contains("32510-02.htm"));
        assert_eq!(t.count(1, 736), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
