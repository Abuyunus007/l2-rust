//! The clan admin commands: `//clan_info [name|objectId]`, `//clan_changeleader`,
//! `//clan_show_pending`, `//clan_force_pending`, `//pledge <create|info|dismiss|setlevel|rep>`
//! and `//remove_clan_penalty join|create <name>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminClan.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPledge.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java (remove_clan_penalty)
//
// # Left out, with the system that brings it
// - `//give_clan_skills`, `//give_all_clan_skills` and `//add_clan_skill` (AdminSkill.java): the
//   pledge skill tree and `Clan.addNewSkill` come with M6-CLAN-03, so these words are not
//   registered yet and answer "The command '...' does not exist!".

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{GmViewPledgeInfo, PledgeMember, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::clan::Clan;
use crate::residence::{CastleId, ClanHallId, FortId};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminClan`, `AdminPledge` and the `remove_clan_penalty` of `AdminEditChar`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_clan_info", admin_clan_info),
        ("admin_clan_changeleader", admin_clan_changeleader),
        ("admin_clan_show_pending", admin_clan_show_pending),
        ("admin_clan_force_pending", admin_clan_force_pending),
        ("admin_pledge", admin_pledge),
        ("admin_remove_clan_penalty", admin_remove_clan_penalty),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminClan.getPlayer(activeChar, st)`: the next word is an object id (from the html) or a
/// character name; without a word the target must be a player. `Ok(None)` after the message the
/// GM gets, `Err` for an id that does not fit an int (Java's `NumberFormatException`).
fn player_arg(world: &World, gm: ObjectId, st: &mut Tokens<'_>) -> Result<Option<ObjectId>, JavaException> {
    let Some(value) = st.next_token() else {
        return Ok(world.target_of(gm).filter(|t| world.is_player(*t)).or_else(|| {
            world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
            None
        }));
    };
    if is_numeric(value) {
        let object_id = ObjectId(parse_int(value)?);
        if world.is_player(object_id) {
            return Ok(Some(object_id));
        }
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
        return Ok(None);
    }
    match world.player_by_name(value) {
        Some(player) => Ok(Some(player)),
        None => {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::IncorrectNamePleaseTryAgain),
            );
            Ok(None)
        }
    }
}

/// `AdminPledge.showMainPage`: `game_menu.htm`.
fn show_main_page(world: &mut World, gm: ObjectId) {
    world.show_admin_html(gm, "game_menu.htm");
}

/// The clan of a player, `None` without one (`Player.getClan()`).
fn clan_id_of(world: &World, player: ObjectId) -> Option<i32> {
    world.clan_of(player).map(|c| c.id)
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

// ------------------------------------------------------------------------------- AdminClan

/// Java `AdminClan.onCommand`, `admin_clan_info`: the clan of the player in `claninfo.htm`.
fn admin_clan_info(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token(); // the command word
    let Some(player) = player_arg(world, gm, &mut st)? else {
        return Ok(());
    };
    let Some(clan) = world.clan_of(player) else {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::TheTargetMustBeAClanMember),
        );
        return Ok(());
    };

    // `CastleManager.getCastleById(..).getName()`, `ClanHallTable.getClanHallById(..)` and
    // `FortManager.getFortById(..)`: a residence id without a residence is Java's
    // `NullPointerException`.
    let castle = if clan.castle_id > 0 {
        world
            .castle(CastleId(clan.castle_id))
            .map(|c| c.name.clone())
            .ok_or_else(null_pointer)?
    } else {
        "No".to_owned()
    };
    let hall = if clan.hideout_id > 0 {
        world
            .clan_hall(ClanHallId(clan.hideout_id))
            .map(|h| h.name.clone())
            .ok_or_else(null_pointer)?
    } else {
        "No".to_owned()
    };
    let fortress = if clan.fort_id > 0 {
        world
            .fort(FortId(clan.fort_id))
            .map(|f| f.name.clone())
            .ok_or_else(null_pointer)?
    } else {
        "No".to_owned()
    };
    let ally = if clan.ally_id > 0 {
        clan.ally_name.clone().unwrap_or_default()
    } else {
        "Not in ally".to_owned()
    };
    let values = [
        ("%clan_name%", clan.name.clone()),
        ("%clan_leader%", world.clan_leader_name(clan.id)),
        ("%clan_level%", clan.level.to_string()),
        ("%clan_has_castle%", castle),
        ("%clan_has_clanhall%", hall),
        ("%clan_has_fortress%", fortress),
        ("%clan_points%", clan.reputation_score.to_string()),
        ("%clan_players_count%", clan.members_count().to_string()),
        ("%clan_ally%", ally),
        ("%current_player_objectId%", player.0.to_string()),
        (
            "%current_player_name%",
            world.object_name(player).unwrap_or_default(),
        ),
    ];
    let mut html = world.admin_html_file(gm, "data/html/admin/claninfo.htm");
    for (pattern, value) in &values {
        html.replace(pattern, value);
    }
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminClan.onCommand`, `admin_clan_changeleader`: the player's clan gets the player as the
/// new leader. An academy member cannot be the leader (the message goes to that player).
fn admin_clan_changeleader(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(player) = player_arg(world, gm, &mut st)? else {
        return Ok(());
    };
    let Some(clan_id) = clan_id_of(world, player) else {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::TheTargetMustBeAClanMember),
        );
        return Ok(());
    };
    let is_member = world
        .clan(clan_id)
        .is_some_and(|clan| clan.member(player).is_some());
    if !is_member {
        return Ok(());
    }
    if world.is_academy_member(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::ThatPrivilegeCannotBeGrantedToAClanAcademyMember),
        );
    } else {
        world.set_new_leader(clan_id, player);
    }
    Ok(())
}

/// Java `AdminClan.onCommand`, `admin_clan_show_pending`: every clan with a pending leader change,
/// in `clanchanges.htm`. The clans are listed by id (Java lists its table's order).
fn admin_clan_show_pending(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let mut pending: Vec<&Clan> = world.clans().filter(|c| c.new_leader_id != 0).collect();
    pending.sort_by_key(|c| c.id);
    let mut rows = String::new();
    for clan in pending {
        let new_leader = clan
            .member(ObjectId(clan.new_leader_id))
            .map(|m| m.name.clone())
            .unwrap_or_default();
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{new_leader}</td><td><a action=\"bypass -h admin_clan_force_pending {}\">Force</a></td></tr>",
            clan.name, clan.id
        ));
    }
    let mut html = world.admin_html_file(gm, "data/html/admin/clanchanges.htm");
    html.replace("%data%", &rows);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminClan.onCommand`, `admin_clan_force_pending <clanId>`: the pending leader of that clan
/// becomes the leader now. Anything not numeric, unknown or without a pending member does nothing.
fn admin_clan_force_pending(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(token) = st.next_token() else {
        return Ok(());
    };
    if !is_numeric(token) {
        return Ok(());
    }
    let clan_id = parse_int(token)?;
    let Some(new_leader) = world.clan(clan_id).map(|c| c.new_leader_id) else {
        return Ok(());
    };
    let has_member = world
        .clan(clan_id)
        .is_some_and(|clan| clan.member(ObjectId(new_leader)).is_some());
    if !has_member {
        return Ok(());
    }
    world.set_new_leader(clan_id, ObjectId(new_leader));
    world.send_sys_message(gm, "Task have been forcely executed.");
    Ok(())
}

// ------------------------------------------------------------------------------- AdminPledge

/// Java `AdminPledge.onCommand`, `admin_pledge`. The target is the player (the GM when the target
/// is not a player). Every failure goes back to `game_menu.htm`, as Java does.
fn admin_pledge(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let player = world.target_of(gm).filter(|t| world.is_player(*t)).unwrap_or(gm);
    let name = world.object_name(player).unwrap_or_default();

    let Some(action) = st.next_token() else {
        world.send_sys_message(gm, "Not allowed Action on Clan");
        show_main_page(world, gm);
        return Ok(());
    };
    if action != "create" && !world.is_clan_leader(player) {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::S1IsNotAClanLeader).with(SystemMessageParam::Text(name)),
        );
        show_main_page(world, gm);
        return Ok(());
    }
    let parameter = st.next_token();

    match action {
        "create" => pledge_create(world, gm, player, parameter),
        "dismiss" => {
            let clan_id = clan_id_of(world, player).unwrap_or(0);
            world.destroy_clan(clan_id, true);
            if world.clan_of(player).is_none() {
                world.send_sys_message(gm, "Clan disbanded.");
            } else {
                world.send_sys_message(gm, "There was a problem while destroying the clan.");
                show_main_page(world, gm);
            }
        }
        "info" => {
            let clan_id = match parameter {
                Some(clan_name) => world.clan_by_name(clan_name).map(|c| c.id),
                None => clan_id_of(world, player),
            };
            // Java: `new GMViewPledgeInfo(null, player)` throws a NullPointerException.
            let Some(clan) = clan_id.and_then(|id| world.clan(id)) else {
                return Err(JavaException("java.lang.NullPointerException".to_owned()));
            };
            let packet = gm_view_pledge_info(world, clan, &name);
            world.send(gm, &packet);
        }
        "setlevel" => pledge_setlevel(world, gm, player, parameter),
        "rep" => pledge_rep(world, gm, player, parameter),
        _ => {
            world.send_sys_message(gm, "Clan action not allowed.");
            show_main_page(world, gm);
        }
    }
    Ok(())
}

/// Java `AdminPledge`, `create`: the player's creation penalty is lifted for the try and put back
/// when the clan is not made.
fn pledge_create(world: &mut World, gm: ObjectId, player: ObjectId, parameter: Option<&str>) {
    let Some(clan_name) = parameter else {
        world.send_sys_message(gm, "Please, enter clan name.");
        show_main_page(world, gm);
        return;
    };
    let previous = world.player_clan_state(player).clan_create_expiry_time;
    if let Some(state) = world.player_clan_state_mut(player) {
        state.clan_create_expiry_time = 0;
    }
    if world.create_clan(player, clan_name).is_some() {
        let leader = world.object_name(player).unwrap_or_default();
        world.send_sys_message(gm, &format!("Clan {clan_name} created. Leader: {leader}"));
        return;
    }
    if let Some(state) = world.player_clan_state_mut(player) {
        state.clan_create_expiry_time = previous;
    }
    world.send_sys_message(gm, "There was a problem while creating the clan.");
    show_main_page(world, gm);
}

/// Java `AdminPledge`, `setlevel <0..11>`.
fn pledge_setlevel(world: &mut World, gm: ObjectId, player: ObjectId, parameter: Option<&str>) {
    let Some(text) = parameter else {
        world.send_sys_message(gm, "Usage: //pledge <setlevel|rep> <number>");
        show_main_page(world, gm);
        return;
    };
    let Some(clan_id) = clan_id_of(world, player) else {
        return;
    };
    let level = match parse_int(text) {
        Ok(level) => level,
        Err(_) => {
            world.send_sys_message(gm, "Level incorrect.");
            world.send_sys_message(gm, "Usage: //pledge <setlevel|rep> <number>");
            show_main_page(world, gm);
            return;
        }
    };
    // `MAX_CLAN_LEVEL` is 11.
    if (0..=11).contains(&level) {
        world.change_clan_level(clan_id, level);
        let clan_name = world.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
        world.send_sys_message(gm, &format!("You set level {level} for clan {clan_name}"));
    } else {
        world.send_sys_message(gm, "Level incorrect.");
        world.send_sys_message(gm, "Usage: //pledge <setlevel|rep> <number>");
        show_main_page(world, gm);
    }
}

/// Java `AdminPledge`, `rep <points>`: only a clan of level 5 or above takes reputation points.
fn pledge_rep(world: &mut World, gm: ObjectId, player: ObjectId, parameter: Option<&str>) {
    // `REP_POINTS_REWARD_LEVEL`
    const REP_POINTS_REWARD_LEVEL: i32 = 5;
    let Some(text) = parameter else {
        world.send_sys_message(gm, "Usage: //pledge <setlevel|rep> <number>");
        show_main_page(world, gm);
        return;
    };
    let Some(clan_id) = clan_id_of(world, player) else {
        return;
    };
    let points = match parse_int(text) {
        Ok(points) => points,
        Err(_) => {
            world.send_sys_message(gm, "Points incorrect.");
            world.send_sys_message(gm, "Usage: //pledge <setlevel|rep> <number>");
            show_main_page(world, gm);
            return;
        }
    };
    let Some((level, name)) = world.clan(clan_id).map(|c| (c.level, c.name.clone())) else {
        return;
    };
    if level < REP_POINTS_REWARD_LEVEL {
        world.send_sys_message(
            gm,
            "Only clans of level 5 or above may receive reputation points.",
        );
        show_main_page(world, gm);
        return;
    }
    world.add_reputation_score(clan_id, points);
    let score = world.clan(clan_id).map_or(0, |c| c.reputation_score);
    world.send_sys_message(
        gm,
        &format!(
            "You {}{} points {}{}'s reputation. Their current score is {score}",
            if points > 0 { "add " } else { "remove " },
            points.wrapping_abs(),
            if points > 0 { "to " } else { "from " },
            name,
        ),
    );
    show_main_page(world, gm);
}

/// Java `GMViewPledgeInfo.writeImpl`: the clan with every member, as the GM's window shows it.
pub(crate) fn gm_view_pledge_info(world: &World, clan: &Clan, player_name: &str) -> GmViewPledgeInfo {
    let members = world
        .clan_members_info(clan.id)
        .into_iter()
        .map(|m| PledgeMember {
            name: m.name,
            level: m.level,
            class_id: m.class_id,
            sex: i32::from(m.female),
            race: m.race,
            online_object_id: if m.online { m.object_id.0 } else { 0 },
            has_sponsor: m.sponsor != 0,
        })
        .collect();
    GmViewPledgeInfo {
        player_name: player_name.to_owned(),
        clan_id: clan.id,
        clan_name: clan.name.clone(),
        leader_name: world.clan_leader_name(clan.id),
        crest_id: clan.crest_id,
        level: clan.level,
        castle_id: clan.castle_id,
        hideout_id: clan.hideout_id,
        fort_id: clan.fort_id,
        rank: clan.rank,
        reputation_score: clan.reputation_score,
        ally_id: clan.ally_id,
        ally_name: clan.ally_name.clone().unwrap_or_default(),
        ally_crest_id: clan.ally_crest_id,
        at_war: clan.is_at_war(),
        members,
    }
}

// ------------------------------------------------------------- AdminEditChar: remove_clan_penalty

/// Java `AdminEditChar.onCommand`, `admin_remove_clan_penalty join|create <name>` (the words are
/// split on spaces only, as `new StringTokenizer(command, " ")` does).
// NOTE(java-quirk): for a character that is not online, Java runs `UPDATE characters SET
// clan_create_expiry_time WHERE char_name=?` (no `= value`), which the database refuses. Java only
// logs that failure and sends nothing to the GM, so this does the same.
fn admin_remove_clan_penalty(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let mut words = Vec::new();
    while let Some(word) = st.next_token() {
        words.push(word);
    }
    if words.len() != 3 {
        world.send_sys_message(gm, "Usage: //remove_clan_penalty join|create charname");
        return Ok(());
    }
    let change_create_expiry_time = words[1].eq_ignore_ascii_case("create");
    let player_name = words[2];
    match world.player_by_name(player_name) {
        Some(player) => {
            if let Some(state) = world.player_clan_state_mut(player) {
                if change_create_expiry_time {
                    state.clan_create_expiry_time = 0;
                } else {
                    state.clan_join_expiry_time = 0;
                }
            }
            world.send_sys_message(
                gm,
                &format!("Clan penalty successfully removed to character: {player_name}"),
            );
        }
        None => {
            warn!(
                "Problem with AdminEditChar: the character {player_name} is not online (Java's UPDATE has no value)"
            );
        }
    }
    Ok(())
}
