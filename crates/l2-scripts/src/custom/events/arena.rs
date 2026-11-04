//! What TvT, Deathmatch and CtF have in common: the buffs of the manager, the screen messages,
//! the scoreboard, the page of the manager, the registration checks and the Java collections
//! (`ConcurrentHashMap<Player, Integer>` as an ordered list).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{
    CC_RECORD_FINISH, CC_RECORD_INITIALIZE, CC_RECORD_UPDATE, CcRecordEntry, ExPvpMatchCcRecord,
    ExSendUiEvent, ExShowScreenMessage, NpcHtmlMessage,
};
use l2_util::ObjectId;
use l2_world::{ScriptCtx, ScriptNpc, SkillKey};

/// `ExShowScreenMessage.TOP_CENTER` and `BOTTOM_RIGHT`.
pub(super) const TOP_CENTER: i32 = 2;
pub(super) const BOTTOM_RIGHT: i32 = 8;

/// The custom Ghost Walking skill (`tvt_event.xml`).
pub(super) const GHOST_WALKING: SkillKey = SkillKey {
    id: 100_000,
    level: 1,
};

pub(super) const FIGHTER_BUFFS: [SkillKey; 7] = [
    SkillKey { id: 4322, level: 1 }, // Wind Walk
    SkillKey { id: 4323, level: 1 }, // Shield
    SkillKey { id: 5637, level: 1 }, // Magic Barrier
    SkillKey { id: 4324, level: 1 }, // Bless the Body
    SkillKey { id: 4325, level: 1 }, // Vampiric Rage
    SkillKey { id: 4326, level: 1 }, // Regeneration
    SkillKey { id: 5632, level: 1 }, // Haste
];

pub(super) const MAGE_BUFFS: [SkillKey; 7] = [
    SkillKey { id: 4322, level: 1 }, // Wind Walk
    SkillKey { id: 4323, level: 1 }, // Shield
    SkillKey { id: 5637, level: 1 }, // Magic Barrier
    SkillKey { id: 4328, level: 1 }, // Bless the Soul
    SkillKey { id: 4329, level: 1 }, // Acumen
    SkillKey { id: 4330, level: 1 }, // Concentration
    SkillKey { id: 4331, level: 1 }, // Empower
];

/// The `BuffHeal` branch: nothing for a player who is neither on the event nor a GM, the combat
/// page for a player in combat, else the manager casts the buffs and the player is restored.
pub(super) fn buff_heal(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
    if ctx.world().is_on_event(player) || ctx.is_gm(player) {
        if ctx.is_in_combat(player) {
            return Some("manager-combat.html".to_owned());
        }

        let buffs = if ctx.is_mage_class(player) {
            MAGE_BUFFS
        } else {
            FIGHTER_BUFFS
        };
        for skill in buffs {
            // `npc.setTarget(player); npc.doCast(skill)`: no animation of `applyEffects`.
            ctx.cast_skill(npc, player, skill);
        }

        ctx.world_mut().player_full_restore(player);
    }

    None
}

/// `sendScreenMessage(player, message, duration)`.
pub(super) fn send_screen_message(ctx: &ScriptCtx<'_>, player: ObjectId, message: &str, duration: i32) {
    ctx.world().send(
        player,
        &ExShowScreenMessage::styled_text(message, TOP_CENTER, duration * 1000, 0, true, false),
    );
}

/// `broadcastScreenMessage` / `broadcastScreenMessageWithEffect` to the players of the instance.
pub(super) fn broadcast_screen_message(
    ctx: &ScriptCtx<'_>,
    instance_id: i32,
    message: &str,
    duration: i32,
    effect: bool,
) {
    ctx.world().instance_broadcast_packet(
        instance_id,
        &ExShowScreenMessage::styled_text(message, TOP_CENTER, duration * 1000, 0, true, effect),
    );
}

/// `new ExSendUIEvent(player, false, false, seconds, 10, NpcStringId.TIME_REMAINING)` with a
/// timer, or the one with 0 that takes it off.
pub(super) fn send_time_remaining(ctx: &ScriptCtx<'_>, player: ObjectId, seconds: i32) {
    let end_time = if seconds > 0 { 10 } else { 0 };
    ctx.world().send(
        player,
        &ExSendUiEvent {
            object_id: player.0,
            hide: false,
            count_up: false,
            start_time: seconds,
            end_time,
            npc_string_id: NpcStringId::TimeRemaining.id(),
            params: Vec::new(),
        },
    );
}

/// `MapUtil.sortByValue(PLAYER_SCORES, true)`: the highest score first, equal scores in the
/// order they were put in.
pub(super) fn sorted_scores(scores: &[(ObjectId, i32)]) -> Vec<(ObjectId, i32)> {
    let mut sorted = scores.to_vec();
    sorted.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    sorted
}

/// `new ExPVPMatchCCRecord(state, sortedScores)` to the players of the instance.
pub(super) fn broadcast_scoreboard(
    ctx: &ScriptCtx<'_>,
    instance_id: i32,
    state: i32,
    scores: &[(ObjectId, i32)],
) {
    ctx.world()
        .instance_broadcast_packet(instance_id, &scoreboard_packet(ctx, state, scores));
}

/// The scoreboard packet.
pub(super) fn scoreboard_packet(
    ctx: &ScriptCtx<'_>,
    state: i32,
    scores: &[(ObjectId, i32)],
) -> ExPvpMatchCcRecord {
    ExPvpMatchCcRecord {
        state,
        players: sorted_scores(scores)
            .into_iter()
            .map(|(player, points)| CcRecordEntry {
                name: ctx.player_name(player),
                points,
            })
            .collect(),
    }
}

pub(super) const SCOREBOARD_INITIALIZE: i32 = CC_RECORD_INITIALIZE;
pub(super) const SCOREBOARD_UPDATE: i32 = CC_RECORD_UPDATE;
pub(super) const SCOREBOARD_FINISH: i32 = CC_RECORD_FINISH;

/// `PLAYER_SCORES.put(player, value)` on the ordered list.
pub(super) fn put_score(scores: &mut Vec<(ObjectId, i32)>, player: ObjectId, value: i32) {
    match scores.iter_mut().find(|(p, _)| *p == player) {
        Some(entry) => entry.1 = value,
        None => scores.push((player, value)),
    }
}

/// `PLAYER_SCORES.get(player)`.
pub(super) fn get_score(scores: &[(ObjectId, i32)], player: ObjectId) -> Option<i32> {
    scores.iter().find(|(p, _)| *p == player).map(|(_, v)| *v)
}

/// `PLAYER_SCORES.remove(player)`.
pub(super) fn remove_score(scores: &mut Vec<(ObjectId, i32)>, player: ObjectId) {
    scores.retain(|(p, _)| *p != player);
}

/// `Collections.shuffle(list)` with the world's random numbers (D-13), the algorithm of the JDK:
/// from the last index down, swap with a random one at or below it.
pub(super) fn shuffle(ctx: &mut ScriptCtx<'_>, list: &mut [ObjectId]) {
    for i in (1..list.len()).rev() {
        let j = ctx.get_random(i as i32 + 1) as usize;
        list.swap(i, j);
    }
}

/// The `manager-cancel` / `manager-register` pages: the file with `%player_numbers%`.
pub(super) fn show_manager_page(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
    file: &str,
    player_numbers: usize,
) {
    let html = ctx.get_htm(player, file).unwrap_or_default();
    let html = html.replace("%player_numbers%", &player_numbers.to_string());
    if let Some(message) = NpcHtmlMessage::new(npc.object_id.0, &html) {
        ctx.world_mut().send_html(player, &message);
    }
}

/// The checks of `canRegister` that are the same in the three events, in Java's order.
/// `registered` is `PLAYER_LIST.contains(player)`, `count` its size.
pub(super) fn can_register(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    registered: bool,
    count: usize,
    levels: (i32, i32),
    maximum_count: usize,
) -> bool {
    if registered {
        ctx.world()
            .send_message(player, "You are already registered on this event.");
        return false;
    }

    let level = ctx.player_level(player);
    if level < levels.0 {
        ctx.world()
            .send_message(player, "Your level is too low to participate.");
        return false;
    }

    if level > levels.1 {
        ctx.world()
            .send_message(player, "Your level is too high to participate.");
        return false;
    }

    if ctx.world().is_registered_on_event(player) || ctx.world().block_checker_arena_of(player) > -1 {
        ctx.world()
            .send_message(player, "You are already registered on an event.");
        return false;
    }

    if count >= maximum_count {
        ctx.world()
            .send_message(player, "There are too many players registered on the event.");
        return false;
    }

    if let Some(lines) = ctx.world().event_registration_refusal(player) {
        for line in lines {
            ctx.world().send_message(player, line);
        }
        return false;
    }

    true
}
