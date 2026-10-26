//! `//reload`, `//reload_menu`, `//reload_menu_page`: the reload panel and what the Rust server
//! can reload while it runs.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminReload.java
// Java: gameserver/cache/HtmCache.java (reload)
//
// # What reloads
// - The panel (`//reload`, `//reload_menu`, `//reload_menu_page <n>`): all of it, with Java's
//   56 buttons and its pager.
// - `//reload htm [path]` and `//reload html [path]`: the html cache. The files are read by the
//   game server task (never inside a tick, D-15): the world asks with
//   `ControlRequest::ReloadHtml` and gets `WorldCommand::HtmlReloaded` back. NOTE: Java reloads
//   the one folder or file named; the Rust cache is always preloaded as a whole (see
//   `HtmCache::load_with`), so the whole cache is read again and the answer only names the file.
//
// # What does not
// Every other type of Java's switch reads data the Rust server holds as immutable, shared
// tables (`Arc`) built once at the start, or runs scripts (D-16). `//reload <type>` for those
// answers that it needs a restart. The types: access, buylist, category, classlist, config,
// crest, cubic, cw, door, dynamicexprate, effect, elementalattribute, enchant, enchantitemhpbonus,
// enchantskillgroups, experience, experienceloss, fakeplayerchat, fence, fish, fishingmonsters,
// fishingrods, handler, henna, hitconditionbonus, initialequipment, initialshortcut, instance,
// item, itemmall, primeshop, karmaloss, levelupcrystal, localisations, mapregion, multisell, npc,
// options, petdata, petskill, playertemplate, quest, recipe, secondaryauth, sets, siegeschedule,
// skill, skilllearn, skilltree, spawn, staticobject, teleport, transform, walker, zone.

use std::sync::Arc;

use l2_data::HtmCache;
use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::outbox::ControlRequest;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// The answer to `ControlRequest::ReloadHtml`.
#[derive(Clone)]
pub enum HtmlReloadResult {
    /// The html files were read again.
    Reloaded(Arc<HtmCache>),
    /// `//reload html <path>` named a file or folder that does not exist.
    MissingFile,
    /// The files could not be read.
    Failed,
}

impl std::fmt::Debug for HtmlReloadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reloaded(cache) => write!(f, "Reloaded({} files)", cache.loaded_files()),
            Self::MissingFile => f.write_str("MissingFile"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}

/// Java `AdminReload.RELOAD_COMMANDS`.
const RELOAD_COMMANDS: &[(&str, &str)] = &[
    ("Access", "admin_reload access"),
    ("Armor Sets", "admin_reload sets"),
    ("BuyList", "admin_reload buylist"),
    ("Category", "admin_reload category"),
    ("Class List", "admin_reload classlist"),
    ("Config", "admin_reload config"),
    ("Crests", "admin_reload crest"),
    ("Cubic", "admin_reload cubic"),
    ("Cursed Weapons", "admin_reload cw"),
    ("Doors", "admin_reload door"),
    ("Dynamic Exp Rate", "admin_reload dynamicexprate"),
    ("Effects", "admin_reload effect"),
    ("Elem. Attribute", "admin_reload elementalattribute"),
    ("Ench Item HP Bonus", "admin_reload enchantitemhpbonus"),
    ("Ench Skill Groups", "admin_reload enchantskillgroups"),
    ("Enchant", "admin_reload enchant"),
    ("Experience", "admin_reload experience"),
    ("Experience Loss", "admin_reload experienceloss"),
    ("Fake Player Chat", "admin_reload fakeplayerchat"),
    ("Fence", "admin_reload fence"),
    ("Fish", "admin_reload fish"),
    ("Fishing Monsters", "admin_reload fishingmonsters"),
    ("Fishing Rods", "admin_reload fishingrods"),
    ("Handler", "admin_reload handler"),
    ("Henna", "admin_reload henna"),
    ("Hit Condition Bonus", "admin_reload hitconditionbonus"),
    ("HTM (path/x.htm)", "admin_reload htm $value"),
    ("HTML (path/x.htm)", "admin_reload html $value"),
    ("Initial Equipment", "admin_reload initialequipment"),
    ("Initial Shortcut", "admin_reload initialshortcut"),
    ("Instance", "admin_reload instance"),
    ("Item", "admin_reload item"),
    ("Item Mall", "admin_reload itemmall"),
    ("Karma Loss", "admin_reload karmaloss"),
    ("L2Store", "admin_reload primeshop"),
    ("LevelUp Crystal", "admin_reload levelupcrystal"),
    ("Localisations", "admin_reload localisations"),
    ("Map Region", "admin_reload mapregion"),
    ("Multisell", "admin_reload multisell"),
    ("NPC", "admin_reload npc $value"),
    ("Options", "admin_reload options"),
    ("Pet Data", "admin_reload petdata"),
    ("Pet Skill", "admin_reload petskill"),
    ("Player Template", "admin_reload playertemplate"),
    ("Quest", "admin_reload quest $value"),
    ("Recipe", "admin_reload recipe"),
    ("Secondary Auth", "admin_reload secondaryauth"),
    ("Siege Schedule", "admin_reload siegeschedule"),
    ("Skill", "admin_reload skill"),
    ("Skill Learn", "admin_reload skilllearn"),
    ("Skill Tree", "admin_reload skilltree"),
    ("Spawn", "admin_reload spawn"),
    ("Static Objects", "admin_reload staticobject"),
    ("Teleport", "admin_reload teleport"),
    ("Transform", "admin_reload transform"),
    ("Walker", "admin_reload walker"),
    ("Zone", "admin_reload zone"),
];

/// The types of Java's switch the Rust server cannot reload (see the module comment).
const NOT_RELOADABLE: &[&str] = &[
    "access",
    "buylist",
    "category",
    "classlist",
    "config",
    "crest",
    "cubic",
    "cw",
    "door",
    "dynamicexprate",
    "effect",
    "elementalattribute",
    "enchant",
    "enchantitemhpbonus",
    "enchantskillgroups",
    "experience",
    "experienceloss",
    "fakeplayerchat",
    "fence",
    "fish",
    "fishingmonsters",
    "fishingrods",
    "handler",
    "henna",
    "hitconditionbonus",
    "initialequipment",
    "initialshortcut",
    "instance",
    "item",
    "itemmall",
    "karmaloss",
    "levelupcrystal",
    "localisations",
    "mapregion",
    "multisell",
    "npc",
    "options",
    "petdata",
    "petskill",
    "playertemplate",
    "primeshop",
    "quest",
    "recipe",
    "secondaryauth",
    "sets",
    "siegeschedule",
    "skill",
    "skilllearn",
    "skilltree",
    "spawn",
    "staticobject",
    "teleport",
    "transform",
    "walker",
    "zone",
];

/// Java `ITEMS_PER_PAGE`: two columns of eight rows.
const ITEMS_PER_PAGE: i32 = 16;

const RELOAD_USAGE: &str = "Usage: //reload <type> [parameters]";

/// Java `AdminReload` registration.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_reload;
    for command in ["admin_reload", "admin_reload_menu", "admin_reload_menu_page"] {
        handlers.register(command, handler);
    }
}

/// Java `AdminReload.onCommand`.
fn admin_reload(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let actual = st.next_token().unwrap_or_default();
    if actual.eq_ignore_ascii_case("admin_reload_menu") {
        return show_reload_menu(world, gm, 0);
    }
    if actual.eq_ignore_ascii_case("admin_reload_menu_page") {
        let page = match st.next_token() {
            Some(token) => parse_int(token)?,
            None => 0,
        };
        return show_reload_menu(world, gm, page);
    }
    if actual.eq_ignore_ascii_case("admin_reload") {
        let Some(kind) = st.next_token() else {
            return show_reload_menu(world, gm, 0);
        };
        let kind = kind.to_lowercase();
        match kind.as_str() {
            "htm" | "html" => {
                let path = st.next_token().map(str::to_owned);
                world
                    .outbox
                    .control(ControlRequest::ReloadHtml { requester: gm, path });
                // The answer (`on_html_reloaded`) says the rest.
                return Ok(());
            }
            other if NOT_RELOADABLE.contains(&other) => {
                world.send_sys_message(
                    gm,
                    &format!("'{other}' cannot be reloaded while the server runs: restart the server."),
                );
            }
            _ => {
                world.send_message(gm, RELOAD_USAGE);
                return Ok(());
            }
        }
        reload_warning(world, gm);
    }
    Ok(())
}

fn reload_warning(world: &mut World, gm: ObjectId) {
    world.send_sys_message(
        gm,
        "WARNING: There are several known issues regarding this feature. Reloading server data during runtime is STRONGLY NOT RECOMMENDED for live servers, just for developing environments.",
    );
}

impl World {
    /// The answer to `ControlRequest::ReloadHtml`: swaps the cache and says what Java says.
    pub(crate) fn on_html_reloaded(
        &mut self,
        requester: ObjectId,
        path: Option<String>,
        result: HtmlReloadResult,
    ) {
        match result {
            HtmlReloadResult::Reloaded(cache) => {
                let usage = ((cache.memory_mb() * 1_048_576.0) as f32 / 1_048_576.0_f32) as f64;
                let files = cache.loaded_files();
                self.talk.html = Some(cache);
                let name = self.object_name(requester).unwrap_or_default();
                match path {
                    Some(path) => {
                        let file = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned();
                        self.broadcast_message_to_gms(&format!("{name}: Reloaded Htm File:{file}."));
                    }
                    None => {
                        self.send_sys_message(
                            requester,
                            &format!("Cache[HTML]: {usage} megabytes on {files} files loaded"),
                        );
                        self.broadcast_message_to_gms(&format!("{name}: Reloaded Htms."));
                    }
                }
            }
            HtmlReloadResult::MissingFile => {
                self.send_sys_message(requester, "File or Directory does not exist.");
            }
            HtmlReloadResult::Failed => {
                warn!("//reload html: the html files could not be read.");
                self.send_sys_message(requester, "The html files could not be read.");
            }
        }
        if self.players.contains_key(&requester) {
            reload_warning(self, requester);
        }
    }
}

/// Java `AdminReload.showReloadMenu`.
fn show_reload_menu(world: &mut World, gm: ObjectId, page: i32) -> Outcome {
    let Some(content) = world.get_htm(gm, "data/html/admin/reload.htm") else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let buttons = generate_reload_buttons(page)?;
    let content = content.replace("%buttons%", &buttons);
    // `new NpcHtmlMessage(0, 1, content)`.
    if let Some(message) = NpcHtmlMessage::new_item(0, 1, &content) {
        world.send_html(gm, &message);
    }
    Ok(())
}

/// Java `AdminReload.generateReloadButtons`. A page before the first one is the
/// `ArrayIndexOutOfBoundsException` of `RELOAD_COMMANDS.get(negative)`.
fn generate_reload_buttons(page: i32) -> Result<String, JavaException> {
    let total = RELOAD_COMMANDS.len() as i32;
    let total_pages = (total + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let start = page.wrapping_mul(ITEMS_PER_PAGE);
    let end = (start.wrapping_add(ITEMS_PER_PAGE)).min(total);

    let mut html = String::new();
    // Value input field.
    html.push_str("<table width=260 cellspacing=0 cellpadding=0><tr>");
    html.push_str("<td fixwidth=50>Value:</td>");
    html.push_str("<td fixwidth=150 height=25><edit var=\"value\" width=148></td>");
    html.push_str("</tr></table><br>");

    // 2-column button grid.
    html.push_str("<table width=260>");
    let mut i = start;
    while i < end {
        html.push_str("<tr>");
        for j in i..(i + 2).min(end) {
            let Some((name, action)) = usize::try_from(j).ok().and_then(|j| RELOAD_COMMANDS.get(j)) else {
                return Err(JavaException(format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {j} out of bounds for length {total}"
                )));
            };
            html.push_str(&format!("<td><button value=\"{name}\" "));
            html.push_str(&format!("action=\"bypass {action}\" "));
            html.push_str(
                "width=125 height=22 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>",
            );
        }
        if (end - start) % 2 != 0 && i + 2 > end {
            html.push_str("<td></td>");
        }
        html.push_str("</tr>");
        i += 2;
    }
    html.push_str("</table>");

    // Pagination bar.
    if total_pages > 1 {
        html.push_str("<table width=260><tr>");
        html.push_str("<td width=80 align=center>");
        if page > 0 {
            html.push_str(&format!(
                "<button value=\"\u{25C4} Prev\" action=\"bypass -h admin_reload_menu_page {}\" width=75 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">",
                page - 1
            ));
        } else {
            html.push_str("<button value=\"\u{25C4} Prev\" action=\"\" width=75 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">");
        }
        html.push_str("</td>");
        html.push_str(&format!(
            "<td width=100 align=center><font color=\"LEVEL\">Page {} / {}</font></td>",
            page.wrapping_add(1),
            total_pages
        ));
        html.push_str("<td width=80 align=center>");
        if page < total_pages - 1 {
            html.push_str(&format!(
                "<button value=\"Next \u{25BA}\" action=\"bypass -h admin_reload_menu_page {}\" width=75 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">",
                page + 1
            ));
        } else {
            html.push_str("<button value=\"Next \u{25BA}\" action=\"\" width=75 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">");
        }
        html.push_str("</td>");
        html.push_str("</tr></table>");
    }
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_page_has_sixteen_buttons_and_a_next_link() {
        let html = generate_reload_buttons(0).unwrap();
        assert_eq!(html.matches("<button value=\"").count() - 2, 16);
        assert!(html.contains("admin_reload access"));
        assert!(html.contains("Page 1 / 4"));
        assert!(html.contains("admin_reload_menu_page 1"));
        assert!(!html.contains("admin_reload_menu_page -1"));
    }

    #[test]
    fn the_last_page_has_the_rest() {
        // 56 commands: three full pages and eight on the fourth.
        let html = generate_reload_buttons(3).unwrap();
        assert_eq!(html.matches("action=\"bypass admin_reload ").count(), 8);
        assert!(html.contains("Page 4 / 4"));
        assert!(html.contains("admin_reload_menu_page 2"));
        // A page past the end draws no buttons and no error.
        assert!(generate_reload_buttons(9).unwrap().contains("Page 10 / 4"));
    }

    #[test]
    fn a_negative_page_is_the_java_exception() {
        let error = generate_reload_buttons(-1).unwrap_err();
        assert_eq!(
            error.0,
            "java.lang.ArrayIndexOutOfBoundsException: Index -16 out of bounds for length 56"
        );
    }

    #[test]
    fn every_button_but_html_is_a_known_type() {
        for (_, action) in RELOAD_COMMANDS {
            let kind = action.split(' ').nth(1).unwrap();
            assert!(
                NOT_RELOADABLE.contains(&kind) || kind == "htm" || kind == "html",
                "{kind}"
            );
        }
    }
}
