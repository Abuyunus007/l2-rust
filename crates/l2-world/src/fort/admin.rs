//! The fortress siege admin commands: `//fortsiege`, `//add_fortattacker`, `//list_fortsiege_clans`,
//! `//clear_fortsiege_list`, `//spawn_fortdoors`, `//endfortsiege`, `//startfortsiege`,
//! `//setfort` and `//removefort`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminFortSiege.java
//
// A GM can run a test siege on this world whatever `SiegeDatabaseWrites` says (D-51): the siege
// runs here, and its rows are written only with the switch.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::residence::{FortId, ResidenceRef};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminFortSiege.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_fortsiege", admin_fort_siege),
        ("admin_add_fortattacker", admin_fort_siege),
        ("admin_list_fortsiege_clans", admin_fort_siege),
        ("admin_clear_fortsiege_list", admin_fort_siege),
        ("admin_spawn_fortdoors", admin_fort_siege),
        ("admin_endfortsiege", admin_fort_siege),
        ("admin_startfortsiege", admin_fort_siege),
        ("admin_setfort", admin_fort_siege),
        ("admin_removefort", admin_fort_siege),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminFortSiege.onCommand`.
fn admin_fort_siege(world: &mut World, gm: ObjectId, command_value: &str) -> Outcome {
    let mut st = Tokens::spaces(command_value);
    // Get actual command.
    let command = st.next_token().unwrap_or("");
    // Get fort.
    let mut fort_id = 0;
    let mut fort = None;
    if st.has_more() {
        fort_id = parse_int(st.next_token().unwrap_or(""))?;
        fort = world.fort(FortId(fort_id)).map(|f| f.id);
    }
    let Some(fort) = fort.filter(|_| fort_id != 0) else {
        // No fort specified.
        show_fort_select_page(world, gm);
        return Ok(());
    };
    let target = world.target_of(gm).filter(|t| world.is_player(*t));

    match command.to_ascii_lowercase().as_str() {
        "admin_add_fortattacker" => match target {
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
            }
            Some(player) => {
                if world.fort_add_attacker(fort, player, false) == 4 {
                    world.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YourClanHasBeenRegisteredToS1SFortressBattle)
                            .with(SystemMessageParam::CastleName(fort.0)),
                    );
                } else {
                    world.send_message(player, "During registering error occurred!");
                }
            }
        },
        "admin_clear_fortsiege_list" => world.fort_clear_siege_clan(fort),
        "admin_endfortsiege" => world.fort_end_siege(fort),
        "admin_list_fortsiege_clans" => world.send_sys_message(gm, "Not implemented yet."),
        "admin_setfort" => match target.filter(|p| world.clan_of(*p).is_some()) {
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
            }
            Some(player) => {
                let clan = world.clan_id_of(player);
                world.fort_end_of_siege(fort, clan);
            }
        },
        "admin_removefort" => {
            if world.fort(fort).is_some_and(|f| f.owner_id != 0) {
                world.fort_remove_owner(fort, true);
            } else {
                world.send_sys_message(gm, "Unable to remove fort");
            }
        }
        "admin_spawn_fortdoors" => world.fort_reset_doors(fort),
        "admin_startfortsiege" => world.fort_start_siege(fort),
        _ => {}
    }
    show_fort_siege_page(world, gm, fort);
    Ok(())
}

/// Java `showFortSelectPage`.
fn show_fort_select_page(world: &mut World, gm: ObjectId) {
    let mut html = world.admin_html_file(gm, "data/html/admin/forts.htm");
    let mut list = String::new();
    let mut i = 0;
    for fort in world.forts() {
        list.push_str(&format!(
            "<td fixwidth=90><a action=\"bypass -h admin_fortsiege {0}\">{1} id: {0}</a></td>",
            fort.id.0, fort.name
        ));
        i += 1;
        if i > 2 {
            list.push_str("</tr><tr>");
            i = 0;
        }
    }
    html.replace("%forts%", &list);
    world.send_admin_html(gm, &html);
}

/// Java `showFortSiegePage`.
fn show_fort_siege_page(world: &mut World, gm: ObjectId, fort: FortId) {
    let mut html = world.admin_html_file(gm, "data/html/admin/fort.htm");
    html.replace("%fortName%", &world.residence_name(ResidenceRef::Fort(fort)));
    html.replace("%fortId%", &fort.0.to_string());
    world.send_admin_html(gm, &html);
}
