//! Quest 144: Pailaka - Injured Dragon (the Ketra Orc shaman sends a hero into the Varka barracks).
// Java: dist/game/data/scripts/quests/Q00144_PailakaInjuredDragon/Q00144_PailakaInjuredDragon.java
//
// Pattern: events are numbers sent by the pages of three NPCs ("1" means a different thing for
// each, the NPC decides). The instance is entered through the instance script's `enter` event
// (`instances/pailaka_injured_dragon.rs`), the officer casts buffs on the player, the last NPC
// rewards and shortens the instance.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const KETRA_ORC_SHAMAN: i32 = 32499;
const KETRA_ORC_SUPPORTER_1: i32 = 32502;
const KETRA_ORC_INTELLIGENCE_OFFICER: i32 = 32509;
const KETRA_ORC_SUPPORTER2: i32 = 32512;

// Monster
const LATANA: i32 = 18660;

// Items
const PAILAKA_INSTANT_SHIELD: i32 = 13032;
const QUICK_HEALING_POTION: i32 = 13033;
const SPEAR_OF_SILENOS: i32 = 13052;
const ENHANCED_SPEAR_OF_SILENOS: i32 = 13053;
const COMPLETE_SPEAR_OF_SILENOS: i32 = 13054;
const PAILAKA_SOULSHOT_GRADE_A: i32 = 13055;
const WEAPON_UPGRADE_STAGE_1: i32 = 13056;
const WEAPON_UPGRADE_STAGE_2: i32 = 13057;
const SILENOS_HAIR_ACCESSORY: i32 = 13058;
const PAILAKA_SHIRT: i32 = 13296;
const SCROLL_OF_ESCAPE: i32 = 736;

// Skills (id, level)
const PAILAKA_REWARD_ENERGY_REPLENISHING: (i32, i32) = (5774, 2);
const BUFFS: [(i32, i32); 12] = [
    (1086, 2), // Haste Lv2
    (1204, 2), // Wind Walk Lv2
    (1059, 3), // Empower Lv3
    (1085, 3), // Acumen Lv3
    (1078, 6), // Concentration Lv6
    (1068, 3), // Might Lv3
    (1240, 3), // Guidance Lv3
    (1077, 3), // Focus Lv3
    (1242, 3), // Death Whisper Lv3
    (1062, 2), // Berserker Spirit Lv2
    (1268, 4), // Vampiric Rage Lv4
    (1045, 6), // Blessed Body Lv6
];

// Misc
const MIN_LEVEL: i32 = 73;
const MAX_LEVEL: i32 = 77;
const MAX_BUFFS: usize = 5;
const XP_REWARD: i64 = 28_000_000;
const SP_REWARD: i32 = 2_850_000;

/// Java `PailakaInjuredDragon.class.getSimpleName()`.
const PAILAKA_INJURED_DRAGON: &str = "PailakaInjuredDragon";

/// Java `Q00144_PailakaInjuredDragon`.
pub struct Q00144PailakaInjuredDragon;

/// Java `countBuffs(player)`.
fn count_buffs(ctx: &ScriptCtx<'_>, player: ObjectId) -> usize {
    BUFFS
        .iter()
        .filter(|&&(id, _)| ctx.is_affected_by_skill(player, id))
        .count()
}

impl Script for Q00144PailakaInjuredDragon {
    fn name(&self) -> &'static str {
        "Q00144_PailakaInjuredDragon"
    }

    fn path(&self) -> &'static str {
        "quests/Q00144_PailakaInjuredDragon"
    }

    fn id(&self) -> i32 {
        144
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KETRA_ORC_SHAMAN]);
        s.add_first_talk_id(&[KETRA_ORC_INTELLIGENCE_OFFICER, KETRA_ORC_SUPPORTER2]);
        s.add_talk_id(&[
            KETRA_ORC_SHAMAN,
            KETRA_ORC_SUPPORTER_1,
            KETRA_ORC_INTELLIGENCE_OFFICER,
            KETRA_ORC_SUPPORTER2,
        ]);
        s.add_kill_id(&[LATANA]);
        s.register_quest_items(&[
            PAILAKA_INSTANT_SHIELD,
            QUICK_HEALING_POTION,
            SPEAR_OF_SILENOS,
            ENHANCED_SPEAR_OF_SILENOS,
            COMPLETE_SPEAR_OF_SILENOS,
            PAILAKA_SOULSHOT_GRADE_A,
            WEAPON_UPGRADE_STAGE_1,
            WEAPON_UPGRADE_STAGE_2,
            SILENOS_HAIR_ACCESSORY,
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
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "start" => {
                let level = ctx.player_level(player);
                if qs.is_created(ctx) && (MIN_LEVEL..=MAX_LEVEL).contains(&level) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                }
                htmltext = Some("32499-07.html".to_owned());
            }
            "1" => {
                // Java reads `npc.getId()` here: no NPC, no answer (it would throw).
                let npc = npc?;
                match npc.id {
                    // Ask to hear more
                    KETRA_ORC_SHAMAN => {
                        htmltext = Some(
                            if ctx.player_level(player) < MIN_LEVEL {
                                "32499-03.htm"
                            } else if ctx.player_level(player) > MAX_LEVEL {
                                "32499-04z.htm"
                            } else {
                                "32499-04.htm"
                            }
                            .to_owned(),
                        );
                    }
                    // Enhance the weapon
                    KETRA_ORC_INTELLIGENCE_OFFICER => {
                        if qs.is_started(ctx) {
                            if ctx.has_quest_items(player, SPEAR_OF_SILENOS) {
                                if ctx.has_quest_items(player, WEAPON_UPGRADE_STAGE_1) {
                                    ctx.take_items(player, SPEAR_OF_SILENOS, -1);
                                    ctx.take_items(player, WEAPON_UPGRADE_STAGE_1, -1);
                                    ctx.give_items(player, ENHANCED_SPEAR_OF_SILENOS, 1);
                                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                                    htmltext = Some("32509-02.html".to_owned());
                                } else {
                                    htmltext = Some("32509-05.html".to_owned());
                                }
                            } else if ctx.has_quest_items(player, ENHANCED_SPEAR_OF_SILENOS) {
                                if ctx.has_quest_items(player, WEAPON_UPGRADE_STAGE_2) {
                                    ctx.take_items(player, ENHANCED_SPEAR_OF_SILENOS, -1);
                                    ctx.take_items(player, WEAPON_UPGRADE_STAGE_2, -1);
                                    ctx.give_items(player, COMPLETE_SPEAR_OF_SILENOS, 1);
                                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                                    htmltext = Some("32509-03.html".to_owned());
                                } else {
                                    htmltext = Some("32509-04.html".to_owned());
                                }
                            } else if ctx.has_quest_items(player, COMPLETE_SPEAR_OF_SILENOS) {
                                htmltext = Some("32509-06.html".to_owned());
                            } else if !ctx.has_quest_items(player, SPEAR_OF_SILENOS) {
                                htmltext = Some("32509-01a.html".to_owned());
                            }
                        }
                    }
                    // Return the Spear
                    KETRA_ORC_SUPPORTER2 => {
                        if !qs.is_completed(ctx) && qs.is_cond(ctx, 4) {
                            ctx.give_items(player, PAILAKA_SHIRT, 1);
                            ctx.give_items(player, SCROLL_OF_ESCAPE, 1);
                            ctx.add_exp_and_sp(player, XP_REWARD, SP_REWARD);
                            let (skill_id, level) = PAILAKA_REWARD_ENERGY_REPLENISHING;
                            ctx.cast_skill(npc, player, SkillKey::new(skill_id, level));
                            qs.exit_quest(ctx, false, true);

                            let instance_id = ctx.world().instance_id_of(npc.object_id);
                            if ctx.world().instance(instance_id).is_some() {
                                let finish_time = ctx.world().script_instance_finish_time_ms();
                                ctx.world_mut().set_instance_duration(instance_id, finish_time);
                                ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
                            }

                            htmltext = Some("32512-02z.html".to_owned());
                        } else if qs.is_completed(ctx) {
                            htmltext = Some("32512-03.html".to_owned());
                        } else {
                            htmltext = Some(ctx.get_no_quest_msg(player));
                        }
                    }
                    _ => {}
                }
            }
            // Ask about this strange darkness
            "3249905" => htmltext = Some("32499-05.htm".to_owned()),
            "2" => {
                let npc = npc?;
                match npc.id {
                    // Keep listening
                    KETRA_ORC_SHAMAN => {
                        if qs.is_created(ctx) && ctx.player_level(player) >= MIN_LEVEL {
                            htmltext = Some("32499-06.htm".to_owned());
                        }
                    }
                    // I understand
                    KETRA_ORC_SUPPORTER_1 => {
                        if qs.is_cond(ctx, 2) {
                            qs.set_cond(ctx, 3, true);
                            qs.set_memo_state(ctx, 3);
                            ctx.give_items(player, SPEAR_OF_SILENOS, 1);
                            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                            htmltext = Some("32502-05.html".to_owned());
                        }
                    }
                    // Receive enhancement magic
                    KETRA_ORC_INTELLIGENCE_OFFICER => {
                        htmltext = Some(
                            if count_buffs(ctx, player) >= MAX_BUFFS {
                                "32509-96.html"
                            } else {
                                "32509-99.html"
                            }
                            .to_owned(),
                        );
                    }
                    KETRA_ORC_SUPPORTER2 => {
                        if qs.is_completed(ctx) {
                            htmltext = Some("32512-03.html".to_owned());
                        }
                    }
                    _ => {}
                }
            }
            // Come back later
            "3249908" => htmltext = Some("32499-08a.html".to_owned()),
            // Go now / Enter Pailaka
            "3" => {
                if qs.is_started(ctx) {
                    ctx.notify_event_of(PAILAKA_INJURED_DRAGON, "enter", npc, Some(player));

                    if qs.is_cond(ctx, 1) {
                        qs.set_cond(ctx, 2, true);
                        htmltext = Some("32499-09.html".to_owned());
                    } else {
                        htmltext = Some("32499-11.html".to_owned());
                    }
                }
            }
            // Keep listening
            "3250202" => htmltext = Some("32502-02.html".to_owned()),
            // Ask if he wants to rescue the dragon
            "3250203" => htmltext = Some("32502-03.html".to_owned()),
            // Ask what can be done
            "3250204" => htmltext = Some("32502-04.html".to_owned()),
            // Ask about the enemy
            "3250207" => htmltext = Some("32502-07.html".to_owned()),
            // Please select the next spell you wish to receive!
            "-1" | "-2" | "-3" | "-4" | "-5" | "-6" | "-7" | "-8" | "-9" | "-10" | "-11" | "-12" => {
                let npc = npc?;
                let buff_count = count_buffs(ctx, player);
                if buff_count < MAX_BUFFS {
                    if !ctx.world().is_casting_now(npc.object_id) {
                        let current = ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .map_or(0, |v| v.get_int("i_ai0", 0));
                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_int("i_ai0", current + 1);
                        // `BUFFS[-(Integer.valueOf(event) + 1)]`
                        let index = event.parse::<i32>().map_or(0, |n| (-(n + 1)) as usize);
                        let (skill_id, level) = BUFFS[index];
                        ctx.cast_skill(npc, player, SkillKey::new(skill_id, level));

                        if buff_count == MAX_BUFFS - 1 {
                            htmltext = Some("32509-97.html".to_owned());
                        } else {
                            htmltext = Some("32509-98.html".to_owned());
                        }
                    } else {
                        htmltext = Some("32509-98.html".to_owned());
                    }
                } else {
                    htmltext = Some("32509-96.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            KETRA_ORC_SHAMAN => match qs.state(ctx) {
                CREATED => htmltext = "32499-01.htm".to_owned(),
                STARTED => {
                    htmltext = if qs.is_cond(ctx, 1) {
                        "32499-08.html"
                    } else {
                        "32499-10.html"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = "32499-02.html".to_owned(),
                _ => {}
            },
            KETRA_ORC_SUPPORTER_1 => {
                htmltext = if qs.is_cond(ctx, 2) {
                    "32502-01.html"
                } else {
                    "32502-06.html"
                }
                .to_owned();
            }
            KETRA_ORC_SUPPORTER2 => {
                if qs.is_completed(ctx) {
                    htmltext = "32512-02.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == KETRA_ORC_SUPPORTER2 {
            let qs = ctx.get_quest_state(player, false);
            if let Some(qs) = qs
                && qs.is_completed(ctx)
            {
                return Some("32512-03.html".to_owned());
            }
        }

        Some(format!("{}.html", npc.id))
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 3)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            qs.set_cond(ctx, 4, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00144_PailakaInjuredDragon";
    /// A marker item the stand-in instance script hands out on `enter`.
    const MARKER: i32 = 900;

    const NPCS: &str = r#"
        <npc id="32499" type="Folk" level="1" name="Ketra Orc Shaman"/>
        <npc id="32502" type="Folk" level="1" name="Ketra Orc Supporter"/>
        <npc id="32509" type="Folk" level="1" name="Ketra Orc Intelligence Officer"/>
        <npc id="32512" type="Folk" level="1" name="Ketra Orc Supporter"/>
        <npc id="18660" type="Monster" level="60" name="Latana"/>"#;

    const ITEMS: &str = r#"
        <item id="900" type="EtcItem" name="Marker"/>
        <item id="13052" type="EtcItem" name="Spear of Silenos"><set name="is_questitem" val="true"/></item>
        <item id="13053" type="EtcItem" name="Enhanced Spear of Silenos"><set name="is_questitem" val="true"/></item>
        <item id="13054" type="EtcItem" name="Complete Spear of Silenos"><set name="is_questitem" val="true"/></item>
        <item id="13056" type="EtcItem" name="Weapon Upgrade Stage 1"><set name="is_questitem" val="true"/></item>
        <item id="13057" type="EtcItem" name="Weapon Upgrade Stage 2"><set name="is_questitem" val="true"/></item>
        <item id="13296" type="Armor" name="Pailaka Shirt"/>
        <item id="736" type="EtcItem" name="Scroll of Escape"/>"#;

    /// Java `PailakaInjuredDragon`, as far as the quest needs it: the `enter` event.
    struct Instance;

    impl Script for Instance {
        fn name(&self) -> &'static str {
            PAILAKA_INJURED_DRAGON
        }
        fn path(&self) -> &'static str {
            "instances/PailakaInjuredDragon"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
        fn on_event(
            &self,
            ctx: &mut ScriptCtx<'_>,
            event: &str,
            _npc: Option<ScriptNpc>,
            player: Option<Oid>,
        ) -> Option<String> {
            if event == "enter"
                && let Some(player) = player
            {
                ctx.give_items(player, MARKER, 1);
            }
            None
        }
    }

    struct Npcs {
        shaman: Oid,
        supporter1: Oid,
        officer: Oid,
        supporter2: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00144PailakaInjuredDragon);
        registry.add(Instance);
        let pages = Pages::of(&Q00144PailakaInjuredDragon)
            .page("32499-01.htm", &["1"])
            .page("32499-04.htm", &["3249905"])
            .page("32499-05.htm", &["2"])
            .page("32499-06.htm", &["start"])
            .page("32499-07.html", &["3", "3249908"])
            .page("32499-08.html", &["3"])
            .page("32499-10.html", &["3"])
            .page("32502-01.html", &["3250202"])
            .page("32502-02.html", &["3250203"])
            .page("32502-03.html", &["3250204"])
            .page("32502-04.html", &["2"])
            .page("32509.html", &["1", "2"])
            .page("32509-99.html", &["-1", "-2", "-3", "-4", "-5", "-6"])
            .page("32509-98.html", &["-1", "-2", "-3", "-4", "-5", "-6"])
            .page("32512.html", &["1"])
            .plain(&[
                "32499-02.html",
                "32499-03.htm",
                "32499-04z.htm",
                "32499-08a.html",
                "32499-09.html",
                "32499-11.html",
                "32502-05.html",
                "32502-06.html",
                "32502-07.html",
                "32509-01a.html",
                "32509-02.html",
                "32509-03.html",
                "32509-04.html",
                "32509-05.html",
                "32509-06.html",
                "32509-96.html",
                "32509-97.html",
                "32512-02.html",
                "32512-02z.html",
                "32512-03.html",
            ])
            .npcs(&[
                KETRA_ORC_SHAMAN,
                KETRA_ORC_SUPPORTER_1,
                KETRA_ORC_INTELLIGENCE_OFFICER,
                KETRA_ORC_SUPPORTER2,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            shaman: t.spawn(KETRA_ORC_SHAMAN, 20),
            supporter1: t.spawn(KETRA_ORC_SUPPORTER_1, 30),
            officer: t.spawn(KETRA_ORC_INTELLIGENCE_OFFICER, 40),
            supporter2: t.spawn(KETRA_ORC_SUPPORTER2, 50),
        };
        (t, npcs)
    }

    #[test]
    fn the_level_picks_the_page_of_the_shaman() {
        let (mut t, npcs) = setup();
        t.enter(1, 72, vec![]);
        assert!(t.quest_page(1, npcs.shaman).contains("32499-01.htm"));
        assert!(t.event(1, NAME, "1").contains("32499-03.htm"));
        t.enter(2, 78, vec![]);
        assert!(t.quest_page(2, npcs.shaman).contains("32499-01.htm"));
        assert!(t.event(2, NAME, "1").contains("32499-04z.htm"));
        // "start" below the level range only shows the page.
        t.enter(3, 72, vec![]);
        assert!(t.quest_page(3, npcs.shaman).contains("32499-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn from_the_shaman_through_the_spear_and_latana_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, npcs.shaman).contains("32499-01.htm"));
        assert!(t.event(1, NAME, "1").contains("32499-04.htm"));
        assert!(t.event(1, NAME, "3249905").contains("32499-05.htm"));
        assert!(t.event(1, NAME, "2").contains("32499-06.htm"));
        assert!(t.event(1, NAME, "start").contains("32499-07.html"));
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(writes.len(), 3, "the memo state is the third row: {writes:?}");

        // Go now: the instance script gets `enter`, cond 2.
        assert!(t.event(1, NAME, "3").contains("32499-09.html"));
        assert_eq!(t.count(1, MARKER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.shaman).contains("32499-10.html"));

        // Supporter 1: three pages, then "I understand" gives the spear and cond 3.
        assert!(t.quest_page(1, npcs.supporter1).contains("32502-01.html"));
        assert!(t.event(1, NAME, "3250202").contains("32502-02.html"));
        assert!(t.event(1, NAME, "3250203").contains("32502-03.html"));
        assert!(t.event(1, NAME, "3250204").contains("32502-04.html"));
        assert!(t.event(1, NAME, "2").contains("32502-05.html"));
        assert_eq!(t.count(1, SPEAR_OF_SILENOS), 1);
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::update(NAME, "cond", "3"));
        assert!(t.quest_page(1, npcs.supporter1).contains("32502-06.html"));

        // Latana dies: cond 4.
        let latana = t.spawn(LATANA, 60);
        t.kill(1, latana);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The last supporter: first talk page, then "1" rewards.
        assert!(t.quest_page(1, npcs.supporter2).contains("32512.html"));
        assert!(t.event(1, NAME, "1").contains("32512-02z.html"));
        assert_eq!(t.count(1, PAILAKA_SHIRT), 1);
        assert_eq!(t.count(1, SCROLL_OF_ESCAPE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        // Completed: the first talk page shows 32512-03.
        assert!(t.quest_page(1, npcs.supporter2).contains("32512-03.html"));
        assert!(t.quest_page(1, npcs.shaman).contains("32499-02.html"));
    }

    #[test]
    fn the_officer_upgrades_the_spear_and_casts_buffs() {
        let (mut t, npcs) = setup();
        let mut data = player_data(
            1,
            75,
            vec![
                item(901, SPEAR_OF_SILENOS, 1),
                item(902, WEAPON_UPGRADE_STAGE_1, 1),
                item(903, WEAPON_UPGRADE_STAGE_2, 1),
            ],
        );
        data.profile.quests = l2_world::QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("3".to_owned()))],
        };
        t.enter_with(data);

        assert!(t.quest_page(1, npcs.officer).contains("32509.html"));
        assert!(t.event(1, NAME, "1").contains("32509-02.html"));
        assert_eq!(t.count(1, SPEAR_OF_SILENOS), 0);
        assert_eq!(t.count(1, ENHANCED_SPEAR_OF_SILENOS), 1);
        assert!(t.quest_page(1, npcs.officer).contains("32509.html"));
        assert!(t.event(1, NAME, "1").contains("32509-03.html"));
        assert_eq!(t.count(1, COMPLETE_SPEAR_OF_SILENOS), 1);
        assert!(t.quest_page(1, npcs.officer).contains("32509.html"));
        assert!(t.event(1, NAME, "1").contains("32509-06.html"));

        // "Receive enhancement magic": no buff yet.
        assert!(t.quest_page(1, npcs.officer).contains("32509.html"));
        assert!(t.event(1, NAME, "2").contains("32509-99.html"));
        // A buff: the NPC's `i_ai0` counts the casts.
        assert!(t.event(1, NAME, "-1").contains("32509-98.html"));
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                ctx.world()
                    .npc_variables(npcs.officer)
                    .map(|v| v.get_int("i_ai0", 0)),
                Some(1)
            );
        });
    }
}
