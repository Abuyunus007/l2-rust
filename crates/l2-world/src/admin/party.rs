//! The party admin commands: `//partyinfo [name]` (the party of a character with its loot rule
//! and a recall button), and the recall menus `//recall_party_menu <name>` and
//! `//recall_clan_menu <name>` that bring a whole party or clan to the GM.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java (admin_partyinfo,
// gatherPartyInfo)
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminMenu.java (recall_party_menu,
// recall_clan_menu, teleportCharacter)
//
// # The recall teleport
// Java `AdminMenu.teleportCharacter` sends the message, runs `teleToLocation(loc, true)` (a random
// offset, no jail check) and shows `charmanage.htm` to the GM. The GM's place is the one of the
// moment, like `activeChar.getLocation()`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::skills::player_class_name;
use super::{AdminHandler, AdminHandlers, JavaException};
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminEditChar` `admin_partyinfo`, `AdminMenu` `admin_recall_party_menu` and
/// `admin_recall_clan_menu`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_partyinfo", admin_partyinfo),
        ("admin_recall_party_menu", admin_recall_party_menu),
        ("admin_recall_clan_menu", admin_recall_clan_menu),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The loot rule's name as Java prints it (`PartyDistributionType.name()` with `_` as a space).
fn loot_name(distribution: Option<l2_protocol_game::server::PartyDistributionType>) -> &'static str {
    use l2_protocol_game::server::PartyDistributionType as Loot;
    match distribution {
        Some(Loot::FindersKeepers) => "FINDERS KEEPERS",
        Some(Loot::Random) => "RANDOM",
        Some(Loot::RandomIncludingSpoil) => "RANDOM INCLUDING SPOIL",
        Some(Loot::ByTurn) => "BY TURN",
        Some(Loot::ByTurnIncludingSpoil) => "BY TURN INCLUDING SPOIL",
        None => "Unknown",
    }
}

/// Java `AdminEditChar.onCommand`, `admin_partyinfo`: `substring(16)` names the character; without
/// one (or with an unknown one) the target is used.
fn admin_partyinfo(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let by_name = command.get(16..).and_then(|name| world.player_by_name(name));
    let target = by_name.or_else(|| world.target_of(gm));
    let Some(target) = target.filter(|t| world.is_player(*t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    if world.is_in_party(target) {
        gather_party_info(world, target, gm);
    } else {
        let name = world.object_name(target).unwrap_or_default();
        world.send_sys_message(gm, &format!("{name} is not in a party."));
    }
    Ok(())
}

/// Java `AdminEditChar.gatherPartyInfo(target, activeChar)`: the members (level, leader icon,
/// name and class) in `partyinfo.htm`.
fn gather_party_info(world: &mut World, target: ObjectId, gm: ObjectId) {
    let Some(party) = world.party_of(target) else {
        return;
    };
    let target_name = world.object_name(target).unwrap_or_default();
    let loot = format!(
        "<tr><td><table width=290 border=0 cellpadding=0 bgcolor=000000><tr><td align=center><font color=\"LEVEL\">Loot:</font> {}</td></tr></table></td></tr>",
        loot_name(world.party_distribution_type(party))
    );
    let recall_button = format!(
        "<button value=\"Recall Party\" action=\"bypass -h admin_recall_party_menu {target_name}\" width=\"80\" height=\"27\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">"
    );

    let mut text = String::from(
        "<tr><td><table width=290 border=0 cellpadding=2 bgcolor=333333><tr><td width=30 align=center><font color=\"LEVEL\">Lvl</font></td><td width=20></td><td width=110><font color=\"LEVEL\">Name</font></td><td width=110 align=center><font color=\"LEVEL\">Class</font></td></tr></table></td></tr>",
    );
    let leader = world.party_leader(party);
    let mut color = true;
    for member in world.party_members(party) {
        text.push_str(&format!(
            "<tr><td><table width=290 border=0 {}cellpadding=2><tr>",
            if color { "bgcolor=000000 " } else { "" }
        ));
        let level = world.player(member).map_or(0, |p| p.data.level);
        text.push_str(&format!("<td width=30 align=center>{level}</td>"));
        if Some(member) == leader {
            text.push_str(
                "<td width=20 align=center><img src=\"L2UI_CH3.party_leadericon\" width=12 height=12></td>",
            );
        } else {
            text.push_str("<td width=20></td>");
        }
        let member_name = world.object_name(member).unwrap_or_default();
        text.push_str(&format!(
            "<td width=110><a action=\"bypass -h admin_character_info {member_name}\">{member_name}</a></td>"
        ));
        let class = player_class_name(world, member).unwrap_or_default();
        text.push_str(&format!("<td width=110 align=center>{class}</td>"));
        text.push_str("</tr></table></td></tr>");
        color = !color;
    }

    let mut html = world.admin_html_file(gm, "data/html/admin/partyinfo.htm");
    html.replace("%party_loot%", &loot);
    html.replace("%player%", &target_name);
    html.replace("%party%", &text);
    html.replace("%recall_party%", &recall_button);
    world.send_admin_html(gm, &html);
}

/// Java `AdminMenu.teleportCharacter(player, loc, activeChar, message)`: the message, the teleport
/// to the GM's place with a random offset, and the GM's `charmanage.htm`.
fn teleport_character(world: &mut World, player: ObjectId, gm: ObjectId, message: &str) {
    world.send_message(player, message);
    if let Some((x, y, z)) = world.position_of(gm) {
        let offset = world.settings.teleport.max_offset_on_teleport;
        world.tele_to_location(player, TeleportTarget::at(x, y, z).with_random_offset(offset));
    }
    world.show_admin_html(gm, "charmanage.htm");
}

/// Java `AdminMenu.onCommand`, `admin_recall_party_menu <playerName>`.
fn admin_recall_party_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `command.length() <= 23`: no name after `admin_recall_party_menu`.
    if command.len() <= 23 {
        world.send_sys_message(gm, "Usage: //recall_party_menu <playerName>");
        return Ok(());
    }
    let name = command.get(24..).unwrap_or("");
    let Some(player) = world.player_by_name(name) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    let Some(party) = world.party_of(player) else {
        world.send_sys_message(gm, "Player is not in party.");
        teleport_character(world, player, gm, "Admin is teleporting you.");
        return Ok(());
    };
    let members = world.party_members(party);
    for member in &members {
        teleport_character(world, *member, gm, "Your party is being teleported by an Admin.");
    }
    let leader = world
        .party_leader(party)
        .and_then(|l| world.object_name(l))
        .unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!(
            "You have teleported the party of {leader} ({} members).",
            members.len()
        ),
    );
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_recall_clan_menu <playerName>`: the clan's online members.
fn admin_recall_clan_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `command.length() <= 22`: no name after `admin_recall_clan_menu`.
    if command.len() <= 22 {
        world.send_sys_message(gm, "Usage: //recall_clan_menu <playerName>");
        return Ok(());
    }
    let name = command.get(23..).unwrap_or("");
    let Some(player) = world.player_by_name(name) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    let Some((clan_id, clan_name)) = world.clan_of(player).map(|c| (c.id, c.name.clone())) else {
        world.send_sys_message(gm, "Player is not in a clan.");
        teleport_character(world, player, gm, "Admin is teleporting you.");
        return Ok(());
    };
    // Java `getOnlineMembers(0)`: the members online, the exclusion of 0 leaves them all.
    let online = world.clan_online_members(clan_id, 0);
    for member in &online {
        teleport_character(world, *member, gm, "Your clan is being teleported by an Admin.");
    }
    world.send_sys_message(
        gm,
        &format!(
            "You have teleported the clan \"{clan_name}\" ({} members).",
            online.len()
        ),
    );
    Ok(())
}
