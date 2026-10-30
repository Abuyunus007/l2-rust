//! Quest 311: Expulsion of Evil Spirits (Chairen, the Ragna Orcs and the Varangka altar).
// Java: dist/game/data/scripts/quests/Q00311_ExpulsionOfEvilSpirits/Q00311_ExpulsionOfEvilSpirits.java
//
// Two things in one script: the quest (a soul core every so often from Ragna Orcs, the amulets
// as the second item, the pendant for ten cores) and the altar of the Ragna Orcs' zone (the
// altar spawns on a global timer, Varangka and two minions come when a player with the pendant
// hits the altar, and players without the pendant are hurt by it). The NPCs of the altar are
// kept in `ExpulsionState` (Java's statics). The respawn time is the global variable
// `VarangkaRespawn` (D-54).

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CHAIREN: i32 = 32655;
const ALTAR: i32 = 18811;
const VARANGKA: i32 = 18808;

// Items
const PROTECTION_SOULS_PENDANT: i32 = 14848;
const SOUL_CORE_CONTAINING_EVIL_SPIRIT: i32 = 14881;
const RAGNA_ORCS_AMULET: i32 = 14882;

// Monsters (id, the amulet chance, as Java's double)
const MONSTERS: [(i32, f64); 14] = [
    (22691, 0.694), // Ragna Orc
    (22692, 0.716), // Ragna Orc Warrior
    (22693, 0.736), // Ragna Orc Hero
    (22694, 0.712), // Ragna Orc Commander
    (22695, 0.698), // Ragna Orc Healer
    (22696, 0.692), // Ragna Orc Shaman
    (22697, 0.640), // Ragna Orc Seer
    (22698, 0.716), // Ragna Orc Archer
    (22699, 0.752), // Ragna Orc Sniper
    (22701, 0.716), // Varangka's Dre Vanul
    (22702, 0.662), // Varangka's Destroyer
    (18808, 0.7),   // Varangka
    (18809, 0.694), // Varangka's minion
    (18810, 0.694), // Varangka's minion
];

// Misc
const MIN_LEVEL: i32 = 80;
const SOUL_CORE_COUNT: i64 = 10;
const RAGNA_ORCS_KILLS_COUNT: i32 = 100;
const RAGNA_ORCS_AMULET_COUNT: i64 = 10;

/// Java `ALTARZONE`: the zone 20201 ("Vangara") of `zones/custom_script.xml`.
const ALTAR_ZONE: i32 = 20201;
const ALTAR_ZONE_TYPE: &str = "ScriptZone";

/// Java `DEBUFF_SKILL_ID` / `DEBUFF_SKILL_LEVEL` of `zoneCheck`.
const DEBUFF_SKILL_ID: i32 = 6148;
const DEBUFF_SKILL_LEVEL: i32 = 1;

/// Java `GlobalVariablesManager` key of the altar's respawn time (ms since the epoch, 0 = none).
const VARANGKA_RESPAWN: &str = "VarangkaRespawn";

/// Java `Q00311_ExpulsionOfEvilSpirits.class.getSimpleName()`.
const QUEST_NAME: &str = "Q00311_ExpulsionOfEvilSpirits";

/// Java's statics of the script: the altar and Varangka's NPCs (ids of the spawned NPCs).
#[derive(Default)]
struct ExpulsionState {
    altar: Option<ScriptNpc>,
    varangka: Option<ScriptNpc>,
    minion1: Option<ScriptNpc>,
    minion2: Option<ScriptNpc>,
}

/// Java `Q00311_ExpulsionOfEvilSpirits`.
pub struct Q00311ExpulsionOfEvilSpirits;

/// Java `checkIfSpawned(npcId)`: an NPC of that template is inside the altar zone. Players are
/// never such an NPC (their ids are class ids).
fn zone_has_npc(ctx: &ScriptCtx<'_>, npc_id: i32) -> bool {
    ctx.world()
        .zone_characters_inside(ALTAR_ZONE, ALTAR_ZONE_TYPE)
        .into_iter()
        .any(|id| ctx.world().npc(id).is_some_and(|npc| npc.npc_id == npc_id))
}

/// Java `ALTARZONE.getCharactersInside()` filtered to the altar's NPCs (`Attackable` in the
/// Java loop).
fn zone_attackable_npcs(ctx: &ScriptCtx<'_>) -> Vec<ScriptNpc> {
    ctx.world()
        .zone_characters_inside(ALTAR_ZONE, ALTAR_ZONE_TYPE)
        .into_iter()
        .filter(|id| ctx.world().is_attackable_npc(*id))
        .filter_map(|id| {
            ctx.world().npc(id).map(|npc| ScriptNpc {
                object_id: id,
                id: npc.npc_id,
            })
        })
        .collect()
}

/// The place of a player (`getX()`, `getY()`); (0, 0) for a player who is not in the world.
fn player_xy(ctx: &ScriptCtx<'_>, player: ObjectId) -> (i32, i32) {
    ctx.world().position_of(player).map_or((0, 0), |(x, y, _)| (x, y))
}

/// Java `altarSpawn` event: the altar appears when it is not in the zone yet.
fn altar_spawn(ctx: &mut ScriptCtx<'_>, npc: Option<ScriptNpc>, player: Option<ObjectId>) {
    if zone_has_npc(ctx, ALTAR) {
        return;
    }
    let altar = ctx.add_spawn(ALTAR, 74120, -101920, -960, 32760, false, 0, 0);
    ctx.state::<ExpulsionState>().altar = altar;
    if let Some(altar) = altar {
        ctx.world_mut().set_invul(altar.object_id, true);
    }
    ctx.set_global_variable(VARANGKA_RESPAWN, "0");
    // Java `checkIfInRange(1200, npc, player, true)`: the timer has no NPC or player, so this is
    // false (Java's LocationUtil returns false for a null object).
    if let (Some(npc), Some(player)) = (npc, player)
        && ctx.check_if_in_range(1200, npc.object_id, player, true)
    {
        ctx.start_quest_timer("zoneCheck", 1000, None, Some(player), false);
    }
}

/// Java `zoneCheck.castDebuff(player)`: the altar casts its debuff on the player.
fn cast_debuff(ctx: &mut ScriptCtx<'_>, altar: ScriptNpc, player: ObjectId) {
    // Java stops the debuffs of the skill on the player (the effected of each debuff).
    ctx.world_mut()
        .script_stop_skill_effects_removed(player, DEBUFF_SKILL_ID);
    ctx.broadcast_magic_skill_use(
        altar.object_id,
        altar.object_id,
        player,
        DEBUFF_SKILL_ID,
        DEBUFF_SKILL_LEVEL,
        1000,
        0,
    );
    ctx.world_mut().apply_effects(
        altar.object_id,
        player,
        SkillKey::new(DEBUFF_SKILL_ID, DEBUFF_SKILL_LEVEL),
    );
}

/// Java `zoneCheck.run()`: while the player is in the zone of a live altar and has no pendant
/// (or no quest state at all), the altar casts its debuff, every 3 s.
fn zone_check(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let Some(altar) = ctx.state::<ExpulsionState>().altar else {
        return;
    };
    if !ctx
        .world()
        .is_character_in_zone(ALTAR_ZONE, ALTAR_ZONE_TYPE, player)
    {
        return;
    }
    let no_state = ctx.player_quest_state(player, QUEST_NAME).is_none();
    if no_state || !ctx.has_quest_items(player, PROTECTION_SOULS_PENDANT) {
        cast_debuff(ctx, altar, player);
        // NOTE(java-quirk): Java schedules a new runnable each time (and one for every entry
        // and attack), so several checks can run side by side; the timer of this port is one per
        // player and name (FOLLOW-UP(engine): timers that do not merge).
        ctx.start_quest_timer("zoneCheck", 3000, None, Some(player), false);
    }
}

impl Script for Q00311ExpulsionOfEvilSpirits {
    fn name(&self) -> &'static str {
        QUEST_NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00311_ExpulsionOfEvilSpirits"
    }

    fn id(&self) -> i32 {
        311
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CHAIREN]);
        s.add_talk_id(&[CHAIREN]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[SOUL_CORE_CONTAINING_EVIL_SPIRIT, RAGNA_ORCS_AMULET]);
        s.add_enter_zone_id(&[ALTAR_ZONE]);
        s.add_attack_id(&[ALTAR]);
    }

    /// Java constructor: the altar's timer, from the saved respawn time.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let respawn_time = ctx.global_variables().get_long(VARANGKA_RESPAWN, 0);
        ctx.set_global_variable(VARANGKA_RESPAWN, &respawn_time.to_string());
        let now = ctx.now_ms();
        if respawn_time == 0 || respawn_time - now < 0 {
            ctx.start_quest_timer("altarSpawn", 5000, None, None, false);
        } else {
            ctx.start_quest_timer("altarSpawn", respawn_time - now, None, None, false);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("altarSpawn") {
            altar_spawn(ctx, npc, player);
            return None;
        }
        if event.eq_ignore_ascii_case("zoneCheck") {
            if let Some(player) = player {
                zone_check(ctx, player);
            }
            return None;
        }
        if event.eq_ignore_ascii_case("minion1") && zone_has_npc(ctx, VARANGKA) {
            let Some(player) = player else {
                return None;
            };
            if !zone_has_npc(ctx, VARANGKA + 1)
                && let Some(varangka) = ctx.state::<ExpulsionState>().varangka
            {
                // Java `player.getX() + Rnd.get(10, 50)`, then the Y.
                let (px, py) = player_xy(ctx, player);
                let x = px + ctx.get_random_range(10, 50);
                let y = py + ctx.get_random_range(10, 50);
                let minion = ctx.add_spawn(VARANGKA + 1, x, y, -967, 0, false, 0, 0);
                ctx.state::<ExpulsionState>().minion1 = minion;
                if let Some(minion) = minion {
                    ctx.set_running(minion);
                    // NOTE(java-quirk): Java reads `_varangka.getTarget().asPlayer()` without a null
                    // check; with no target the minion stays without hate and attack.
                    if let Some(target) = ctx.world().target_of(varangka.object_id) {
                        ctx.world_mut()
                            .add_damage_hate(minion.object_id, target, 1, 99_999);
                        ctx.set_intention_attack(minion, target);
                    }
                }
            }
            return None;
        }
        if event.eq_ignore_ascii_case("minion2") {
            let Some(player) = player else {
                return None;
            };
            if !zone_has_npc(ctx, VARANGKA + 2) && zone_has_npc(ctx, VARANGKA) {
                let (px, py) = player_xy(ctx, player);
                let x = px + ctx.get_random_range(10, 50);
                let y = py + ctx.get_random_range(10, 50);
                let minion = ctx.add_spawn(VARANGKA + 2, x, y, -967, 0, false, 0, 0);
                ctx.state::<ExpulsionState>().minion2 = minion;
                let varangka = ctx.state::<ExpulsionState>().varangka;
                if let Some(minion) = minion {
                    ctx.set_running(minion);
                    if let Some(varangka) = varangka
                        && let Some(target) = ctx.world().target_of(varangka.object_id)
                    {
                        ctx.world_mut()
                            .add_damage_hate(minion.object_id, target, 1, 99_999);
                        ctx.set_intention_attack(minion, target);
                    }
                }
            }
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        if ctx.player_level(player) < MIN_LEVEL {
            return None;
        }

        let mut htmltext = None;
        match event {
            "32655-03.htm" | "32655-15.html" => {
                htmltext = Some(event.to_owned());
            }
            "32655-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32655-11.html" => {
                if ctx.get_quest_items_count(player, SOUL_CORE_CONTAINING_EVIL_SPIRIT) >= SOUL_CORE_COUNT {
                    ctx.take_items(player, SOUL_CORE_CONTAINING_EVIL_SPIRIT, SOUL_CORE_COUNT);
                    ctx.give_items(player, PROTECTION_SOULS_PENDANT, 1);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32655-12.html".to_owned());
                }
            }
            "32655-13.html" => {
                if !ctx.has_quest_items(player, SOUL_CORE_CONTAINING_EVIL_SPIRIT)
                    && ctx.get_quest_items_count(player, RAGNA_ORCS_AMULET) >= RAGNA_ORCS_AMULET_COUNT
                {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32655-14.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member_qs) = ctx.random_party_member_state(killer, 1, 2, Some(npc)) else {
            return;
        };
        let member = member_qs.player();

        if npc.id == VARANGKA {
            if !member_qs.is_cond(ctx, 1) {
                return;
            }
            let state = ctx.state::<ExpulsionState>();
            let (altar, minion1, minion2) = (state.altar, state.minion1, state.minion2);
            state.altar = None;
            state.varangka = None;
            // Java `_altar.doDie(killer)`: the altar is the one spawned by the timer.
            if let Some(altar) = altar {
                ctx.do_die(altar, killer);
            }
            if zone_has_npc(ctx, VARANGKA + 1)
                && let Some(minion) = minion1
            {
                ctx.do_die(minion, killer);
            }
            if zone_has_npc(ctx, VARANGKA + 2)
                && let Some(minion) = minion2
            {
                ctx.do_die(minion, killer);
            }
            ctx.cancel_quest_timers("minion1");
            ctx.cancel_quest_timers("minion2");
            let state = ctx.state::<ExpulsionState>();
            state.minion1 = None;
            state.minion2 = None;

            let respawn = i64::from(ctx.get_random_range(14_400_000, 28_800_000));
            let at = ctx.now_ms() + respawn;
            ctx.set_global_variable(VARANGKA_RESPAWN, &at.to_string());
            ctx.start_quest_timer("altarSpawn", respawn, None, None, false);
            ctx.take_items(member, PROTECTION_SOULS_PENDANT, 1);
            return;
        } else if npc.id == VARANGKA + 1 {
            ctx.state::<ExpulsionState>().minion1 = None;
            let delay = i64::from(ctx.get_random_range(60_000, 120_000));
            ctx.start_quest_timer("minion1", delay, Some(npc), Some(killer), false);
            return;
        } else if npc.id == VARANGKA + 2 {
            ctx.state::<ExpulsionState>().minion2 = None;
            let delay = i64::from(ctx.get_random_range(60_000, 120_000));
            ctx.start_quest_timer("minion2", delay, Some(npc), Some(killer), false);
            return;
        }

        // The Ragna Orcs: a soul core after 100 kills, with a chance that grows with each kill;
        // every other kill counts up in the memo state (slot 1).
        let count = member_qs.get_memo_state_ex(ctx, 1) + 1;
        if count >= RAGNA_ORCS_KILLS_COUNT && ctx.get_random(20) < (count % 100) + 1 {
            member_qs.set_memo_state_ex(ctx, 1, 0);
            ctx.give_items(member, SOUL_CORE_CONTAINING_EVIL_SPIRIT, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        } else {
            member_qs.set_memo_state_ex(ctx, 1, count);
        }

        // Java `MONSTERS.get(npc.getId()) < Rnd.get(1d)`.
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0.0, |(_, chance)| *chance);
        if chance < ctx.world_mut().rng().below_f64(1.0) {
            ctx.give_items(member, RAGNA_ORCS_AMULET, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32655-01.htm"
            } else {
                "32655-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = if ctx
                .has_at_least_one_quest_item(player, &[RAGNA_ORCS_AMULET, SOUL_CORE_CONTAINING_EVIL_SPIRIT])
            {
                "32655-06.html"
            } else {
                "32655-05.html"
            }
            .to_owned();
        }
        Some(htmltext)
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.player_quest_state(attacker, QUEST_NAME).is_none() {
            return;
        }
        if ctx.has_quest_items(attacker, PROTECTION_SOULS_PENDANT) && ctx.get_random(100) < 20 {
            if ctx.state::<ExpulsionState>().varangka.is_none() && !zone_has_npc(ctx, VARANGKA) {
                let varangka = ctx.add_spawn(VARANGKA, 74914, -101922, -967, 0, false, 0, 0);
                ctx.state::<ExpulsionState>().varangka = varangka;
                if ctx.state::<ExpulsionState>().minion1.is_none() && !zone_has_npc(ctx, VARANGKA + 1) {
                    let x = 74914 + ctx.get_random_range(10, 50);
                    let y = -101922 + ctx.get_random_range(10, 50);
                    let minion = ctx.add_spawn(VARANGKA + 1, x, y, -967, 0, false, 0, 0);
                    ctx.state::<ExpulsionState>().minion1 = minion;
                }
                if ctx.state::<ExpulsionState>().minion2.is_none() && !zone_has_npc(ctx, VARANGKA + 2) {
                    let x = 74914 + ctx.get_random_range(10, 50);
                    let y = -101922 + ctx.get_random_range(10, 50);
                    let minion = ctx.add_spawn(VARANGKA + 2, x, y, -967, 0, false, 0, 0);
                    ctx.state::<ExpulsionState>().minion2 = minion;
                }
                // Every Varangka NPC in the zone runs at the attacker and hates them.
                for npc in zone_attackable_npcs(ctx) {
                    if (VARANGKA..=VARANGKA + 2).contains(&npc.id) {
                        ctx.set_running(npc);
                        ctx.world_mut()
                            .add_damage_hate(npc.object_id, attacker, 1, 99_999);
                        ctx.set_intention_attack(npc, attacker);
                    }
                }
            }
        } else if !ctx.has_quest_items(attacker, PROTECTION_SOULS_PENDANT) {
            ctx.start_quest_timer("zoneCheck", 1000, None, Some(attacker), false);
        }
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if ctx.world().player(creature).is_some() {
            ctx.start_quest_timer("zoneCheck", 1000, None, Some(creature), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = QUEST_NAME;

    const NPCS: &str = r#"
        <npc id="32655" type="Folk" level="1" name="Chairen"/>
        <npc id="22691" type="Monster" level="78" name="Ragna Orc"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14848" type="EtcItem" name="Protection of Souls Pendant"><set name="is_questitem" val="true"/></item>
        <item id="14881" type="EtcItem" name="Soul Core Containing Evil Spirit"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14882" type="EtcItem" name="Ragna Orc's Amulet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00311ExpulsionOfEvilSpirits);
        let pages = Pages::of(&Q00311ExpulsionOfEvilSpirits)
            .page("32655-01.htm", &["32655-03.htm"])
            .page("32655-03.htm", &["32655-04.htm"])
            .plain(&["32655-02.htm", "32655-04.htm", "32655-14.html"])
            .page("32655-12.html", &["32655-13.html", "32655-15.html"])
            .page("32655-05.html", &["32655-13.html", "32655-15.html"])
            .page("32655-06.html", &["32655-11.html"])
            .page("32655-11.html", &["32655-13.html", "32655-15.html"])
            .plain(&["32655-13.html", "32655-15.html"])
            .npcs(&[CHAIREN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let chairen = t.spawn(CHAIREN, 20);
        (t, chairen)
    }

    /// Kills Ragna Orcs until an amulet drops; returns the number of kills.
    fn hunt_amulet(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let orc = t.spawn(22691, 60);
            t.kill(1, orc);
            if t.count(1, RAGNA_ORCS_AMULET) >= 1 {
                return kills;
            }
        }
        panic!("no amulet after 400 kills");
    }

    #[test]
    fn a_level_seventy_nine_character_is_sent_away() {
        let (mut t, chairen) = setup();
        t.enter_as(1, 79, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, chairen).contains("32655-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_and_the_ragna_orcs() {
        let (mut t, chairen) = setup();
        t.enter_as(1, 80, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, chairen).contains("32655-01.htm"));
        assert!(t.event(1, NAME, "32655-03.htm").contains("32655-03.htm"));
        assert!(t.event(1, NAME, "32655-04.htm").contains("32655-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, chairen).contains("32655-05.html"));

        // An amulet from the Ragna Orcs, then the page with the exchange. Every kill writes the
        // memo state of slot 1 (`setMemoStateEx(1, count)`), and nothing else.
        let kills = hunt_amulet(&mut t);
        assert!(kills >= 1, "{kills}");
        let writes = t.take_writes();
        assert!(!writes.is_empty());
        assert!(writes.iter().all(|w| format!("{w:?}").contains("memoStateEx1")));
        assert!(t.quest_page(1, chairen).contains("32655-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_cores_make_the_pendant_and_ten_amulets_end_it() {
        let (mut t, chairen) = setup();
        t.enter_as(
            1,
            80,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 14881, 10), item(901, 14882, 10)],
        );
        assert!(t.quest_page(1, chairen).contains("32655-01.htm"));
        t.event(1, NAME, "32655-03.htm");
        t.event(1, NAME, "32655-04.htm");
        t.take_writes();

        // The page with ten cores: the pendant, the cores go.
        assert!(t.quest_page(1, chairen).contains("32655-06.html"));
        assert!(t.event(1, NAME, "32655-11.html").contains("32655-11.html"));
        assert_eq!((t.count(1, 14881), t.count(1, PROTECTION_SOULS_PENDANT)), (0, 1));
        assert!(t.take_writes().is_empty());

        // The end: no cores left and ten amulets: the quest ends repeatable.
        t.bypass(1, &format!("Script {NAME} 32655-13.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32655-13.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, RAGNA_ORCS_AMULET), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn nine_cores_are_not_enough_for_the_pendant() {
        let (mut t, chairen) = setup();
        t.enter_as(
            1,
            80,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 14881, 9)],
        );
        t.quest_page(1, chairen);
        t.event(1, NAME, "32655-03.htm");
        t.event(1, NAME, "32655-04.htm");
        t.take_writes();

        assert!(t.quest_page(1, chairen).contains("32655-06.html"));
        assert!(t.event(1, NAME, "32655-11.html").contains("32655-12.html"));
        assert_eq!((t.count(1, 14881), t.count(1, PROTECTION_SOULS_PENDANT)), (9, 0));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_end_is_refused_with_too_few_amulets() {
        let (mut t, chairen) = setup();
        t.enter_as(
            1,
            80,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 14882, 9)],
        );
        t.quest_page(1, chairen);
        t.event(1, NAME, "32655-03.htm");
        t.event(1, NAME, "32655-04.htm");
        t.take_writes();

        assert!(t.quest_page(1, chairen).contains("32655-06.html"));
        // No cores, so 11 shows 12 (the cores are short), whose link ends the quest: 13 is refused
        // with fewer than ten amulets, 14 says so and the quest goes on.
        assert!(t.event(1, NAME, "32655-11.html").contains("32655-12.html"));
        assert!(t.event(1, NAME, "32655-13.html").contains("32655-14.html"));
        assert_eq!(t.count(1, RAGNA_ORCS_AMULET), 9);
        assert!(t.take_writes().is_empty());
    }
}
