//! `//search <text> [page]`: the item templates whose name contains the text, 15 a page in
//! `search.htm`, each with an "Add" button for `//create_item`. `//search` alone shows the empty
//! search page.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSearch.java
//
// The rows keep Java's markup as it is, including its literal backslashes in the `src` of the
// icon and in the grade font of the crystal grades (`NOTE(java-quirk)` below): the client shows
// them the way Java sends them.

use l2_data::item::{CrystalType, ItemTemplate};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSearch.PAGE_LIMIT`.
const PAGE_LIMIT: i64 = 15;
/// Java `Math.min(100, ...)`: at most 100 page links.
const MAX_PAGE_LINKS: i64 = 100;

/// Java `AdminSearch.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_search", admin_search)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminSearch.onCommand`: `StringTokenizer(command, " ")`, the text, then the page (a page
/// that is not a number is the first).
fn admin_search(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut tokens = Tokens::spaces(command);
    tokens.next_token();
    let Some(text) = tokens.next_token() else {
        let mut html = world.admin_html_file(gm, "data/html/admin/search.htm");
        html.replace("%items%", "");
        html.replace("%pages%", "");
        world.send_admin_html(gm, &html);
        return Ok(());
    };
    let mut page = 1;
    if let Some(value) = tokens.next_token() {
        page = parse_int(value).unwrap_or(1);
    }
    results(world, gm, text, page)
}

/// Java `AdminSearch.getItemGrade`: the grade of a crystal type, as the markup Java writes.
// NOTE(java-quirk): the grades other than NG are written with backslash-escaped quotes
// (`name=\"hs8\"`), and NG with plain quotes. Kept as they are.
fn item_grade(crystal: CrystalType) -> String {
    match crystal {
        CrystalType::None => "<font name=\"hs8\" color=ae9977 size=-1>NG</font>".to_owned(),
        other => format!(
            "<font name=\\\"hs8\\\" color=ae9977 size=-1>{}</font>",
            other.name()
        ),
    }
}

/// Java `AdminSearch.getFontedWord(text, name)`: the matched part of the name in the level colour.
fn fonted_word(text: &str, name: &str) -> String {
    let position = name.to_lowercase().find(&text.to_lowercase()).unwrap_or(0);
    let font = "<font color=\"LEVEL\">";
    let mut out = String::with_capacity(name.len() + font.len() + 7 + text.len());
    let at = byte_index(name, position);
    out.push_str(&name[..at]);
    out.push_str(font);
    let end = byte_index(name, position + text.len());
    out.push_str(&name[at..end]);
    out.push_str("</font>");
    out.push_str(&name[end..]);
    out
}

/// A byte index into `name` clamped to its end and moved to a character boundary.
fn byte_index(name: &str, index: usize) -> usize {
    let mut i = index.min(name.len());
    while !name.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// One row of the result table (Java's body handler).
fn item_row(item: &ItemTemplate, text: &str) -> String {
    let stack = if item.stackable {
        "<font name=\"hs8\" color=008000 size=-1>ST</font>"
    } else {
        "<font name=\"hs8\" color=ff0000 size=-1>ST</font>"
    };
    let icon = item.icon.as_deref().unwrap_or("null");
    format!(
        "<tr><td align=center><button value=\\\"Add\\\" action=\"bypass admin_create_item {}\" width=32 height=22 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td><td><img src=\\\"{icon}\" width=32 height=32></td><td>{}</td><td>{stack}</td><td align=center>{}</td><td>{}</td></tr>",
        item.id,
        item.display_id,
        item_grade(item.crystal_type),
        fonted_word(text, &item.name),
    )
}

/// Java `AdminSearch.results(player, text, page)`.
fn results(world: &mut World, gm: ObjectId, text: &str, page: i32) -> Outcome {
    let lowered = text.to_lowercase();
    let items: Vec<ItemTemplate> = world
        .item_templates()
        .map(|templates| {
            templates
                .iter()
                .filter(|item| item.name.to_lowercase().contains(&lowered))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let mut html = world.admin_html_file(gm, "data/html/admin/search.htm");
    if items.is_empty() {
        html.replace("%items%", &format!("<tr><td>No items found with word {text}.</td></tr>"));
        html.replace("%pages%", "");
        world.send_admin_html(gm, &html);
        return Ok(());
    }

    let total = items.len() as i64;
    let pages = (total + PAGE_LIMIT - 1) / PAGE_LIMIT;
    let max = MAX_PAGE_LINKS.min(pages);
    // `items.subList((page - 1) * 15, min(page * 15, size))` and the checks of `subList`.
    let from = (i64::from(page) - 1) * PAGE_LIMIT;
    let to = (i64::from(page) * PAGE_LIMIT).min(total);
    if from < 0 {
        return Err(JavaException(format!(
            "java.lang.IndexOutOfBoundsException: fromIndex = {from}"
        )));
    }
    if from > to {
        return Err(JavaException(format!(
            "java.lang.IllegalArgumentException: fromIndex({from}) > toIndex({to})"
        )));
    }

    let rows: String = items[from as usize..to as usize]
        .iter()
        .map(|item| item_row(item, text))
        .collect();
    html.replace("%items%", &rows);

    let mut links = String::new();
    for pagenr in 1..=max {
        if i64::from(page) == pagenr {
            links.push_str(&format!("{pagenr}&nbsp;"));
        } else {
            links.push_str(&format!(
                "<a action=\"bypass -h admin_search {text} {pagenr}\">{pagenr}</a>&nbsp;"
            ));
        }
    }
    html.replace("%pages%", &links);
    world.send_admin_html(gm, &html);
    Ok(())
}
