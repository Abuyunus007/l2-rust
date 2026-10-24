//! The clan hall words of the GM panel: `//clanhall <id>` (the hall's page), `//clanhallset`,
//! `//clanhalldel`, `//clanhallopendoors`, `//clanhallclosedoors` and `//clanhallteleportself`.
//! `//clanhall` alone shows the castle select page, as `//siege` does.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSiege.java (the clan hall part)
//
// The conquerable halls' words are `//chsiege_*` (AdminCHSiege, M7-CHSIEGE-01), not these.
//
// # Left out, with the system that brings it
// - `//clanhallset` of an auctionable hall deletes the hall's auction (`ClanHallAuction`
//   `deleteAuctionFromDB`). The auction list is private to `clanhall/auction.rs`: until a
//   crate-level accessor exists the auction stays, see the `// M7-ADMIN-06:` marker below.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::residence::ClanHallId;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// The clan hall words of Java `AdminSiege`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_clanhall", admin_clanhall),
        ("admin_clanhallset", admin_clanhall),
        ("admin_clanhalldel", admin_clanhall),
        ("admin_clanhallopendoors", admin_clanhall),
        ("admin_clanhallclosedoors", admin_clanhall),
        ("admin_clanhallteleportself", admin_clanhall),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// Java `AdminSiege.onCommand` for the clan hall words. A hall id that is not in the table makes
/// Java's `clanhall` variable `null`; the words use it after their target checks, so that is
/// where the `NullPointerException` comes.
fn admin_clanhall(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let word = st.next_token().unwrap_or_default();
    let Some(value) = st.next_token() else {
        // `showCastleSelectPage`: the castle select page of `//siege`, which lists the halls too.
        return show_castle_select_page(world, gm);
    };
    if !is_numeric(value) {
        return Ok(());
    }
    let hall_id = ClanHallId(parse_int(value)?);
    let hall = world.clan_hall(hall_id).map(|h| h.id);
    let target = world.target_of(gm).filter(|t| world.is_player(*t));
    let invalid_target = |world: &World| {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    };
    match word {
        "admin_clanhallset" => {
            let Some(player) = target.filter(|p| world.clan_of(*p).is_some()) else {
                invalid_target(world);
                return Ok(());
            };
            let hall = hall.ok_or_else(null_pointer)?;
            if world.clan_hall(hall).is_some_and(|h| h.owner_id > 0) {
                world.send_sys_message(gm, "This Clan Hall is not free!");
                return Ok(());
            }
            let Some(clan) = world.clan_of(player).map(|c| c.id) else {
                return Ok(());
            };
            if world.clan(clan).is_some_and(|c| c.hideout_id > 0) {
                world.send_sys_message(gm, "You have already a Clan Hall!");
                return Ok(());
            }
            if world.clan_hall(hall).is_some_and(|h| !h.is_siegable_hall()) {
                world.clan_hall_table_set_owner(hall, clan);
                // M7-ADMIN-06: Java then deletes the hall's auction row
                // (`ClanHallAuctionManager.getAuction(id).deleteAuctionFromDB()`); blocked on an
                // accessor in `clanhall/auction.rs`.
            } else {
                // `clanhall.setOwner(clan)` and `clan.setHideoutId(id)`.
                world.clan_hall_set_owner(hall, clan);
                if let Some(c) = world.clan_mut(clan) {
                    c.hideout_id = hall.0;
                }
            }
        }
        "admin_clanhalldel" => {
            let hall = hall.ok_or_else(null_pointer)?;
            if world.clan_hall(hall).is_some_and(|h| !h.is_siegable_hall()) {
                if world.clan_hall_is_free(hall) {
                    world.send_sys_message(gm, "This Clan Hall is already free!");
                } else {
                    world.clan_hall_table_set_free(hall);
                    world.clan_hall_init_npc_auction(hall.0);
                }
            } else {
                let old_owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
                if old_owner > 0 {
                    world.clan_hall_free(hall);
                    if let Some(clan) = world.clan_mut(old_owner) {
                        clan.hideout_id = 0;
                    }
                    world.broadcast_clan_status(old_owner);
                }
            }
        }
        "admin_clanhallopendoors" => {
            let hall = hall.ok_or_else(null_pointer)?;
            world.manager_open_close_doors(hall, true);
        }
        "admin_clanhallclosedoors" => {
            let hall = hall.ok_or_else(null_pointer)?;
            world.manager_open_close_doors(hall, false);
        }
        "admin_clanhallteleportself" => {
            let hall = hall.ok_or_else(null_pointer)?;
            teleport_self(world, gm, hall);
        }
        _ => {
            let hall = hall.ok_or_else(null_pointer)?;
            if world.clan_hall(hall).is_some_and(|h| !h.is_siegable_hall()) {
                show_clan_hall_page(world, gm, hall);
            } else {
                show_siegable_hall_page(world, gm, hall);
            }
        }
    }
    Ok(())
}

/// Java `activeChar.teleToLocation(zone.getSpawnLoc(), true)`: a random point of the hall's zone
/// (the first one without `RandomRespawnInTownEnabled`), with the random offset. A hall without a
/// zone does nothing.
fn teleport_self(world: &mut World, gm: ObjectId, hall: ClanHallId) {
    let Some(zone) = world.clan_hall_zone(hall).cloned() else {
        return;
    };
    let random = world.settings.teleport.random_respawn_in_town_enabled;
    let loc = {
        let mut pick = |len: usize| {
            if random {
                usize::try_from(world.rng().below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
            } else {
                0
            }
        };
        zone.spawn_loc(&mut pick)
    };
    let Some(loc) = loc else {
        return;
    };
    let offset = world.settings.teleport.max_offset_on_teleport;
    world.tele_to_location(
        gm,
        TeleportTarget::at(loc.x, loc.y, loc.z).with_random_offset(offset),
    );
}

/// Java `AdminSiege.showClanHallPage`: `clanhall.htm` with the hall's name and id and the name of
/// its owner (`None` without one).
fn show_clan_hall_page(world: &mut World, gm: ObjectId, hall: ClanHallId) {
    let Some((name, id, owner_id)) = world
        .clan_hall(hall)
        .map(|h| (h.name.clone(), h.id.0, h.owner_id))
    else {
        return;
    };
    let owner = world
        .clan(owner_id)
        .map_or_else(|| "None".to_owned(), |c| c.name.clone());
    let mut html = world.admin_html_file(gm, "data/html/admin/clanhall.htm");
    html.replace("%clanhallName%", &name);
    html.replace("%clanhallId%", &id.to_string());
    html.replace("%clanhallOwner%", &owner);
    world.send_admin_html(gm, &html);
}

/// Java `AdminSiege.showSiegableHallPage`: `siegablehall.htm`; an owner without a clan, and no
/// owner at all, both read "No Owner".
fn show_siegable_hall_page(world: &mut World, gm: ObjectId, hall: ClanHallId) {
    let Some((name, id, owner_id)) = world
        .clan_hall(hall)
        .map(|h| (h.name.clone(), h.id.0, h.owner_id))
    else {
        return;
    };
    let owner = if owner_id > 0 {
        world
            .clan(owner_id)
            .map_or_else(|| "No Owner".to_owned(), |c| c.name.clone())
    } else {
        "No Owner".to_owned()
    };
    let mut html = world.admin_html_file(gm, "data/html/admin/siegablehall.htm");
    html.replace("%clanhallId%", &id.to_string());
    html.replace("%clanhallName%", &name);
    html.replace("%clanhallOwner%", &owner);
    world.send_admin_html(gm, &html);
}

/// `AdminSiege.showCastleSelectPage`, which `//siege` owns: the handler of `admin_siege` runs
/// with the bare word, as Java calls the same page from the same branch.
fn show_castle_select_page(world: &mut World, gm: ObjectId) -> Outcome {
    match world.admin.handlers.get("admin_siege") {
        Some(siege) => siege(world, gm, "admin_siege"),
        None => Ok(()),
    }
}
