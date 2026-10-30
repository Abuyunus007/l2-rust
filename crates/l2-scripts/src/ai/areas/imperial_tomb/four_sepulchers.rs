//! Four Sepulchers: the four chapels of the Imperial Tomb. A party of four enters from a manager,
//! fights through the rooms (doors, chests, keys) and kills the boss for a goblet (quest 620).
// Java: dist/game/data/scripts/ai/areas/ImperialTomb/FourSepulchers/FourSepulchers.java
//
// Java's `static` maps (`STORED_PROGRESS`, `STORED_MONSTER_SPAWNS`) are the script's state
// (`ctx.state`). `ThreadPool.schedule` is a quest timer: the delayed door closing is the timer
// `CLOSE_DOOR_<id>`, the oust at the end of the event the timer `OUST` of the manager. The entry
// hours use the system clock like Java (`System.currentTimeMillis()`), in UTC minutes.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};
use tracing::warn;

use super::four_sepulchers_data::ROOM_SPAWN_DATA;

// NPCs
const CONQUEROR_MANAGER: i32 = 31921;
const EMPEROR_MANAGER: i32 = 31922;
const GREAT_SAGES_MANAGER: i32 = 31923;
const JUDGE_MANAGER: i32 = 31924;
const MYSTERIOUS_CHEST: i32 = 31468;
const KEY_CHEST: i32 = 31467;
const ROOM_3_VICTIM: i32 = 18150;
const ROOM_3_CHEST_REWARDER: i32 = 18158;
const ROOM_4_CHARM_1: i32 = 18196;
const ROOM_4_CHARM_2: i32 = 18197;
const ROOM_4_CHARM_3: i32 = 18198;
const ROOM_4_CHARM_4: i32 = 18199;
const ROOM_5_STATUE_GUARD: i32 = 18232;
const ROOM_6_REWARD_CHEST: i32 = 18256;
const CONQUEROR_BOSS: i32 = 25346;
const EMPEROR_BOSS: i32 = 25342;
const GREAT_SAGES_BOSS: i32 = 25339;
const JUDGE_BOSS: i32 = 25349;
const TELEPORTER: i32 = 31452;
const FIRST_TALK_NPCS: [i32; 25] = [
    TELEPORTER, 31453, 31454, 31919, 31920, 31925, 31926, 31927, 31928, 31929, 31930, 31931, 31932, 31933,
    31934, 31935, 31936, 31937, 31938, 31939, 31940, 31941, 31942, 31943, 31944,
];
const CHEST_REWARD_MONSTERS: [i32; 4] = [
    18120, // room 1
    ROOM_3_CHEST_REWARDER,
    18177, // room 4
    18212, // room 5 - wave 2
];

// Items
const ENTRANCE_PASS: i32 = 7075;
const USED_PASS: i32 = 7261;
const CHAPEL_KEY: i32 = 7260;
const ANTIQUE_BROOCH: i32 = 7262;

/// Java `Q00620_FourGoblets.class.getSimpleName()`.
const FOUR_GOBLETS: &str = "Q00620_FourGoblets";

// Locations
const fn start_hall_spawn(manager: i32) -> Option<(i32, i32, i32)> {
    match manager {
        CONQUEROR_MANAGER => Some((181632, -85587, -7218)),
        EMPEROR_MANAGER => Some((179963, -88978, -7218)),
        GREAT_SAGES_MANAGER => Some((173217, -86132, -7218)),
        JUDGE_MANAGER => Some((175608, -82296, -7218)),
        _ => None,
    }
}

// Zones
const CONQUEROR_ZONE: i32 = 200221;
const EMPEROR_ZONE: i32 = 200222;
const GREAT_SAGES_ZONE: i32 = 200224;
const JUDGE_ZONE: i32 = 200223;
/// Java `MANAGER_ZONES`, in its `HashMap` order of the keys 31921..31924.
const MANAGER_ZONES: [(i32, i32); 4] = [
    (CONQUEROR_MANAGER, CONQUEROR_ZONE),
    (EMPEROR_MANAGER, EMPEROR_ZONE),
    (GREAT_SAGES_MANAGER, GREAT_SAGES_ZONE),
    (JUDGE_MANAGER, JUDGE_ZONE),
];
const SCRIPT_ZONE: &str = "ScriptZone";

fn zone_of_manager(manager: i32) -> Option<i32> {
    MANAGER_ZONES.iter().find(|(m, _)| *m == manager).map(|(_, z)| *z)
}

// Spawns: `(sepulcherId, roomNumber, x, y, z, heading)`
const CHEST_SPAWN_LOCATIONS: [(i32, i32, i32, i32, i32, i32); 20] = [
    (1, 1, 182074, -85579, -7216, 32768),
    (1, 2, 183868, -85577, -7216, 32768),
    (1, 3, 185681, -85573, -7216, 32768),
    (1, 4, 187498, -85566, -7216, 32768),
    (1, 5, 189306, -85571, -7216, 32768),
    (2, 1, 180375, -88968, -7216, 32768),
    (2, 2, 182151, -88962, -7216, 32768),
    (2, 3, 183960, -88964, -7216, 32768),
    (2, 4, 185792, -88966, -7216, 32768),
    (2, 5, 187625, -88953, -7216, 32768),
    (3, 1, 173218, -85703, -7216, 49152),
    (3, 2, 173206, -83929, -7216, 49152),
    (3, 3, 173208, -82085, -7216, 49152),
    (3, 4, 173191, -80290, -7216, 49152),
    (3, 5, 173198, -78465, -7216, 49152),
    (4, 1, 175601, -81905, -7216, 49152),
    (4, 2, 175619, -80094, -7216, 49152),
    (4, 3, 175608, -78268, -7216, 49152),
    (4, 4, 175588, -76472, -7216, 49152),
    (4, 5, 175594, -74655, -7216, 49152),
];

// Doors: `(sepulcherId, waveNumber, doorId)`
const DOORS: [(i32, i32, i32); 20] = [
    (1, 2, 25150012),
    (1, 3, 25150013),
    (1, 4, 25150014),
    (1, 5, 25150015),
    (1, 7, 25150016),
    (2, 2, 25150002),
    (2, 3, 25150003),
    (2, 4, 25150004),
    (2, 5, 25150005),
    (2, 7, 25150006),
    (3, 2, 25150032),
    (3, 3, 25150033),
    (3, 4, 25150034),
    (3, 5, 25150035),
    (3, 7, 25150036),
    (4, 2, 25150022),
    (4, 3, 25150023),
    (4, 4, 25150024),
    (4, 5, 25150025),
    (4, 7, 25150026),
];

// Skill
const PETRIFY: SkillKey = SkillKey { id: 4616, level: 1 };

fn charm_skill(npc_id: i32) -> i32 {
    match npc_id {
        ROOM_4_CHARM_1 => 4146,
        ROOM_4_CHARM_2 => 4145,
        ROOM_4_CHARM_3 => 4148,
        _ => 4624,
    }
}

// Misc
fn charm_msg(npc_id: i32) -> NpcStringId {
    match npc_id {
        ROOM_4_CHARM_1 => NpcStringId::ThePAtkReductionDeviceHasNowBeenDestroyed,
        ROOM_4_CHARM_2 => NpcStringId::TheDefenseReductionDeviceHasBeenDestroyed,
        // TODO (Java): THE_HP_REGENERATION_REDUCTION_DEVICE_WILL_BE_ACTIVATED_IN_3_MINUTES2 for charm 4.
        _ => NpcStringId::ThePoisonDeviceHasNowBeenDestroyed,
    }
}

const VICTIM_MSG: [NpcStringId; 3] = [
    NpcStringId::HelpMe,
    NpcStringId::DonTMiss,
    NpcStringId::KeepPushing,
];
const MANAGER_MESSAGES: [NpcStringId; 2] = [
    NpcStringId::YouMayNowEnterTheSepulcher,
    NpcStringId::IfYouPlaceYourHandOnTheStoneStatueInFrontOfEachSepulcherYouWillBeAbleToEnter,
];
const PARTY_MEMBER_COUNT: usize = 4;
const ENTRY_TIME: i64 = 55; // Minute for entry.
const EVENT_TIME: i64 = 50; // Minutes for the event.

/// Java `STORED_PROGRESS` and `STORED_MONSTER_SPAWNS` (index 1..=4; 0 is unused).
pub struct SepulchersState {
    progress: [i32; 5],
    monsters: [Vec<ObjectId>; 5],
}

impl Default for SepulchersState {
    fn default() -> Self {
        Self {
            progress: [0, 1, 1, 1, 1],
            monsters: Default::default(),
        }
    }
}

/// Java `getSepulcherId(player)`: 0 for a player in no sepulcher.
fn get_sepulcher_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    for (id, zone) in [
        (1, CONQUEROR_ZONE),
        (2, EMPEROR_ZONE),
        (3, GREAT_SAGES_ZONE),
        (4, JUDGE_ZONE),
    ] {
        if ctx
            .world()
            .zone_players_inside(zone, SCRIPT_ZONE)
            .contains(&player)
        {
            return id;
        }
    }
    0
}

/// `STORED_PROGRESS.get(sepulcherId)`: `None` for 0, where Java throws.
fn progress(ctx: &mut ScriptCtx<'_>, sepulcher_id: i32) -> Option<i32> {
    usize::try_from(sepulcher_id)
        .ok()
        .filter(|i| (1..=4).contains(i))
        .map(|i| ctx.state::<SepulchersState>().progress[i])
}

fn set_progress(ctx: &mut ScriptCtx<'_>, sepulcher_id: i32, value: i32) {
    if let Ok(i) = usize::try_from(sepulcher_id)
        && (1..=4).contains(&i)
    {
        ctx.state::<SepulchersState>().progress[i] = value;
    }
}

fn monsters(ctx: &mut ScriptCtx<'_>, sepulcher_id: i32) -> Vec<ObjectId> {
    usize::try_from(sepulcher_id)
        .ok()
        .filter(|i| (1..=4).contains(i))
        .map(|i| ctx.state::<SepulchersState>().monsters[i].clone())
        .unwrap_or_default()
}

fn set_monsters(ctx: &mut ScriptCtx<'_>, sepulcher_id: i32, list: Vec<ObjectId>) {
    if let Ok(i) = usize::try_from(sepulcher_id)
        && (1..=4).contains(&i)
    {
        ctx.state::<SepulchersState>().monsters[i] = list;
    }
}

/// Java `Q00620_FourGoblets` state started / completed of a player.
fn quest_started(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<bool> {
    ctx.quest_state_of(FOUR_GOBLETS, player, false)
        .map(|qs| qs.is_started(ctx))
}

/// Java `showHtmlFile(player, file, npc, member)`.
fn show_html_file(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    file: &str,
    npc: ScriptNpc,
    member: Option<ObjectId>,
) {
    let Some(mut html) = ctx.get_htm(player, file) else {
        return;
    };
    if let Some(member) = member {
        let name = ctx
            .world()
            .player(member)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        html = html.replace("%member%", &name);
    }
    ctx.show_result(player, Some(&html), Some(npc));
}

/// Milliseconds to the next `ENTRY_TIME` minute of an hour, from the clock `now_ms`.
fn ms_until_announce(now_ms: i64) -> i64 {
    let minute = (now_ms / 60_000).rem_euclid(60);
    let into_minute = now_ms.rem_euclid(60_000);
    let minutes = if minute >= ENTRY_TIME {
        60 - minute + ENTRY_TIME
    } else {
        ENTRY_TIME - minute
    };
    minutes * 60_000 - into_minute
}

/// Java `spawnMysteriousChest(player)`.
fn spawn_mysterious_chest(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let sepulcher_id = get_sepulcher_id(ctx, player);
    let Some(current_wave) = progress(ctx, sepulcher_id) else {
        return;
    };
    for (sepulcher, room, x, y, z, heading) in CHEST_SPAWN_LOCATIONS {
        if sepulcher == sepulcher_id && room == current_wave {
            ctx.add_spawn(MYSTERIOUS_CHEST, x, y, z, heading, false, 0, 0);
            break;
        }
    }
}

/// Java `spawnKeyChest(player, loc)`.
fn spawn_key_chest(ctx: &mut ScriptCtx<'_>, player: ObjectId, loc: Option<(i32, i32, i32, i32)>) {
    match loc {
        Some((x, y, z, heading)) => {
            ctx.add_spawn(KEY_CHEST, x, y, z, heading, false, 0, 0);
        }
        None => {
            ctx.add_spawn_at(KEY_CHEST, player, false, 0);
        }
    }
}

/// Java `spawnNextWave(player)`.
fn spawn_next_wave(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let sepulcher_id = get_sepulcher_id(ctx, player);
    let Some(current_wave) = progress(ctx, sepulcher_id) else {
        return;
    };
    let mut list = monsters(ctx, sepulcher_id);
    for &(sepulcher, wave, npc_id, x, y, z, heading) in ROOM_SPAWN_DATA.iter() {
        if sepulcher == sepulcher_id
            && wave == current_wave
            && let Some(npc) = ctx.add_spawn(npc_id, x, y, z, heading, false, 0, 0)
        {
            list.push(npc.object_id);
        }
    }
    set_monsters(ctx, sepulcher_id, list);

    if current_wave == 4
        && let Some((x, y, z)) = ctx.world().position_of(player)
    {
        for zone in ctx.world().script_effect_zones_at(x, y, z) {
            ctx.world_mut().set_effect_zone_enabled(zone, true);
        }
    }

    if current_wave == 2 || current_wave == 5 {
        ctx.start_quest_timer("WAVE_DEFEATED_CHECK", 5000, None, Some(player), false);
    } else {
        // No need to check these waves.
        set_monsters(ctx, sepulcher_id, Vec::new());
    }
}

/// Java `tryEnter(npc, player)`; `current_minute` is the minute of the hour.
fn try_enter(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, current_minute: i64) {
    let npc_id = npc.id;
    let Some(party) = ctx.world().party_of(player) else {
        show_html_file(ctx, player, &format!("{npc_id}-SP.html"), npc, None);
        return;
    };

    if !ctx.world().is_party_leader(player) {
        show_html_file(ctx, player, &format!("{npc_id}-NL.html"), npc, None);
        return;
    }

    let Some(zone) = zone_of_manager(npc_id) else {
        return;
    };
    if !ctx.world().zone_players_inside(zone, SCRIPT_ZONE).is_empty() {
        show_html_file(ctx, player, &format!("{npc_id}-FULL.html"), npc, None);
        return;
    }

    let members = ctx.world().party_members(party);
    if members.len() < PARTY_MEMBER_COUNT {
        show_html_file(ctx, player, &format!("{npc_id}-SP.html"), npc, None);
        return;
    }

    // Get current time in minutes for entering.
    if (0..ENTRY_TIME).contains(&current_minute) {
        show_html_file(ctx, player, &format!("{npc_id}-NE.html"), npc, None);
        return;
    }

    // Set the timer to spawn the chest when event start.
    let time_until_next_hour = (60 - current_minute) * 60 * 1000;
    ctx.start_quest_timer(
        "SPAWN_MYSTERIOUS_CHEST",
        time_until_next_hour,
        Some(npc),
        Some(player),
        false,
    );

    for &mem in &members {
        let qs = ctx.quest_state_of(FOUR_GOBLETS, mem, false);
        if qs.is_none_or(|qs| !qs.is_started(ctx) && !qs.is_completed(ctx)) {
            show_html_file(ctx, player, &format!("{npc_id}-NS.html"), npc, Some(mem));
            return;
        }

        if !ctx.has_quest_items(mem, ENTRANCE_PASS) {
            show_html_file(ctx, player, &format!("{npc_id}-SE.html"), npc, Some(mem));
            return;
        }

        // NOTE(java-quirk): the weight penalty checked is the leader's, for every member.
        if ctx.world().player(player).is_some_and(|p| p.weight_penalty >= 3) {
            ctx.send_system_message(
                mem,
                l2_protocol_game::SystemMessageId::UnableToProcessThisRequestUntilYourInventorySWeightAndSlotCountAreLessThan80PercentOfCapacity,
            );
            return;
        }
    }

    let sepulcher_id = get_sepulcher_id(ctx, player);

    // Delete any existing spawns.
    for creature in ctx.world().zone_characters_inside(zone, SCRIPT_ZONE) {
        let Some(n) = ctx.script_npc(creature) else {
            continue;
        };
        if ctx.world().is_monster_npc(creature)
            || ctx.world().script_is_raid(creature)
            || n.id == MYSTERIOUS_CHEST
            || n.id == KEY_CHEST
            || n.id == TELEPORTER
        {
            ctx.delete_npc(n);
        }
    }

    // Disable EffectZones.
    for (sepulcher, room, x, y, z, _) in CHEST_SPAWN_LOCATIONS {
        if sepulcher == sepulcher_id && room == 4 {
            for effect_zone in ctx.world().script_effect_zones_at(x, y, z) {
                ctx.world_mut().set_effect_zone_enabled(effect_zone, false);
            }
            break;
        }
    }

    // Close all doors.
    for (sepulcher, _, door) in DOORS {
        if sepulcher == sepulcher_id {
            ctx.close_door(door, 0);
        }
    }

    // Teleport players inside.
    let party_members: Vec<ObjectId> = members
        .into_iter()
        .filter(|mem| ctx.check_if_in_range(700, player, *mem, true))
        .collect();
    for mem in party_members {
        if let Some((x, y, z)) = start_hall_spawn(npc_id) {
            ctx.teleport_player(mem, x, y, z, 0, true);
        }
        ctx.take_items(mem, ENTRANCE_PASS, 1);
        ctx.take_items(mem, CHAPEL_KEY, -1);
        if !ctx.has_quest_items(mem, ANTIQUE_BROOCH) {
            ctx.give_items(mem, USED_PASS, 1);
        }
    }

    show_html_file(ctx, player, &format!("{npc_id}-OK.html"), npc, None);

    // Kick all players when/if time is over.
    ctx.start_quest_timer("OUST", EVENT_TIME * 60 * 1000, Some(npc), None, false);

    // Init progress.
    set_progress(ctx, sepulcher_id, 1); // Start from 1.
}

/// Java `ai.areas.ImperialTomb.FourSepulchers.FourSepulchers`.
pub struct FourSepulchers;

impl Script for FourSepulchers {
    fn name(&self) -> &'static str {
        "FourSepulchers"
    }

    fn path(&self) -> &'static str {
        "ai/areas/ImperialTomb/FourSepulchers"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let managers = [
            CONQUEROR_MANAGER,
            EMPEROR_MANAGER,
            GREAT_SAGES_MANAGER,
            JUDGE_MANAGER,
            MYSTERIOUS_CHEST,
            KEY_CHEST,
        ];
        s.add_first_talk_id(&managers);
        s.add_talk_id(&managers);
        s.add_first_talk_id(&FIRST_TALK_NPCS);
        s.add_talk_id(&FIRST_TALK_NPCS);
        s.add_kill_id(&CHEST_REWARD_MONSTERS);
        let kills = [
            ROOM_3_VICTIM,
            ROOM_4_CHARM_1,
            ROOM_4_CHARM_2,
            ROOM_4_CHARM_3,
            ROOM_4_CHARM_4,
            ROOM_6_REWARD_CHEST,
            CONQUEROR_BOSS,
            EMPEROR_BOSS,
            GREAT_SAGES_BOSS,
            JUDGE_BOSS,
        ];
        s.add_kill_id(&kills);
        s.add_spawn_id(&[
            ROOM_3_VICTIM,
            ROOM_4_CHARM_1,
            ROOM_4_CHARM_2,
            ROOM_4_CHARM_3,
            ROOM_4_CHARM_4,
            ROOM_5_STATUE_GUARD,
            ROOM_6_REWARD_CHEST,
            CONQUEROR_BOSS,
            EMPEROR_BOSS,
            GREAT_SAGES_BOSS,
            JUDGE_BOSS,
        ]);
    }

    // The end of the Java constructor: the first announcement.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let delay = ms_until_announce(ctx.now_ms());
        ctx.start_quest_timer("ANNOUNCE", delay, None, None, false);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = Some(event.to_owned());

        if let Some(door) = event.strip_prefix("CLOSE_DOOR_") {
            if let Ok(door) = door.parse::<i32>() {
                ctx.close_door(door, 0);
            }
            return None;
        }

        match event {
            "ENTER" => {
                let (npc, player) = (npc?, player?);
                match quest_started(ctx, player) {
                    None => return Some(ctx.get_no_quest_msg(player)),
                    Some(true) => {
                        let minute = (ctx.now_ms() / 60_000).rem_euclid(60);
                        try_enter(ctx, npc, player, minute);
                        return None;
                    }
                    Some(false) => {}
                }
            }
            "OPEN_GATE" => {
                let (npc, player) = (npc?, player?);
                let Some(started) = quest_started(ctx, player) else {
                    return Some(ctx.get_no_quest_msg(player));
                };

                if started && ctx.is_script_value(npc, 0) {
                    if ctx.has_quest_items(player, CHAPEL_KEY) {
                        ctx.set_script_value(npc, 1);
                        ctx.take_items(player, CHAPEL_KEY, -1);
                        let sepulcher_id = get_sepulcher_id(ctx, player);
                        // NOTE(java-quirk): a player in no sepulcher throws here in Java.
                        let current_wave = progress(ctx, sepulcher_id)? + 1;
                        set_progress(ctx, sepulcher_id, current_wave); // update progress
                        for (sepulcher, wave, door) in DOORS {
                            if sepulcher == sepulcher_id && wave == current_wave {
                                ctx.open_door(door, 0);
                                ctx.start_quest_timer(
                                    &format!("CLOSE_DOOR_{door}"),
                                    15_000,
                                    None,
                                    None,
                                    false,
                                );
                                break;
                            }
                        }

                        if current_wave < 7 {
                            spawn_mysterious_chest(ctx, player);
                        } else {
                            spawn_next_wave(ctx, player);
                        }

                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::TheMonstersHaveSpawned,
                            &[],
                        );
                    } else {
                        let name = ctx.world().npc_template_name(npc.object_id);
                        if let Some(html) = ctx.get_htm(player, "Gatekeeper-no.html") {
                            let html = html.replace("%npcname%", &name);
                            ctx.show_result(player, Some(&html), Some(npc));
                        }
                    }

                    return None;
                }

                // TODO (Java): replace with a proper html.
                htmltext = Some(ctx.get_no_quest_msg(player));
            }
            "SPAWN_MYSTERIOUS_CHEST" => {
                spawn_mysterious_chest(ctx, player?);
                return None;
            }
            "VICTIM_FLEE" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    // NOTE(java-quirk): `npc.getSpawn()` throws for a victim without a spawn.
                    let (sx, sy, _) = ctx.npc_spawn_point(npc)?;
                    let dx = ctx.get_random_range(-400, 400);
                    let dy = ctx.get_random_range(-400, 400);
                    let at = ctx.world().position_of(npc.object_id);
                    let destination = ctx
                        .world()
                        .npc_valid_location_toward(npc.object_id, sx + dx, sy + dy);
                    if let (Some((x, y, _)), Some((tx, ty, tz))) = (at, destination)
                        && f64::from((x - sx).pow(2) + (y - sy).pow(2)).sqrt() < 600.0
                    {
                        ctx.npc_move_to(npc, tx, ty, tz);
                    }

                    if let Some(text) = ctx.get_random_entry(&VICTIM_MSG) {
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
                    }
                    ctx.start_quest_timer("VICTIM_FLEE", 3000, Some(npc), None, false);
                }

                return None;
            }
            "REMOVE_PETRIFY" => {
                let npc = npc?;
                ctx.world_mut()
                    .script_stop_skill_effects_removed(npc.object_id, PETRIFY.id);
                // npc.setTargetable(true);
                ctx.world_mut().set_invul(npc.object_id, false);
                return None;
            }
            "WAVE_DEFEATED_CHECK" => {
                let player = player?;
                let sepulcher_id = get_sepulcher_id(ctx, player);
                let current_wave = progress(ctx, sepulcher_id)?;
                let mut last_location = None;
                let mut list = monsters(ctx, sepulcher_id);
                for spawn in list.clone() {
                    if let Some((x, y, z)) = ctx.world().position_of(spawn) {
                        last_location = Some((x, y, z, ctx.heading_of(spawn)));
                    }
                    if ctx.is_dead(spawn) {
                        list.retain(|s| *s != spawn);
                    }
                }
                set_monsters(ctx, sepulcher_id, list.clone());

                if list.is_empty() {
                    if current_wave == 2 {
                        if ctx.get_random_boolean() {
                            spawn_next_wave(ctx, player);
                        } else {
                            spawn_key_chest(ctx, player, last_location);
                        }
                    } else if current_wave == 5 {
                        set_progress(ctx, sepulcher_id, current_wave + 1);
                        spawn_next_wave(ctx, player);
                    }
                } else if sepulcher_id > 0 {
                    ctx.start_quest_timer("WAVE_DEFEATED_CHECK", 5000, None, Some(player), false);
                }

                return None;
            }
            "ANNOUNCE" => {
                for (manager_id, _) in MANAGER_ZONES {
                    let manager = ctx
                        .world()
                        .npc_ids()
                        .into_iter()
                        .find(|id| {
                            ctx.world()
                                .npc(*id)
                                .is_some_and(|n| n.npc_id == manager_id && !n.decayed)
                        })
                        .and_then(|id| ctx.script_npc(id));
                    match manager {
                        Some(manager) => {
                            for message in MANAGER_MESSAGES {
                                ctx.broadcast_npc_say(manager, ChatType::NpcShout, message, &[]);
                            }
                        }
                        None => warn!("Four Sepulcher Manager with ID {manager_id} is missing!"),
                    }
                }

                let delay = ms_until_announce(ctx.now_ms());
                ctx.start_quest_timer("ANNOUNCE", delay, None, None, false);
            }
            "OUST" => {
                // The event time is over: `oustAllPlayers()` of the manager's zone.
                if let Some(zone) = npc.and_then(|n| zone_of_manager(n.id)) {
                    ctx.world_mut().script_oust_all_players(zone, SCRIPT_ZONE);
                }
                return None;
            }
            _ => {}
        }

        htmltext.take()
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == MYSTERIOUS_CHEST {
            if ctx.is_script_value(npc, 0) {
                ctx.set_script_value(npc, 1);
                ctx.delete_npc(npc);
                spawn_next_wave(ctx, player);
            }

            return None;
        }

        if npc.id == KEY_CHEST {
            if ctx.is_script_value(npc, 0) {
                ctx.set_script_value(npc, 1);
                ctx.delete_npc(npc);
                ctx.give_items(player, CHAPEL_KEY, 1);
            }

            return None;
        }

        Some(format!("{}.html", npc.id))
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);
        if npc.id == ROOM_3_VICTIM {
            ctx.disable_core_ai(npc, true);
            ctx.set_running(npc);
            ctx.start_quest_timer("VICTIM_FLEE", 1000, Some(npc), None, false);
        }

        if npc.id == ROOM_5_STATUE_GUARD {
            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
            ctx.world_mut().do_cast(npc.object_id, PETRIFY);
            ctx.set_can_return_to_spawn_point(npc, false);
            // npc.setTargetable(false); npc.setInvul(true);
            ctx.cancel_quest_timer("REMOVE_PETRIFY", Some(npc), None);
            ctx.start_quest_timer("REMOVE_PETRIFY", 5 * 60 * 1000, Some(npc), None, false); // 5 minutes
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            ROOM_3_VICTIM => {
                ctx.add_spawn_at(ROOM_3_CHEST_REWARDER, npc.object_id, false, 0);
            }
            ROOM_4_CHARM_1 | ROOM_4_CHARM_2 | ROOM_4_CHARM_3 | ROOM_4_CHARM_4 => {
                if let Some((x, y, z)) = ctx.world().position_of(killer) {
                    let skill_id = charm_skill(npc.id);
                    for zone in ctx.world().script_effect_zones_at(x, y, z) {
                        if ctx.world().effect_zone_skill_level(zone, skill_id) > 0 {
                            ctx.world_mut().set_effect_zone_enabled(zone, false);
                            break;
                        }
                    }
                }

                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, charm_msg(npc.id), &[]);
            }
            CONQUEROR_BOSS | EMPEROR_BOSS | GREAT_SAGES_BOSS | JUDGE_BOSS => {
                let sepulcher_id = get_sepulcher_id(ctx, killer);
                // NOTE(java-quirk): a killer in no sepulcher throws in Java before anything else.
                let Some(current_wave) = progress(ctx, sepulcher_id) else {
                    return;
                };
                set_progress(ctx, sepulcher_id, current_wave + 1);

                if ctx.world().party_of(killer).is_some() && sepulcher_id > 0 {
                    for mem in ctx.world().party_members_of(killer) {
                        if ctx.check_if_in_range(ctx.alt_party_range(), killer, mem, true) {
                            let started = quest_started(ctx, mem) == Some(true);
                            let goblet = 7255 + sepulcher_id;
                            if started
                                && !ctx.has_at_least_one_quest_item(mem, &[ANTIQUE_BROOCH])
                                && !ctx.has_at_least_one_quest_item(mem, &[goblet])
                            {
                                ctx.give_items(mem, goblet, 1);
                                ctx.play_sound(mem, QuestSound::ItemsoundQuestMiddle);
                            }
                        }
                    }
                }

                spawn_next_wave(ctx, killer);

                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    let heading = ctx.heading_of(npc.object_id);
                    ctx.add_spawn(TELEPORTER, x, y, z, heading, true, 0, 0);
                }
            }
            ROOM_6_REWARD_CHEST => {
                let amount = i64::from(ctx.get_random_range(300, 1300));
                ctx.drop_item(npc, Some(killer), 57, amount);
            }
            _ => {
                let loc = ctx
                    .world()
                    .position_of(npc.object_id)
                    .map(|(x, y, z)| (x, y, z, ctx.heading_of(npc.object_id)));
                spawn_key_chest(ctx, killer, loc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"
        <npc id="31921" type="Folk" level="70" name="Conquerors' Sepulcher Manager"/>
        <npc id="31468" type="Folk" level="70" name="Mysterious Chest"/>
        <npc id="31467" type="Folk" level="70" name="Key Chest"/>
        <npc id="18120" type="Monster" level="70" name="Rewarder"/>
        <npc id="18132" type="Monster" level="70" name="Guard"/>
        <npc id="18133" type="Monster" level="70" name="Guard 2"/>
        <npc id="18137" type="Monster" level="70" name="Guard 3"/>
        <npc id="25346" type="Monster" level="80" name="Boss"/>
        <npc id="31452" type="Folk" level="70" name="Teleporter"/>"#;

    // The four script zones of `custom_script.xml` the sepulchers use.
    const ZONES: &str = r#"<list enabled="true">
        <zone name="s1" id="200221" type="ScriptZone" shape="NPoly" minZ="-7263" maxZ="-7063">
            <node X="181198" Y="-86516" /><node X="192394" Y="-86521" /><node X="192391" Y="-84626" /><node X="181193" Y="-84622" />
        </zone>
        <zone name="s2" id="200222" type="ScriptZone" shape="NPoly" minZ="-7263" maxZ="-7063">
            <node X="179511" Y="-89903" /><node X="190707" Y="-89908" /><node X="190704" Y="-88013" /><node X="179506" Y="-88009" />
        </zone>
        <zone name="s3" id="200223" type="ScriptZone" shape="NPoly" minZ="-7263" maxZ="-7063">
            <node X="174498" Y="-82751" /><node X="176622" Y="-82751" /><node X="176616" Y="-71304" /><node X="174489" Y="-71303" />
        </zone>
        <zone name="s4" id="200224" type="ScriptZone" shape="NPoly" minZ="-7263" maxZ="-7063">
            <node X="172183" Y="-86660" /><node X="174307" Y="-86661" /><node X="174301" Y="-75214" /><node X="172174" Y="-75213" />
        </zone>
    </list>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7075" type="EtcItem" name="Entrance Pass"><set name="is_questitem" val="true"/></item>
        <item id="7260" type="EtcItem" name="Chapel Key"><set name="is_questitem" val="true"/></item>
        <item id="7261" type="EtcItem" name="Used Pass"><set name="is_questitem" val="true"/></item>
        <item id="7262" type="EtcItem" name="Antique Brooch"><set name="is_questitem" val="true"/></item>
        <item id="7256" type="EtcItem" name="Goblet"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00620_FourGoblets` as far as the sepulchers read it.
    struct FourGoblets;

    impl Script for FourGoblets {
        fn name(&self) -> &'static str {
            FOUR_GOBLETS
        }
        fn path(&self) -> &'static str {
            "quests/Q00620_FourGoblets"
        }
        fn id(&self) -> i32 {
            620
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(FourSepulchers);
        registry.add(FourGoblets);
        let dir = "data/scripts/ai/areas/ImperialTomb/FourSepulchers";
        let mut files: Vec<(String, String)> = [
            "31921-SP.html",
            "31921-NL.html",
            "31921-NE.html",
            "31921-OK.html",
            "31921-NS.html",
            "31921-SE.html",
            "31921-FULL.html",
            "Gatekeeper-no.html",
        ]
        .iter()
        .map(|f| {
            (
                format!("{dir}/{f}"),
                format!("<html><body>{f} %member%</body></html>"),
            )
        })
        .collect();
        files.push(("data/zones/script.xml".to_owned(), ZONES.to_owned()));
        let files: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &files, |world, root| {
            world.set_zone_data(
                Arc::new(l2_data::Zones::load(root)),
                Arc::new(l2_data::MapRegions::load(root)),
            );
        });
        let manager = t.spawn(CONQUEROR_MANAGER, 20);
        (
            t,
            ScriptNpc {
                object_id: manager,
                id: CONQUEROR_MANAGER,
            },
        )
    }

    #[test]
    fn the_announcement_is_at_minute_55() {
        // 12:10:30 -> 44 min 30 s.
        let base = 12 * 3_600_000;
        assert_eq!(
            ms_until_announce(base + 10 * 60_000 + 30_000),
            44 * 60_000 + 30_000
        );
        assert_eq!(ms_until_announce(base + 55 * 60_000), 60 * 60_000);
        assert_eq!(ms_until_announce(base + 56 * 60_000), 59 * 60_000);
    }

    #[test]
    fn the_data_tables_are_java_s() {
        assert_eq!(ROOM_SPAWN_DATA.len(), 762);
        assert_eq!(DOORS.len(), 20);
        assert_eq!(zone_of_manager(JUDGE_MANAGER), Some(JUDGE_ZONE));
        assert_eq!(start_hall_spawn(EMPEROR_MANAGER), Some((179963, -88978, -7218)));
    }

    #[test]
    fn the_manager_shows_its_page_to_the_first_talk() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![]);
        let mut page = None;
        t.world.run_as_script("FourSepulchers", |ctx| {
            page = FourSepulchers.on_first_talk(ctx, manager, ObjectId(1));
        });
        assert_eq!(page.as_deref(), Some("31921.html"));
    }

    #[test]
    fn entering_needs_a_party_its_leader_and_the_time() {
        let (mut t, manager) = setup();
        for id in 1..=4 {
            t.enter(id, 80, vec![item(1000 + id, ENTRANCE_PASS, 1)]);
        }
        t.take();
        let enter = |t: &mut ScriptTest, player: i32, minute: i64| {
            t.world.run_as_script("FourSepulchers", |ctx| {
                try_enter(ctx, manager, ObjectId(player), minute);
            });
        };
        // No party.
        enter(&mut t, 1, 56);
        // A party of 2 is too small.
        t.party(1, 2);
        enter(&mut t, 1, 56);
        // Not the leader.
        enter(&mut t, 2, 56);
        t.party(1, 3);
        t.party(1, 4);
        // Before minute 55.
        enter(&mut t, 1, 10);
        let out = t.take();
        assert!(!pages_to(&out, 1).is_empty());
        // Nobody left the place.
        assert!(
            t.world
                .zone_players_inside(CONQUEROR_ZONE, SCRIPT_ZONE)
                .is_empty()
        );
    }

    #[test]
    fn a_full_party_with_the_quest_and_passes_is_sent_in() {
        let (mut t, manager) = setup();
        for id in 1..=4 {
            t.enter(id, 80, vec![item(1000 + id, ENTRANCE_PASS, 1)]);
            t.world.run_as_script(FOUR_GOBLETS, |ctx| {
                let qs = ctx.get_quest_state(ObjectId(id), true).unwrap();
                qs.start_quest(ctx);
            });
        }
        t.take_writes();
        t.party(1, 2);
        t.party(1, 3);
        t.party(1, 4);
        t.world.run_as_script("FourSepulchers", |ctx| {
            try_enter(ctx, manager, ObjectId(1), 56);
        });
        t.ticks(60);
        for id in 1..=4 {
            // The client answers the teleport.
            t.input(id, l2_world::ClientInput::Appearing);
            t.world.revalidate_zone(ObjectId(id), true);
            assert_eq!(t.count(id, ENTRANCE_PASS), 0);
            assert_eq!(t.count(id, USED_PASS), 1);
        }
        assert_eq!(t.world.zone_players_inside(CONQUEROR_ZONE, SCRIPT_ZONE).len(), 4);

        // The mysterious chest of room 1 comes when the hour starts; opening it spawns the wave.
        t.world.run_as_script("FourSepulchers", |ctx| {
            spawn_mysterious_chest(ctx, ObjectId(1));
        });
        let chest = t.npcs_of(MYSTERIOUS_CHEST);
        assert_eq!(chest.len(), 1);
        t.world.run_as_script("FourSepulchers", |ctx| {
            let chest = ctx.script_npc(chest[0]).unwrap();
            assert_eq!(FourSepulchers.on_first_talk(ctx, chest, ObjectId(1)), None);
        });
        assert!(!t.in_world(chest[0]));
        assert_eq!(t.npcs_of(18132).len(), 4);
        assert_eq!(t.npcs_of(18137).len(), 6);

        // The oust timer of the manager runs for 50 minutes (the test world has no town to send
        // the players to, so only the timer is checked).
        t.world.run_as_script("FourSepulchers", |ctx| {
            assert!(ctx.has_quest_timer("OUST", Some(manager), None));
        });
        t.wait_ms(50 * 60 * 1000 + 100);
        t.world.run_as_script("FourSepulchers", |ctx| {
            assert!(!ctx.has_quest_timer("OUST", Some(manager), None));
        });
    }

    #[test]
    fn the_key_chest_gives_the_key_once() {
        let (mut t, _) = setup();
        t.enter(1, 80, vec![]);
        let key_chest = t.spawn(KEY_CHEST, 30);
        t.world.run_as_script("FourSepulchers", |ctx| {
            let chest = ctx.script_npc(key_chest).unwrap();
            assert_eq!(FourSepulchers.on_first_talk(ctx, chest, ObjectId(1)), None);
        });
        assert_eq!(t.count(1, CHAPEL_KEY), 1);
        assert!(!t.in_world(key_chest));
    }

    #[test]
    fn a_dead_rewarder_leaves_a_key_chest_at_its_place() {
        let (mut t, _) = setup();
        t.enter(1, 80, vec![]);
        let rewarder = t.spawn(18120, 40);
        t.kill(1, rewarder);
        assert_eq!(t.npcs_of(KEY_CHEST).len(), 1);
    }
}
