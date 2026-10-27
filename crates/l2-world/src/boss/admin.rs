//! The admin commands `//grandboss [id]`, `//grandboss_skip`, `//grandboss_respawn`,
//! `//grandboss_minions` and `//grandboss_abort`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminGrandBoss.java
//
// The page shows the state of one boss. The row of the boss is read again first (D-53, D-50): the
// Java game server decides the states while it runs the bosses, and the page must show that. The
// read goes out through the outbox; its answer (`WorldCommand::BossReread`) shows the page.
//
// `skip`, `respawn`, `minions` and `abort` send an event to the Antharas or Baium script
// (`notifyEvent`); the scripts are D-05, so the event is not delivered and the GM is told.

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::{BossReadKind, BossReadThen};
use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::outbox::PersistCommand;
use crate::world::World;

type Outcome = Result<(), JavaException>;

const ANTHARAS: i32 = 29068;
const ANTHARAS_ZONE: i32 = 70050;
const VALAKAS: i32 = 29028;
const VALAKAS_ZONE: i32 = 70052;
const BAIUM: i32 = 29020;
const BAIUM_ZONE: i32 = 70051;
const QUEEN_ANT: i32 = 29001;
const ORFEN: i32 = 29014;
const CORE: i32 = 29006;
const ZAKEN: i32 = 29022;
const ZAKEN_ZONE: i32 = 70053;
const FRINTEZZA: i32 = 29045;
const FRINTEZZA_ZONE: i32 = 70054;

/// The type `AdminGrandBoss` asks `ZoneManager.getZoneById` for.
const NO_RESTART_ZONE: &str = "NoRestartZone";

const MENU_PAGE: &str = "data/html/admin/grandboss/grandboss.htm";

/// Java `AdminGrandBoss.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_grandboss", admin_grandboss),
        ("admin_grandboss_skip", admin_grandboss),
        ("admin_grandboss_respawn", admin_grandboss),
        ("admin_grandboss_minions", admin_grandboss),
        ("admin_grandboss_abort", admin_grandboss),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminGrandBoss.onCommand`.
fn admin_grandboss(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let actual = st.next_token().unwrap_or("").to_lowercase();
    match actual.as_str() {
        "admin_grandboss" => {
            if st.has_more() {
                let boss_id = parse_int(st.next_token().unwrap_or(""))?;
                manage_html(world, gm, boss_id);
            } else {
                // `new NpcHtmlMessage(0, 1)` + `setHtml(HtmCache.getHtm(..))`.
                if let Some(html) = world.get_htm(gm, MENU_PAGE) {
                    send_page(world, gm, html);
                }
            }
        }
        "admin_grandboss_skip" => {
            if st.has_more() {
                let boss_id = parse_int(st.next_token().unwrap_or(""))?;
                if boss_id == ANTHARAS {
                    // FOLLOW-UP(D-05): `antharasAi().notifyEvent("SKIP_WAITING", null, activeChar)`.
                    script_not_ported(world, gm);
                    manage_html(world, gm, boss_id);
                } else {
                    world.send_sys_message(gm, "Wrong ID!");
                }
            } else {
                world.send_sys_message(gm, "Usage: //grandboss_skip Id");
            }
        }
        "admin_grandboss_respawn" => {
            if st.has_more() {
                let boss_id = parse_int(st.next_token().unwrap_or(""))?;
                match boss_id {
                    // FOLLOW-UP(D-05): `notifyEvent("RESPAWN_ANTHARAS" / "RESPAWN_BAIUM", ..)`.
                    ANTHARAS | BAIUM => {
                        script_not_ported(world, gm);
                        manage_html(world, gm, boss_id);
                    }
                    _ => world.send_sys_message(gm, "Wrong ID!"),
                }
            } else {
                world.send_sys_message(gm, "Usage: //grandboss_respawn Id");
            }
        }
        "admin_grandboss_minions" => {
            if st.has_more() {
                let boss_id = parse_int(st.next_token().unwrap_or(""))?;
                match boss_id {
                    // FOLLOW-UP(D-05): `notifyEvent("DESPAWN_MINIONS", ..)`.
                    ANTHARAS | BAIUM => script_not_ported(world, gm),
                    _ => world.send_sys_message(gm, "Wrong ID!"),
                }
            } else {
                world.send_sys_message(gm, "Usage: //grandboss_minions Id");
            }
        }
        "admin_grandboss_abort" => {
            if st.has_more() {
                let boss_id = parse_int(st.next_token().unwrap_or(""))?;
                match boss_id {
                    // FOLLOW-UP(D-05): `notifyEvent("ABORT_FIGHT", ..)`.
                    ANTHARAS | BAIUM => {
                        script_not_ported(world, gm);
                        manage_html(world, gm, boss_id);
                    }
                    _ => world.send_sys_message(gm, "Wrong ID!"),
                }
            } else {
                world.send_sys_message(gm, "Usage: //grandboss_abort Id");
            }
        }
        _ => {}
    }
    Ok(())
}

fn script_not_ported(world: &World, gm: ObjectId) {
    world.send_sys_message(gm, "The boss scripts are not ported yet; the event was not sent.");
}

/// `new NpcHtmlMessage(0, 1)` + `setHtml`.
fn send_page(world: &mut World, gm: ObjectId, html: String) {
    if let Some(mut message) = NpcHtmlMessage::new(0, &html) {
        message.item_id = 1;
        world.send_html(gm, &message);
    }
}

fn is_grand_boss_page(boss_id: i32) -> bool {
    [ANTHARAS, VALAKAS, BAIUM, QUEEN_ANT, ORFEN, CORE, ZAKEN, FRINTEZZA].contains(&boss_id)
}

/// Java `manageHtml(activeChar, grandBossId)`: checks the id and asks for the row (D-50); the page
/// follows when the answer is in ([`World::show_grand_boss_page`]).
fn manage_html(world: &mut World, gm: ObjectId, boss_id: i32) {
    if !is_grand_boss_page(boss_id) {
        world.send_sys_message(gm, "Wrong ID!");
        return;
    }
    world.outbox.persist(PersistCommand::BossRereadRequest {
        kind: BossReadKind::GrandBoss(boss_id),
        then: BossReadThen::ShowGrandBoss { gm, boss_id },
    });
}

/// The text of the NullPointerException Java throws when a map has no entry to unbox.
fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

impl World {
    /// The second half of `AdminGrandBoss.manageHtml`: the page of one boss with its state, once
    /// the row is read. A Java exception (a boss without a row) goes to the GM like the dispatcher
    /// does for any handler.
    pub(crate) fn show_grand_boss_page(&mut self, gm: ObjectId, boss_id: i32) {
        match self.grand_boss_page(gm, boss_id) {
            Ok(Some(html)) => send_page(self, gm, html),
            Ok(None) => {}
            Err(e) => self.send_sys_message(gm, &e.to_string()),
        }
    }

    fn grand_boss_page(&self, gm: ObjectId, boss_id: i32) -> Result<Option<String>, JavaException> {
        let status = self.grand_boss_status(boss_id).ok_or_else(null_pointer)?;
        let (zone, page) = match boss_id {
            ANTHARAS => (Some(ANTHARAS_ZONE), "grandboss_antharas.htm"),
            VALAKAS => (Some(VALAKAS_ZONE), "grandboss_valakas.htm"),
            BAIUM => (Some(BAIUM_ZONE), "grandboss_baium.htm"),
            QUEEN_ANT => (None, "grandboss_queenant.htm"),
            ORFEN => (None, "grandboss_orfen.htm"),
            CORE => (None, "grandboss_core.htm"),
            ZAKEN => (Some(ZAKEN_ZONE), "grandboss_zaken.htm"),
            _ => (Some(FRINTEZZA_ZONE), "grandboss_frintezza.htm"),
        };
        let (dead_status, color_text) = if [ANTHARAS, VALAKAS, BAIUM, FRINTEZZA].contains(&boss_id) {
            (
                3,
                match status {
                    0 => Some(("00FF00", "Alive")),
                    1 => Some(("FFFF00", "Waiting")),
                    2 => Some(("FF9900", "In Fight")),
                    3 => Some(("FF0000", "Dead")),
                    _ => None,
                },
            )
        } else {
            (
                1,
                match status {
                    0 => Some(("00FF00", "Alive")),
                    1 => Some(("FF0000", "Dead")),
                    _ => None,
                },
            )
        };
        // `replace("%bossStatus%", null)` throws.
        let (color, text) = color_text.ok_or_else(null_pointer)?;
        let info = self.grand_boss_info(boss_id).ok_or_else(null_pointer)?;
        let offset = l2_util::time::local_offset_millis(info.respawn_time);
        let respawn = crate::clanhall::siege::schedule::format_date_time(info.respawn_time, offset);
        let Some(template) = self.get_htm(gm, &format!("data/html/admin/grandboss/{page}")) else {
            return Ok(None);
        };
        // `ZoneManager.getZoneById(zoneId, NoRestartZone.class)`, then `getPlayersInside().size()`.
        let inside = zone
            .filter(|id| {
                self.zoning
                    .zones
                    .as_ref()
                    .is_some_and(|z| z.get_of_type(*id, NO_RESTART_ZONE).is_some())
            })
            .map(|id| {
                self.players
                    .values()
                    .filter(|p| {
                        p.zones
                            .members()
                            .iter()
                            .any(|(zid, ty)| *zid == id && ty == NO_RESTART_ZONE)
                    })
                    .count()
                    .to_string()
            })
            .unwrap_or_else(|| "Zone not found!".to_owned());
        let html = template
            .replace("%bossStatus%", text)
            .replace("%bossColor%", color)
            .replace(
                "%respawnTime%",
                if status == dead_status {
                    &respawn
                } else {
                    "Already respawned!"
                },
            )
            .replace("%playersInside%", &inside);
        Ok(Some(html))
    }
}
