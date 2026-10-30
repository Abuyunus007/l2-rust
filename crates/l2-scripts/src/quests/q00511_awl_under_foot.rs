//! Quest 511: Awl Under Foot (the fortress wardens send a party into a dungeon of three raids).
// Java: dist/game/data/scripts/quests/Q00511_AwlUnderFoot/Q00511_AwlUnderFoot.java
//
// Pattern: an old-style instance quest (like Kamaloka, `instances/kamaloka.rs`): the quest makes
// the dynamic instance and its `InstanceWorld`, remembers which worlds are its own (Java's
// `FAUWorld`) in the script state, and spawns the raids with quest timers.

use std::collections::{BTreeMap, BTreeSet};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{InstanceWorld, Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::info;

/// Java `REENTERTIME`.
const REENTERTIME: i64 = 14_400_000;
/// Java `RAID_SPAWN_DELAY`.
const RAID_SPAWN_DELAY: i64 = 120_000;

// QUEST ITEMS
const DL_MARK: i32 = 9797;

// REWARDS
const KNIGHT_EPALUETTE: i32 = 9912;

// MONSTER TO KILL -- Only last 3 Raids (lvl ordered) give DL_MARK
const RAIDS1: [i32; 3] = [25572, 25575, 25578];
const RAIDS2: [i32; 4] = [25579, 25582, 25585, 25588];
const RAIDS3: [i32; 3] = [25589, 25592, 25593];

// Skill
const RAID_CURSE: SkillKey = SkillKey { id: 5456, level: 1 };

/// Java `_fortDungeons`: the warden and the instance template of its fortress dungeon.
const FORT_DUNGEONS: [(i32, i32); 21] = [
    (35666, 22),
    (35698, 23),
    (35735, 24),
    (35767, 25),
    (35804, 26),
    (35835, 27),
    (35867, 28),
    (35904, 29),
    (35936, 30),
    (35974, 31),
    (36011, 32),
    (36043, 33),
    (36081, 34),
    (36118, 35),
    (36149, 36),
    (36181, 37),
    (36219, 38),
    (36257, 39),
    (36294, 40),
    (36326, 41),
    (36364, 42),
];

/// What Java keeps in the fields of the quest: `FortDungeon._reEnterTime` per warden, and the
/// instance ids of the worlds that are `FAUWorld`s.
#[derive(Default)]
struct State {
    re_enter_time: BTreeMap<i32, i64>,
    worlds: BTreeSet<i32>,
}

/// Java `_fortDungeons.get(npcId).getInstanceId()`.
fn dungeon_of(npc_id: i32) -> Option<i32> {
    FORT_DUNGEONS.iter().find(|d| d.0 == npc_id).map(|d| d.1)
}

/// Java `Q00511_AwlUnderFoot`.
pub struct Q00511AwlUnderFoot;

impl Q00511AwlUnderFoot {
    /// Java `spawnRaid.run()`: the raid of the world's status, in the middle of the dungeon.
    fn spawn_raid(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        // The instance may be gone already (the party left and it ended).
        let Some(status) = ctx.world().instance_world(instance_id).map(InstanceWorld::status) else {
            return;
        };
        let list: &[i32] = match status {
            0 => &RAIDS1,
            1 => &RAIDS2,
            _ => &RAIDS3,
        };
        let Some(spawn_id) = ctx.get_random_entry(list) else {
            return;
        };

        if let Some(raid) = ctx.add_spawn(spawn_id, 53319, 245814, -6576, 0, false, 0, instance_id) {
            // Java `if (raid instanceof RaidBoss) setUseRaidCurse(false)`: every one of these is.
            ctx.world_mut().set_use_raid_curse(raid.object_id, false);
        }
    }

    /// Java `checkConditions(player)`: the party has to be right. `None` is Java's `null`.
    fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<String> {
        let Some(party) = ctx.world().party_of(player) else {
            return Some("FortressWarden-03.htm".to_owned());
        };

        let leader = ctx.world().party_leader(party);
        if leader != Some(player) {
            let name = leader.map(|l| ctx.player_name(l)).unwrap_or_default();
            return Some(
                ctx.get_htm(player, "FortressWarden-04.htm")
                    .unwrap_or_default()
                    .replace("%leader%", &name),
            );
        }

        for party_member in ctx.world().party_members(party) {
            let qs = ctx.get_quest_state(party_member, false);
            if qs.is_none_or(|qs| qs.get_cond(ctx) < 1) {
                let name = ctx.player_name(party_member);
                return Some(
                    ctx.get_htm(player, "FortressWarden-05.htm")
                        .unwrap_or_default()
                        .replace("%player%", &name),
                );
            }

            if !ctx.check_if_in_range(1000, player, party_member, true) {
                let name = ctx.player_name(party_member);
                return Some(
                    ctx.get_htm(player, "FortressWarden-06.htm")
                        .unwrap_or_default()
                        .replace("%player%", &name),
                );
            }
        }

        None
    }

    /// Java `checkFortCondition(player, npc, isEnter)`. `None` is Java's `null`.
    fn check_fort_condition(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        is_enter: bool,
    ) -> Option<String> {
        let fortress = ctx.world().npc_fort(npc.object_id);
        let dungeon = dungeon_of(npc.id);
        let (Some(fortress), Some(_dungeon)) = (fortress, dungeon) else {
            return Some("FortressWarden-01.htm".to_owned());
        };

        let Some(clan) = ctx.world().clan_of(player) else {
            return Some("FortressWarden-01.htm".to_owned());
        };
        if clan.fort_id != fortress.0 {
            return Some("FortressWarden-01.htm".to_owned());
        }

        let fort_state = ctx.world().fort(fortress).map_or(0, |f| f.state);
        if fort_state == 0 {
            return Some("FortressWarden-02a.htm".to_owned());
        } else if fort_state == 2 {
            return Some("FortressWarden-02b.htm".to_owned());
        } else if is_enter
            && ctx.state::<State>().re_enter_time.get(&npc.id).copied().unwrap_or(0) > ctx.now_ms()
        {
            return Some("FortressWarden-07.htm".to_owned());
        }

        let Some(party) = ctx.world().party_of(player) else {
            return Some("FortressWarden-03.htm".to_owned());
        };

        for party_member in ctx.world().party_members(party) {
            let fort_id = ctx.world().clan_of(party_member).map(|c| c.fort_id);
            if fort_id.is_none_or(|f| f == 0 || f != fortress.0) {
                let name = ctx.player_name(party_member);
                return Some(
                    ctx.get_htm(player, "FortressWarden-05.htm")
                        .unwrap_or_default()
                        .replace("%player%", &name),
                );
            }
        }

        None
    }

    /// Java `enterInstance(player, coords, dungeon, retValue)`.
    fn enter_instance(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        coords: (i32, i32, i32),
        npc_id: i32,
        ret_value: Option<String>,
    ) -> Option<String> {
        // check for existing instances for this player
        let existing = ctx
            .world()
            .player_instance_world(player)
            .map(|w| w.instance_id);

        // existing instance
        if let Some(instance_id) = existing {
            if !ctx.state::<State>().worlds.contains(&instance_id) {
                ctx.world().send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                    ),
                );
                return Some(String::new());
            }

            ctx.teleport_player(player, coords.0, coords.1, coords.2, instance_id, false);
            return Some(String::new());
        }

        // New instance
        if ret_value.is_some() {
            return ret_value;
        }

        let ret = Self::check_conditions(ctx, player);
        if ret.is_some() {
            return ret;
        }

        // Java dereferences `dungeon` here: `checkFortCondition` refused every other warden.
        let template_id = dungeon_of(npc_id)?;
        let instance_id = ctx.world_mut().create_dynamic_instance(template_id);
        let created_template = ctx.world().instance(instance_id).map_or(-1, |i| i.template_id);
        ctx.world_mut()
            .add_instance_world(InstanceWorld::new(instance_id, created_template));
        ctx.state::<State>().worlds.insert(instance_id);
        let exit = ctx.world().position_of(player);
        if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
            instance.exit_location = exit;
        }
        let re_enter = ctx.now_ms() + REENTERTIME;
        ctx.state::<State>().re_enter_time.insert(npc_id, re_enter);
        info!(
            "Fortress AwlUnderFoot started {template_id} Instance: {instance_id} created by player: {}",
            ctx.player_name(player)
        );
        ctx.start_quest_timer(
            &format!("SPAWN_RAID_{instance_id}"),
            RAID_SPAWN_DELAY,
            None,
            None,
            false,
        );

        // teleport players
        match ctx.world().party_of(player) {
            None => {
                ctx.teleport_player(player, coords.0, coords.1, coords.2, instance_id, false);
                if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                    world.add_allowed(player);
                }
            }
            Some(party) => {
                for party_member in ctx.world().party_members(party) {
                    ctx.teleport_player(party_member, coords.0, coords.1, coords.2, instance_id, false);
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.add_allowed(party_member);
                    }
                    ctx.get_quest_state(party_member, true);
                }
            }
        }

        // Java `player.getClan().getName()`: the clan exists (`checkFortCondition` checked it).
        let clan = ctx.world().clan_of(player).map(|c| c.name.clone()).unwrap_or_default();
        ctx.get_htm(player, "FortressWarden-08.htm")
            .map(|page| page.replace("%clan%", &clan))
    }

    /// Java `rewardPlayer(player)`.
    fn reward_player(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            ctx.give_items(player, DL_MARK, 140);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }

    /// The part of Java's `onTalk` after the fortress checks.
    fn talk(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;
        let cond = qs.get_cond(ctx);
        if dungeon_of(npc_id).is_some() && cond == 0 {
            if ctx.player_level(player) >= 60 {
                htmltext = "FortressWarden-09.htm".to_owned();
            } else {
                htmltext = "FortressWarden-00.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            }
        } else if dungeon_of(npc_id).is_some() && cond > 0 && qs.state(ctx) == STARTED {
            let count = ctx.get_quest_items_count(player, DL_MARK);
            if cond == 1 && count > 0 {
                htmltext = "FortressWarden-14.htm".to_owned();
                ctx.take_items(player, DL_MARK, -1);
                ctx.reward_items(player, KNIGHT_EPALUETTE, count);
            } else if cond == 1 && count == 0 {
                htmltext = "FortressWarden-10.htm".to_owned();
            }
        }
        Some(htmltext)
    }
}

impl Script for Q00511AwlUnderFoot {
    fn name(&self) -> &'static str {
        "Q00511_AwlUnderFoot"
    }

    fn path(&self) -> &'static str {
        "quests/Q00511_AwlUnderFoot"
    }

    fn id(&self) -> i32 {
        511
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let wardens: Vec<i32> = FORT_DUNGEONS.iter().map(|d| d.0).collect();
        s.add_start_npc(&wardens);
        s.add_talk_id(&wardens);

        s.add_kill_id(&RAIDS1);
        s.add_kill_id(&RAIDS2);
        s.add_kill_id(&RAIDS3);

        let raids: Vec<i32> = (25572..=25595).collect();
        s.add_attack_id(&raids);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // The spawn of a raid is Java's `ThreadPool.schedule(new spawnRaid(world), ...)`: a timer
        // without NPC and player (a player's bypass always has a player).
        if player.is_none()
            && let Some(instance_id) = event.strip_prefix("SPAWN_RAID_").and_then(|i| i.parse().ok())
        {
            Self::spawn_raid(ctx, instance_id);
            return None;
        }

        let player = player?;
        let htmltext = Some(event.to_owned());
        if event.eq_ignore_ascii_case("enter") {
            let npc = npc?;
            let tele = (53322, 246380, -6580);
            let ret = Self::check_fort_condition(ctx, player, npc, true);
            return Self::enter_instance(ctx, player, tele, npc.id, ret);
        }

        let qs = ctx.get_quest_state(player, true)?;
        if event.eq_ignore_ascii_case("FortressWarden-10.htm") {
            if qs.is_cond(ctx, 0) {
                qs.start_quest(ctx);
            }
        } else if event.eq_ignore_ascii_case("FortressWarden-15.htm") {
            qs.exit_quest(ctx, true, true);
        }

        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        let attacker = if is_summon {
            let Some(summon) = ctx.summon_of(player) else {
                return;
            };
            summon
        } else {
            player
        };
        let level_of = |ctx: &ScriptCtx<'_>, id: ObjectId| ctx.world().stats(id).map_or(0, |s| s.level());
        let buffed = |ctx: &ScriptCtx<'_>, id: ObjectId| {
            ctx.world()
                .effect_list(id)
                .is_some_and(|l| l.buff_count() > 0 || l.dance_count() > 0)
        };
        if level_of(ctx, attacker) - level_of(ctx, npc.object_id) >= 9 {
            if buffed(ctx, attacker) {
                ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
                ctx.world_mut().do_simultaneous_cast(npc.object_id, RAID_CURSE);
            } else if ctx.world().party_of(player).is_some() {
                for pmember in ctx.world().party_members_of(player) {
                    if buffed(ctx, pmember) {
                        ctx.world_mut().set_npc_target(npc.object_id, Some(pmember));
                        ctx.world_mut().do_simultaneous_cast(npc.object_id, RAID_CURSE);
                    }
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        // Java `InstanceManager.getWorld(npc) instanceof FAUWorld`.
        let Some(instance_id) = ctx.world().instance_world_of(npc.object_id).map(|w| w.instance_id) else {
            return;
        };
        if !ctx.state::<State>().worlds.contains(&instance_id) {
            return;
        }

        if RAIDS3.contains(&npc.id) {
            if ctx.world().party_of(player).is_some() {
                for pl in ctx.world().party_members_of(player) {
                    Self::reward_player(ctx, pl);
                }
            } else {
                Self::reward_player(ctx, player);
            }

            ctx.world_mut().set_instance_duration(instance_id, 360_000);
            ctx.world_mut().remove_instance_npcs(instance_id);
        } else {
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.inc_status();
            }
            ctx.start_quest_timer(
                &format!("SPAWN_RAID_{instance_id}"),
                RAID_SPAWN_DELAY,
                None,
                None,
                false,
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let ret = Self::check_fort_condition(ctx, player, npc, false);
        if ret.is_some() {
            return ret;
        }

        Self::talk(ctx, npc, player)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00511_AwlUnderFoot";

    const NPCS: &str = r#"
        <npc id="35666" type="Folk" level="70" name="Fortress Warden"/>
        <npc id="25572" type="RaidBoss" level="70" name="Raid 1"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25575" type="RaidBoss" level="70" name="Raid 1b"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25578" type="RaidBoss" level="70" name="Raid 1c"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25579" type="RaidBoss" level="70" name="Raid 2"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25582" type="RaidBoss" level="70" name="Raid 2b"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25585" type="RaidBoss" level="70" name="Raid 2c"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25588" type="RaidBoss" level="70" name="Raid 2d"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25589" type="RaidBoss" level="70" name="Raid 3"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25592" type="RaidBoss" level="70" name="Raid 3b"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25593" type="RaidBoss" level="70" name="Raid 3c"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9797" type="EtcItem" name="Mark"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9912" type="EtcItem" name="Knight's Epaulette"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00511AwlUnderFoot);
        let pages = Pages::of(&Q00511AwlUnderFoot)
            .page("FortressWarden-09.htm", &["FortressWarden-10.htm"])
            .page("FortressWarden-14.htm", &["FortressWarden-15.htm"])
            .plain(&[
                "FortressWarden-00.htm",
                "FortressWarden-01.htm",
                "FortressWarden-03.htm",
                "FortressWarden-10.htm",
                "FortressWarden-15.htm",
            ])
            .npcs(&[35666]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let warden = t.spawn(35666, 20);
        (t, warden)
    }

    /// A dungeon world of the quest with the character in it (the entry needs a fortress).
    fn dungeon(t: &mut ScriptTest) -> i32 {
        let mut instance_id = 0;
        t.world.run_as_script(NAME, |ctx| {
            instance_id = ctx.world_mut().create_dynamic_instance(0);
            ctx.world_mut().add_instance_world(InstanceWorld::new(instance_id, 22));
            ctx.state::<State>().worlds.insert(instance_id);
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.add_allowed(Oid(1));
            }
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
        });
        instance_id
    }

    #[test]
    fn a_warden_without_a_fortress_refuses_everyone() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, warden).contains("FortressWarden-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_talk_after_the_fortress_check() {
        let (mut t, warden) = setup();
        t.enter(1, 59, vec![]);
        let npc = ScriptNpc {
            object_id: warden,
            id: 35666,
        };
        // Level 59: refused, and the (created) state is dropped.
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00511AwlUnderFoot::talk(ctx, npc, Oid(1)),
                Some("FortressWarden-00.htm".to_owned())
            );
        });
        t.enter(2, 60, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00511AwlUnderFoot::talk(ctx, npc, Oid(2)),
                Some("FortressWarden-09.htm".to_owned())
            );
        });
        // The accept link starts the quest at cond 0 -> 1.
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00511AwlUnderFoot.on_event(ctx, "FortressWarden-10.htm", Some(npc), Some(Oid(2)));
            assert_eq!(page, Some("FortressWarden-10.htm".to_owned()));
        });
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::of(2, rows::insert(NAME, "<state>", "Started"))),
            "{writes:?}"
        );
        // No marks yet: the "go on" page.
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00511AwlUnderFoot::talk(ctx, npc, Oid(2)),
                Some("FortressWarden-10.htm".to_owned())
            );
        });
    }

    #[test]
    fn a_player_without_a_party_is_asked_for_one() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00511AwlUnderFoot::check_conditions(ctx, Oid(1)),
                Some("FortressWarden-03.htm".to_owned())
            );
        });
    }

    #[test]
    fn the_raids_come_one_after_the_other_and_the_last_pays_140_marks() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        let instance_id = dungeon(&mut t);
        t.take_writes();

        // The first raid appears after two minutes.
        t.world.run_as_script(NAME, |ctx| {
            ctx.start_quest_timer(&format!("SPAWN_RAID_{instance_id}"), RAID_SPAWN_DELAY, None, None, false);
        });
        t.wait_ms(120_100);
        let first: Vec<_> = RAIDS1.iter().flat_map(|id| t.npcs_of(*id)).collect();
        assert_eq!(first.len(), 1);
        assert!(!t.world.gives_raid_curse(first[0]));

        // Its death raises the status and starts the next timer; the second raid is from RAIDS2.
        t.kill(1, first[0]);
        t.wait_ms(120_100);
        let second: Vec<_> = RAIDS2.iter().flat_map(|id| t.npcs_of(*id)).collect();
        assert_eq!(second.len(), 1);
        t.kill(1, second[0]);
        t.wait_ms(120_100);
        let third: Vec<_> = RAIDS3.iter().flat_map(|id| t.npcs_of(*id)).collect();
        assert_eq!(third.len(), 1);

        // The last one: 140 marks for the player at cond 1, the instance ends in 6 minutes.
        t.kill(1, third[0]);
        assert_eq!(t.count(1, DL_MARK), 140);

        // The warden turns the marks into epaulettes (rates 1).
        let npc = ScriptNpc {
            object_id: warden,
            id: 35666,
        };
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00511AwlUnderFoot::talk(ctx, npc, Oid(1)),
                Some("FortressWarden-14.htm".to_owned())
            );
        });
        assert_eq!((t.count(1, KNIGHT_EPALUETTE), t.count(1, DL_MARK)), (140, 0));
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00511AwlUnderFoot.on_event(ctx, "FortressWarden-15.htm", Some(npc), Some(Oid(1)));
            assert_eq!(page, Some("FortressWarden-15.htm".to_owned()));
        });
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_stray_raid_outside_the_quests_worlds_does_nothing() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        let raid = t.spawn(25589, 60);
        t.kill(1, raid);
        assert_eq!(t.count(1, DL_MARK), 0);
        assert!(t.npcs_of(25589).len() <= 1);
    }

    #[test]
    fn a_bypass_cannot_fire_the_raid_timer() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        let instance_id = dungeon(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00511AwlUnderFoot.on_event(ctx, &format!("SPAWN_RAID_{instance_id}"), None, Some(Oid(1)));
            // Any other event is echoed, as in Java; but no raid appears.
            assert_eq!(page, Some(format!("SPAWN_RAID_{instance_id}")));
        });
        let raids: Vec<_> = RAIDS1.iter().flat_map(|id| t.npcs_of(*id)).collect();
        assert!(raids.is_empty());
    }
}
