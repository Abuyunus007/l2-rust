//! Mulia and Ilia, the clan traders: Blood Alliance, Blood Oath and Epaulettes for reputation.
// Java: dist/game/data/scripts/ai/others/ClanTrader/ClanTrader.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const CLAN_TRADER: [i32; 2] = [
    32024, // Mulia
    32025, // Ilia
];

// Items
const BLOOD_ALLIANCE: i32 = 9911; // Blood Alliance
const BLOOD_ALLIANCE_COUNT: i64 = 1; // Blood Alliance Count
const BLOOD_OATH: i32 = 9910; // Blood Oath
const BLOOD_OATH_COUNT: i64 = 10; // Blood Oath Count
const KNIGHTS_EPAULETTE: i32 = 9912; // Knight's Epaulette
const KNIGHTS_EPAULETTE_COUNT: i64 = 100; // Knight's Epaulette Count

/// Java `ai.others.ClanTrader.ClanTrader`.
pub struct ClanTrader;

/// Java `ClanTrader.giveReputation(npc, player, count, itemId, itemCount)`.
fn give_reputation(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
    count: i32,
    item_id: i32,
    item_count: i64,
) -> String {
    if ctx.get_quest_items_count(player, item_id) >= item_count {
        ctx.take_items(player, item_id, item_count);
        ctx.add_clan_reputation_score(player, count);
        ctx.send_system_message_int(
            player,
            SystemMessageId::YourClanHasAdded1sPointsToItsClanReputationScore,
            count,
        );
        return format!("{}-04.html", npc.id);
    }

    format!("{}-03.html", npc.id)
}

impl Script for ClanTrader {
    fn name(&self) -> &'static str {
        "ClanTrader"
    }

    fn path(&self) -> &'static str {
        "ai/others/ClanTrader"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&CLAN_TRADER);
        s.add_talk_id(&CLAN_TRADER);
        s.add_first_talk_id(&CLAN_TRADER);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "32024.html" | "32024-02.html" | "32025.html" | "32025-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "repinfo" => {
                let (player, npc) = (player?, npc?);
                // NOTE(java-quirk): `player.getClan().getLevel()` throws without a clan; the
                // link is only on the clan leader's page.
                let level = ctx.clan_level_of(player)?;
                htmltext = Some(if level > 4 {
                    format!("{}-02.html", npc.id)
                } else {
                    format!("{}-05.html", npc.id)
                });
            }
            "exchange-ba" => {
                let (player, npc) = (player?, npc?);
                ctx.clan_level_of(player)?;
                let points = ctx.script_settings().bloodalliance_points;
                htmltext = Some(give_reputation(
                    ctx,
                    npc,
                    player,
                    points,
                    BLOOD_ALLIANCE,
                    BLOOD_ALLIANCE_COUNT,
                ));
            }
            "exchange-bo" => {
                let (player, npc) = (player?, npc?);
                ctx.clan_level_of(player)?;
                let points = ctx.script_settings().bloodoath_points;
                htmltext = Some(give_reputation(
                    ctx,
                    npc,
                    player,
                    points,
                    BLOOD_OATH,
                    BLOOD_OATH_COUNT,
                ));
            }
            "exchange-ke" => {
                let (player, npc) = (player?, npc?);
                ctx.clan_level_of(player)?;
                let points = ctx.script_settings().knightsepaulette_points;
                htmltext = Some(give_reputation(
                    ctx,
                    npc,
                    player,
                    points,
                    KNIGHTS_EPAULETTE,
                    KNIGHTS_EPAULETTE_COUNT,
                ));
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.is_clan_leader(player) || ctx.has_member_fame_access(player) {
            return Some(format!("{}.html", npc.id));
        }

        Some(format!("{}-01.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32024" type="Folk" level="70" name="Mulia"/>"#;
    const DIR: &str = "data/scripts/ai/others/ClanTrader";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ClanTrader);
        let mut pages = Vec::new();
        for n in ["", "-01", "-02", "-03", "-04", "-05"] {
            pages.push((
                format!("{DIR}/32024{n}.html"),
                format!(
                    "<html><body>page{n}<a action=\"bypass -h Script ClanTrader repinfo\">r</a></body></html>"
                ),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let npc = t.spawn(32024, 20);
        t.enter(1, 60, vec![]);
        t.take();
        (t, npc)
    }

    #[test]
    fn a_player_without_a_clan_gets_the_refusal_page() {
        let (mut t, npc) = setup();
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page-01"));
    }

    #[test]
    fn the_exchange_links_need_a_clan() {
        // Java throws a NullPointerException; nothing happens here.
        let (mut t, npc) = setup();
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script ClanTrader repinfo");
        assert!(pages_to(&t.take(), 1).is_empty());
    }
}
