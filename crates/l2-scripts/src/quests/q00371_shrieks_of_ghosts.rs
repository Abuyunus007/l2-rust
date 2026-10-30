//! Quest 371: Shrieks of Ghosts (Reva and Patrin in the Town of Dion, the Hallates monsters).
// Java: dist/game/data/scripts/quests/Q00371_ShrieksOfGhosts/Q00371_ShrieksOfGhosts.java
//
// Pattern: a repeatable kill-count quest with two drops per monster (`docs/design/scripts.md`,
// "Quest patterns"): one roll of 1000 gives an ancient ash urn, a second band an ancient porcelain.
// Reva pays for the urns, Patrin trades a porcelain for a quality one (or nothing). Only the urn is
// a registered quest item of this script, the porcelains stay after the quest like in Java.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const REVA: i32 = 30867;
const PATRIN: i32 = 30929;

// Items
const ANCIENT_ASH_URN: i32 = 5903;
const ANCIENT_PORCELAIN: i32 = 6002;
const ANCIENT_PORCELAIN_EXCELLENT: i32 = 6003;
const ANCIENT_PORCELAIN_HIGH_QUALITY: i32 = 6004;
const ANCIENT_PORCELAIN_LOW_QUALITY: i32 = 6005;
const ANCIENT_PORCELAIN_LOWEST_QUALITY: i32 = 6006;

// Misc
const MIN_LEVEL: i32 = 59;

/// Java `MOBS`: monster, the chance (out of 1000) of an urn, and the chance up to which a
/// porcelain drops.
const MOBS: [(i32, i32, i32); 3] = [
    (20818, 350, 400), // hallates_warrior
    (20820, 583, 673), // hallates_knight
    (20824, 458, 538), // hallates_commander
];

/// Java `Q00371_ShrieksOfGhosts`.
pub struct Q00371ShrieksOfGhosts;

impl Script for Q00371ShrieksOfGhosts {
    fn name(&self) -> &'static str {
        "Q00371_ShrieksOfGhosts"
    }

    fn path(&self) -> &'static str {
        "quests/Q00371_ShrieksOfGhosts"
    }

    fn id(&self) -> i32 {
        371
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[REVA]);
        s.add_talk_id(&[REVA, PATRIN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, ..)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[ANCIENT_ASH_URN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "30867-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30867-05.html" => {
                let urn_count = ctx.get_quest_items_count(player, ANCIENT_ASH_URN);
                if urn_count < 1 {
                    htmltext = Some(event.to_owned());
                } else if urn_count < 100 {
                    ctx.give_adena(player, urn_count * 1000 + 15000, true);
                    ctx.take_items(player, ANCIENT_ASH_URN, -1);
                    htmltext = Some("30867-06.html".to_owned());
                } else {
                    ctx.give_adena(player, urn_count * 1000 + 37700, true);
                    ctx.take_items(player, ANCIENT_ASH_URN, -1);
                    htmltext = Some("30867-07.html".to_owned());
                }
            }
            "30867-08.html" | "30929-01.html" | "30929-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30867-09.html" => {
                let urn_count = ctx.get_quest_items_count(player, ANCIENT_ASH_URN);
                ctx.give_adena(player, urn_count * 1000, true);
                qs.exit_quest(ctx, true, true);
                htmltext = Some("30867-09.html".to_owned());
            }
            "30929-03.html" => {
                if !ctx.has_quest_items(player, ANCIENT_PORCELAIN) {
                    htmltext = Some(event.to_owned());
                } else {
                    let random = ctx.get_random(100);
                    let (item, page) = if random < 2 {
                        (Some(ANCIENT_PORCELAIN_EXCELLENT), "30929-04.html")
                    } else if random < 32 {
                        (Some(ANCIENT_PORCELAIN_HIGH_QUALITY), "30929-05.html")
                    } else if random < 62 {
                        (Some(ANCIENT_PORCELAIN_LOW_QUALITY), "30929-06.html")
                    } else if random < 77 {
                        (Some(ANCIENT_PORCELAIN_LOWEST_QUALITY), "30929-07.html")
                    } else {
                        (None, "30929-08.html")
                    };
                    if let Some(item) = item {
                        ctx.give_items(player, item, 1);
                    }
                    ctx.take_items(player, ANCIENT_PORCELAIN, 1);
                    htmltext = Some(page.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        let Some(&(_, first_chance, second_chance)) = MOBS.iter().find(|(id, ..)| *id == npc.id) else {
            return;
        };
        let random = ctx.get_random(1000);
        if random < first_chance {
            ctx.give_item_randomly(qs.player(), Some(npc), ANCIENT_ASH_URN, 1, 1, 0, 1.0, true);
        } else if random < second_chance {
            ctx.give_item_randomly(qs.player(), Some(npc), ANCIENT_PORCELAIN, 1, 1, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            // NOTE(java-quirk): Java asks for `30867-03.htm`; the datapack's page is `30867-03.html`.
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30867-01.htm"
            } else {
                "30867-03.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if npc.id == REVA {
                htmltext = if ctx.has_quest_items(player, ANCIENT_PORCELAIN) {
                    "30867-04.html"
                } else {
                    "30867-10.html"
                }
                .to_owned();
            } else {
                htmltext = "30929-01.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00371_ShrieksOfGhosts";

    const NPCS: &str = r#"
        <npc id="30867" type="Folk" level="70" name="Reva"/>
        <npc id="30929" type="Folk" level="70" name="Patrin"/>
        <npc id="20818" type="Monster" level="60" name="Hallates Warrior"/>
        <npc id="20820" type="Monster" level="60" name="Hallates Knight"/>
        <npc id="20824" type="Monster" level="60" name="Hallates Commander"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5903" type="EtcItem" name="Ancient Ash Urn"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6002" type="EtcItem" name="Ancient Porcelain"><set name="is_stackable" val="true"/></item>
        <item id="6003" type="EtcItem" name="Ancient Porcelain (Excellent)"/>
        <item id="6004" type="EtcItem" name="Ancient Porcelain (High Quality)"/>
        <item id="6005" type="EtcItem" name="Ancient Porcelain (Low Quality)"/>
        <item id="6006" type="EtcItem" name="Ancient Porcelain (Lowest Quality)"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00371ShrieksOfGhosts);
        let pages = Pages::of(&Q00371ShrieksOfGhosts)
            .page("30867-01.htm", &["30867-02.htm"])
            .plain(&["30867-02.htm", "30867-08.html", "30867-05.html", "30867-06.html"])
            .page(
                "30867-04.html",
                &["30867-05.html", "30867-08.html", "30867-09.html"],
            )
            .page(
                "30867-10.html",
                &["30867-05.html", "30867-08.html", "30867-09.html"],
            )
            .page("30867-07.html", &["30867-08.html", "30867-09.html"])
            .plain(&["30867-09.html"])
            .page("30929-01.html", &["30929-02.html", "30929-03.html"])
            .page("30929-02.html", &["30929-01.html"])
            .plain(&[
                "30929-03.html",
                "30929-04.html",
                "30929-05.html",
                "30929-06.html",
                "30929-07.html",
                "30929-08.html",
            ])
            .npcs(&[REVA, PATRIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let reva = t.spawn(REVA, 20);
        let patrin = t.spawn(PATRIN, 40);
        (t, reva, patrin)
    }

    fn hunt_urns(t: &mut ScriptTest, urns: i64) {
        for kills in 0..600 {
            if t.count(1, ANCIENT_ASH_URN) >= urns {
                return;
            }
            let mob = t.spawn(MOBS[kills % 3].0, 60);
            t.kill(1, mob);
        }
        panic!("no {urns} urns after 600 kills");
    }

    #[test]
    fn the_urns_are_paid_at_1000_each_plus_15000() {
        let (mut t, reva, _) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, reva).contains("30867-01.htm"));
        assert!(t.event(1, NAME, "30867-02.htm").contains("30867-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, reva).contains("30867-10.html"));

        // Three urns: 3000 + 15000; the quest goes on (06).
        hunt_urns(&mut t, 3);
        let urns = t.count(1, ANCIENT_ASH_URN);
        // A porcelain may have dropped too: Reva's page is 04 then, and 05 is linked from both.
        let page = t.quest_page(1, reva);
        assert!(
            page.contains("30867-10.html") || page.contains("30867-04.html"),
            "{page}"
        );
        assert!(t.event(1, NAME, "30867-05.html").contains("30867-06.html"));
        assert_eq!(t.count(1, 57), urns * 1000 + 15000);
        assert_eq!(t.count(1, ANCIENT_ASH_URN), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn quitting_pays_the_urns_and_ends_the_quest() {
        let (mut t, reva, _) = setup();
        t.enter(1, 59, vec![item(100, ANCIENT_ASH_URN, 2)]);
        t.quest_page(1, reva);
        t.event(1, NAME, "30867-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, reva).contains("30867-10.html"));
        assert!(t.event(1, NAME, "30867-09.html").contains("30867-09.html"));
        assert_eq!(t.count(1, 57), 2000);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn patrin_trades_a_porcelain_for_a_quality_one_or_nothing() {
        let (mut t, reva, patrin) = setup();
        t.enter(1, 59, vec![item(100, ANCIENT_PORCELAIN, 1)]);
        t.quest_page(1, reva);
        t.event(1, NAME, "30867-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, reva).contains("30867-04.html"));
        assert!(t.quest_page(1, patrin).contains("30929-01.html"));
        assert!(t.event(1, NAME, "30929-03.html").contains("30929-0"));
        assert_eq!(t.count(1, ANCIENT_PORCELAIN), 0);
        let qualities: i64 = [6003, 6004, 6005, 6006].iter().map(|id| t.count(1, *id)).sum();
        assert!(qualities <= 1);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hallates_drop_urns_and_porcelains() {
        let (mut t, reva, _) = setup();
        t.enter(1, 59, vec![]);
        t.quest_page(1, reva);
        t.event(1, NAME, "30867-02.htm");
        t.take_writes();
        for kills in 0..400 {
            if t.count(1, ANCIENT_ASH_URN) >= 1 && t.count(1, ANCIENT_PORCELAIN) >= 1 {
                break;
            }
            let mob = t.spawn(MOBS[kills % 2].0, 60);
            t.kill(1, mob);
        }
        assert!(t.count(1, ANCIENT_ASH_URN) >= 1);
        assert!(t.count(1, ANCIENT_PORCELAIN) >= 1);
        assert!(t.take_writes().is_empty());
    }
}
