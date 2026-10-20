//! The html side of the admin pages: Java's `NpcHtmlMessage.setFile` + `replace`, and the pager
//! of the lists (`PageBuilder` with the default page handler and the `ButtonsStyle`).
// Java: network/serverpackets/AbstractHtmlPacket.java (setFile, replace, setHtml)
// Java: mechanics/html/{PageBuilder,PageResult,IBodyHandler}.java
// Java: mechanics/html/pagehandlers/DefaultPageHandler.java, mechanics/html/styles/ButtonsStyle.java
// Java: mechanics/html/formatters/DefaultFormatter.java
//
// Only the combination the admin handlers use exists: the default formatter (`<bypass> <page>`),
// the default page handler (offset 2) and the buttons style. The handlers of other panels can add
// the other styles next to them.

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::warn;

use crate::world::World;

/// A page under construction: Java's `NpcHtmlMessage` between `setFile` and `sendPacket`.
pub(crate) struct AdminHtml {
    message: NpcHtmlMessage,
}

impl AdminHtml {
    /// Java `new NpcHtmlMessage()` + `setHtml(html)`.
    pub(crate) fn from_html(html: &str) -> Self {
        // `NpcHtmlMessage::new` only refuses a negative object id; 0 never is.
        let message = NpcHtmlMessage::new(0, html).unwrap_or(NpcHtmlMessage {
            npc_obj_id: 0,
            html: html.to_owned(),
            item_id: 0,
        });
        Self { message }
    }

    /// Java `AbstractHtmlPacket.replace(pattern, value)`: every occurrence of `pattern`. Java's
    /// `replaceAll` reads `pattern` as a regex, but every pattern the admin pages use is a plain
    /// `%word%`, and the value's `$` is escaped by Java itself, so this is a literal replace.
    pub(crate) fn replace(&mut self, pattern: &str, value: &str) -> &mut Self {
        self.message.html = self.message.html.replace(pattern, value);
        self
    }

    /// The page as it goes on the wire.
    pub(crate) fn message(&self) -> &NpcHtmlMessage {
        &self.message
    }
}

impl World {
    /// Java `new NpcHtmlMessage()` + `setFile(player, path)`: the page of the cache, or Java's
    /// "My Text is missing" page (and a warning) when the cache has no such file.
    pub(crate) fn admin_html_file(&self, player: ObjectId, path: &str) -> AdminHtml {
        match self.get_htm(player, path) {
            Some(html) => AdminHtml::from_html(&html),
            None => {
                warn!("NpcHtmlMessage: Missing html page {path}");
                AdminHtml::from_html(&format!(
                    "<html><body>My Text is missing:<br>{path}</body></html>"
                ))
            }
        }
    }

    /// `player.sendPacket(html)`.
    pub(crate) fn send_admin_html(&mut self, to: ObjectId, html: &AdminHtml) {
        self.send_html(to, html.message());
    }
}

// ----------------------------------------------------------------------------- PageBuilder

/// Java `PageResult`.
pub(crate) struct PageResult {
    /// `getPages()`.
    pub(crate) pages: i32,
    /// `getPagerTemplate()`.
    pub(crate) pager: String,
    /// `getBodyTemplate()`.
    pub(crate) body: String,
}

impl PageResult {
    /// The `%pages%` text every admin list uses: the pager in a table, or nothing for one page.
    pub(crate) fn pager_html(&self) -> String {
        if self.pages > 1 {
            format!("<table width=280 cellspacing=0><tr>{}</tr></table>", self.pager)
        } else {
            String::new()
        }
    }
}

/// `DefaultPageHandler.INSTANCE`: two pages at each end and around the current one.
const PAGES_OFFSET: i32 = 2;

/// `ButtonsStyle.applyBypass(bypass, name, isEnabled)`: the current page is a plain cell, the
/// others a button.
fn button(bypass: &str, name: i32, enabled: bool) -> String {
    if enabled {
        format!("<td>{name}</td>")
    } else {
        format!(
            "<td><button action=\"{bypass}\" value=\"{name}\" width=\"40\" height=\"15\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>"
        )
    }
}

/// `ButtonsStyle.applySeparator()`.
const SEPARATOR: &str = "<td align=center> | </td>";

/// `DefaultPageHandler.apply(bypass, currentPage, pages, ...)` with the `DefaultFormatter`
/// (`bypass + " " + page`) and the buttons style. `pages` is what `PageBuilder.build` passes: the
/// page count minus one.
// NOTE(java-quirk): `build` hands over `pages - 1`, so the last page gets no button when the
// current page is within two of it (three pages show only the first two buttons).
fn apply_pager(bypass: &str, current: i32, pages: i32, out: &mut String) {
    let link = |page: i32| button(&format!("{bypass} {page}"), page + 1, current == page);
    let start = (current - PAGES_OFFSET).max(0);
    let finish = (current + PAGES_OFFSET + 1).min(pages);

    // The first pages, in case we are in the middle or at the end.
    if start > PAGES_OFFSET {
        for i in 0..PAGES_OFFSET {
            out.push_str(&link(i));
        }
        out.push_str(SEPARATOR);
    }
    for i in start..finish {
        out.push_str(&link(i));
    }
    // The last pages.
    if pages > finish {
        out.push_str(SEPARATOR);
        for i in (pages - PAGES_OFFSET)..pages {
            out.push_str(&link(i));
        }
    }
}

/// Java `PageBuilder.newBuilder(elements, perPage, bypass).currentPage(page).style(ButtonsStyle)
/// .bodyHandler(body).build()`. `body` is Java's `IBodyHandler.apply(pages, element, sb)`.
pub(crate) fn build_page<T>(
    elements: &[T],
    per_page: i32,
    bypass: &str,
    current_page: i32,
    mut body: impl FnMut(&T, &mut String),
) -> PageResult {
    // `bypass.trim()`: every character up to U+0020.
    let bypass = bypass.trim_matches(|c: char| c <= ' ');
    let current = current_page.max(0);
    let count = i32::try_from(elements.len()).unwrap_or(i32::MAX);
    let pages = count / per_page + i32::from(count % per_page > 0);

    let mut pager = String::new();
    if pages > 1 {
        apply_pager(bypass, current, pages - 1, &mut pager);
    }
    // NOTE(java-quirk): the clamp compares with `pages`, not `pages - 1`, and comes after the
    // pager was built: a page number past `pages` shows the last page, but `pages` itself shows an
    // empty list.
    let current = if current > pages { pages - 1 } else { current };
    // Java's int product wraps for a huge page number.
    let start = usize::try_from(per_page.wrapping_mul(current).max(0)).unwrap_or(0);

    let mut out = String::new();
    for element in elements
        .iter()
        .skip(start)
        .take(usize::try_from(per_page).unwrap_or(0))
    {
        body(element, &mut out);
    }
    PageResult {
        pages,
        pager,
        body: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pager(current: i32, total_pages: i32) -> String {
        let mut out = String::new();
        apply_pager("bypass admin_x", current, total_pages - 1, &mut out);
        out
    }

    fn cell(page: i32, current: i32) -> String {
        button(&format!("bypass admin_x {page}"), page + 1, page == current)
    }

    #[test]
    fn the_current_page_is_a_plain_cell_and_the_others_buttons() {
        assert_eq!(cell(0, 0), "<td>1</td>");
        assert_eq!(
            cell(1, 0),
            "<td><button action=\"bypass admin_x 1\" value=\"2\" width=\"40\" height=\"15\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>"
        );
    }

    #[test]
    fn three_pages_show_only_two_buttons_like_java() {
        // `pages - 1 = 2` is the finish bound, so page index 2 is not shown.
        assert_eq!(pager(0, 3), format!("{}{}", cell(0, 0), cell(1, 0)));
    }

    #[test]
    fn many_pages_get_separators() {
        // 10 pages, current 5: start = 3 > 2 gives pages 0 and 1, a separator, 3..8, a separator,
        // then 7 and 8 (the last index 9 is again missing).
        let mut expected = String::new();
        for i in 0..2 {
            expected.push_str(&cell(i, 5));
        }
        expected.push_str(SEPARATOR);
        for i in 3..8 {
            expected.push_str(&cell(i, 5));
        }
        expected.push_str(SEPARATOR);
        for i in 7..9 {
            expected.push_str(&cell(i, 5));
        }
        assert_eq!(pager(5, 10), expected);
    }

    #[test]
    fn build_page_cuts_the_elements_of_the_page() {
        let items: Vec<i32> = (0..16).collect();
        let list = |page| {
            build_page(&items, 7, "bypass admin_list", page, |n, sb| {
                sb.push_str(&format!("[{n}]"));
            })
        };
        let first = list(0);
        assert_eq!(first.pages, 3);
        assert_eq!(first.body, "[0][1][2][3][4][5][6]");
        assert_eq!(list(1).body, "[7][8][9][10][11][12][13]");
        assert_eq!(list(2).body, "[14][15]");
        // A negative page is page 0; past the end is the last page, except `pages` itself.
        assert_eq!(list(-4).body, first.body);
        assert_eq!(list(9).body, "[14][15]");
        assert_eq!(list(3).body, "");
        assert!(first.pager.contains("bypass admin_list 1"));
        assert_eq!(
            first.pager_html(),
            format!("<table width=280 cellspacing=0><tr>{}</tr></table>", first.pager)
        );
        // One page has no pager.
        let single = build_page(&items[..3], 7, "bypass admin_list", 0, |n, sb| {
            sb.push_str(&n.to_string())
        });
        assert_eq!(single.pages, 1);
        assert_eq!(single.pager, "");
        assert_eq!(single.pager_html(), "");
    }

    #[test]
    fn an_empty_list_has_no_pages_and_the_bypass_is_trimmed() {
        let none: [i32; 0] = [];
        let page = build_page(&none, 7, " bypass admin_x ", 0, |_, _| {});
        assert_eq!(page.pages, 0);
        assert_eq!(page.body, "");

        let items: Vec<i32> = (0..22).collect();
        let four = build_page(&items, 7, "  bypass admin_x \n", 0, |_, _| {});
        assert_eq!(four.pages, 4);
        assert!(four.pager.contains("action=\"bypass admin_x 1\""));
    }
}
