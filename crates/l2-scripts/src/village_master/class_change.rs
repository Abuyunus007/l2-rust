//! What the class change scripts of `village_master` share: the page whitelist of their
//! `onEvent`, the player's class and categories, and the `ClassChangeRequested` branch that
//! twelve of them repeat once per class (a table here, one row per Java `else if`).
// Java: dist/game/data/scripts/village_master/*Change{1,2}/*.java (ClassChangeRequested)
//
// Not a Java script: a helper module of the folder. The rows of each script's table are its
// `else if ((classId == X) && (player.getPlayerClass() == PlayerClass.Y))` blocks in Java's order,
// with the four pages of the block in the order Java tests them.

use l2_util::ObjectId;
use l2_world::ScriptCtx;
use l2_world::subclass::CategoryType;

/// Java `SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE`: the reward of the 1st class transfer.
pub(crate) const SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE: i32 = 8869;
/// Java `SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE`: the reward of the 2nd class transfer.
pub(crate) const SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE: i32 = 8870;

/// One `else if` block of `ClassChangeRequested`.
pub(crate) struct Transfer {
    /// Java `classId == X`: the class the link asks for.
    pub class_id: i32,
    /// Java `player.getPlayerClass() == PlayerClass.Y`: the class the player must be.
    pub from: i32,
    /// Java `hasQuestItems(player, ...)`: every one of them is needed, and all are taken.
    pub proofs: &'static [i32],
    /// The page numbers of the block: low level with the proofs (`fnLowLevel`), low level without
    /// (`fnLowLevelNoProof`), the class change done (`fnAfterClassChange`), no proofs (`fnNoProof`).
    pub pages: [i32; 4],
}

/// Java `npc.getId() + "-NN.htm"` (or a fixed NPC id): the file name of a page.
pub(crate) fn page(prefix: i32, number: i32) -> String {
    format!("{prefix}-{number:02}.htm")
}

/// The `case "<npc>-NN.htm":` labels of an `onEvent` switch: `event` is the page `NN` of one of
/// `npcs`, for `NN` in `numbers`.
pub(crate) fn is_listed_page(event: &str, npcs: &[i32], numbers: &[i32]) -> bool {
    npcs.iter()
        .any(|&npc| numbers.iter().any(|&n| page(npc, n) == event))
}

/// Java `StringUtil.isNumeric(text)`: not empty, digits only.
// NOTE: Java's `Character.isDigit` also takes the digits of other scripts; the pages only link
// ASCII numbers (and the html action check drops any other link).
pub(crate) fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Java `player.getPlayerClass().getId()`.
pub(crate) fn class_of(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world().player(player).map_or(-1, |p| p.data.class_id)
}

/// Java `player.isInCategory(type)`.
pub(crate) fn in_category(ctx: &ScriptCtx<'_>, player: ObjectId, category: CategoryType) -> bool {
    ctx.world().is_in_category(player, category)
}

/// Java `player.getPlayerClass().level()`.
pub(crate) fn class_level(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    u16::try_from(class_of(ctx, player)).map_or(0, |c| l2_world::subclass::ClassId(c).class_level())
}

/// The class change of a village master, in the Java order of the `*Change1/2` scripts:
/// `takeItems`, `setPlayerClass`, `setBaseClass`, `broadcastUserInfo`, `giveItems(coupon, 15)`.
/// ("SystemMessage and cast skill is done by setClassId.")
pub(crate) fn change_class(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    class_id: i32,
    proofs: &[i32],
    coupon: i32,
) {
    ctx.take_items_many(player, -1, proofs);
    ctx.world_mut().set_player_class(player, class_id);
    ctx.world_mut().set_base_class(player, class_id);
    ctx.world_mut().broadcast_user_info(player);
    ctx.give_items(player, coupon, 15);
}

/// The `else if` blocks of `ClassChangeRequested` after the category checks: the first row whose
/// class and source class match decides the page. `prefix` is the NPC id the pages start with,
/// `min_level` the level of the transfer (20 or 40). `None` when no row matches (Java's
/// `htmltext = null`).
pub(crate) fn transfer(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    prefix: i32,
    rows: &[Transfer],
    class_id: i32,
    min_level: i32,
    coupon: i32,
) -> Option<String> {
    let current = class_of(ctx, player);
    let row = rows
        .iter()
        .find(|r| r.class_id == class_id && r.from == current)?;
    let [low_level, low_level_no_proof, after_class_change, no_proof] = row.pages;
    let proofs = ctx.has_all_quest_items(player, row.proofs);
    let number = if ctx.player_level(player) < min_level {
        if proofs { low_level } else { low_level_no_proof }
    } else if proofs {
        change_class(ctx, player, row.class_id, row.proofs, coupon);
        after_class_change
    } else {
        no_proof
    };
    Some(page(prefix, number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_pages_have_two_digits() {
        assert!(is_listed_page("30066-01.htm", &[30066, 30288], &[1, 2]));
        assert!(is_listed_page("30288-02.htm", &[30066, 30288], &[1, 2]));
        assert!(!is_listed_page("30288-03.htm", &[30066, 30288], &[1, 2]));
        assert!(!is_listed_page("30066-1.htm", &[30066], &[1]));
        assert!(!is_listed_page("30066-01.html", &[30066], &[1]));
    }
}
