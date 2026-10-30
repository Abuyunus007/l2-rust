//! Quest 252: It Smells Delicious (Stan, the cook of the Gludin village, wants 10 diaries from the
//! orcs and beasts of the Gludio area and 5 cookbook pages from the chef).
// Java: dist/game/data/scripts/quests/Q00252_ItSmellsDelicious/Q00252_ItSmellsDelicious.java
//
// Pattern: two kill drops with chances and limits, one from a random party member at cond 1 and
// one from the killer only (`docs/design/scripts.md`, "Kills, drops and parties"). The party
// filter is `check_party_member_state`, Java's `checkPartyMember(qs, npc)`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const STAN: i32 = 30200;

// Items
const DIARY: i32 = 15500;
const COOKBOOK_PAGE: i32 = 15501;

// Monsters: the orcs and beasts drop diaries, the chef the cookbook pages
const MOBS: [i32; 3] = [22786, 22787, 22788];
const CHEF: i32 = 18908;

// Misc
const DIARY_CHANCE: f64 = 0.599;
const DIARY_MAX_COUNT: i64 = 10;
const COOKBOOK_PAGE_CHANCE: f64 = 0.36;
const COOKBOOK_PAGE_MAX_COUNT: i64 = 5;

/// Java `hasMaxDiaries(qs)`.
fn has_max_diaries(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.get_quest_items_count(player, DIARY) >= DIARY_MAX_COUNT
}

/// Java `hasMaxCookbookPages(qs)`.
fn has_max_cookbook_pages(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.get_quest_items_count(player, COOKBOOK_PAGE) >= COOKBOOK_PAGE_MAX_COUNT
}

/// Java `Q00252_ItSmellsDelicious`.
pub struct Q00252ItSmellsDelicious;

impl Script for Q00252ItSmellsDelicious {
    fn name(&self) -> &'static str {
        "Q00252_ItSmellsDelicious"
    }

    fn path(&self) -> &'static str {
        "quests/Q00252_ItSmellsDelicious"
    }

    fn id(&self) -> i32 {
        252
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STAN]);
        s.add_talk_id(&[STAN]);
        s.add_kill_id(&[CHEF]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[DIARY, COOKBOOK_PAGE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30200-04.htm" => Some(event.to_owned()),
            "30200-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30200-08.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_adena(player, 147656, true);
                    ctx.add_exp_and_sp(player, 716238, 78324);
                    qs.exit_quest(ctx, false, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Only the killer gets the chef's pages.
        if npc.id == CHEF {
            if let Some(qs) = ctx.get_quest_state(killer, false)
                && qs.is_cond(ctx, 1)
                && ctx.give_item_randomly(
                    killer,
                    Some(npc),
                    COOKBOOK_PAGE,
                    1,
                    1,
                    COOKBOOK_PAGE_MAX_COUNT,
                    COOKBOOK_PAGE_CHANCE,
                    true,
                )
                // NOTE(java-quirk): Java checks the diaries here, not the cookbook pages.
                && has_max_diaries(ctx, killer)
            {
                qs.set_cond(ctx, 2, true);
            }
        } else if let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) {
            let member = qs.player();
            if ctx.give_item_randomly(
                member,
                Some(npc),
                DIARY,
                1,
                1,
                DIARY_MAX_COUNT,
                DIARY_CHANCE,
                true,
            ) && has_max_cookbook_pages(ctx, member)
            {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    /// Java `checkPartyMember(qs, npc)`: a member still short of diaries can get one.
    fn check_party_member_state(&self, ctx: &mut ScriptCtx<'_>, qs: Qs, _npc: Option<ScriptNpc>) -> bool {
        !has_max_diaries(ctx, qs.player())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            htmltext = if ctx.player_level(player) >= 82 {
                "30200-01.htm"
            } else {
                "30200-02.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if qs.is_cond(ctx, 1) {
                htmltext = "30200-06.html".to_owned();
            } else if qs.is_cond(ctx, 2)
                && has_max_diaries(ctx, player)
                && has_max_cookbook_pages(ctx, player)
            {
                htmltext = "30200-07.html".to_owned();
            }
        } else {
            htmltext = "30200-03.html".to_owned();
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

    const NAME: &str = "Q00252_ItSmellsDelicious";

    const NPCS: &str = r#"
        <npc id="30200" type="Folk" level="70" name="Stan"/>
        <npc id="22786" type="Monster" level="60" name="Orc"/>
        <npc id="18908" type="Monster" level="60" name="Chef"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15500" type="EtcItem" name="Diary"><set name="is_questitem" val="true"/></item>
        <item id="15501" type="EtcItem" name="Cookbook Page"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00252ItSmellsDelicious);
        let pages = Pages::of(&Q00252ItSmellsDelicious)
            .page("30200-01.htm", &["30200-04.htm"])
            .page("30200-04.htm", &["30200-05.htm"])
            .plain(&[
                "30200-02.htm",
                "30200-03.html",
                "30200-05.htm",
                "30200-06.html",
                "30200-08.html",
            ])
            .page("30200-07.html", &["30200-08.html"])
            .npcs(&[STAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let stan = t.spawn(STAN, 20);
        (t, stan)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, stan) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, stan).contains("30200-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_from_stan_to_the_reward() {
        let (mut t, stan) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, stan).contains("30200-01.htm"));
        assert!(t.event(1, NAME, "30200-04.htm").contains("30200-04.htm"));
        assert!(t.event(1, NAME, "30200-05.htm").contains("30200-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, stan).contains("30200-06.html"));

        // Ten diaries from the monsters; a diary sets nothing until the pages are in.
        for _ in 0..400 {
            if t.count(1, DIARY) >= DIARY_MAX_COUNT {
                break;
            }
            let orc = t.spawn(22786, 20);
            t.kill(1, orc);
        }
        assert_eq!(t.count(1, DIARY), DIARY_MAX_COUNT);
        assert!(t.take_writes().is_empty());

        // The chef's pages; the fifth page sets cond 2.
        let mut writes = Vec::new();
        for _ in 0..400 {
            if t.count(1, COOKBOOK_PAGE) >= COOKBOOK_PAGE_MAX_COUNT {
                break;
            }
            let chef = t.spawn(CHEF, 20);
            t.kill(1, chef);
            writes.extend(t.take_writes());
        }
        assert_eq!(t.count(1, COOKBOOK_PAGE), COOKBOOK_PAGE_MAX_COUNT);
        assert_eq!(writes, [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, stan).contains("30200-07.html"));

        // Stan pays and the quest ends; the items go with it.
        assert!(t.event(1, NAME, "30200-08.html").contains("30200-08.html"));
        assert_eq!(t.count(1, 57), 147656);
        assert_eq!((t.count(1, DIARY), t.count(1, COOKBOOK_PAGE)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, stan).contains("30200-03.html"));
    }

    #[test]
    fn a_party_member_with_all_diaries_is_not_drawn() {
        let (mut t, stan) = setup();
        t.enter(1, 82, vec![]);
        t.enter(2, 82, vec![item(10, DIARY, 10)]);
        // Player 2 takes the quest with the ten diaries already in the bag.
        t.quest_page(2, stan);
        t.event(2, NAME, "30200-04.htm");
        t.event(2, NAME, "30200-05.htm");
        t.party(1, 2);
        t.take_writes();

        // Player 1 has no state, and player 2 has enough diaries: nobody is drawn.
        let orc = t.spawn(22786, 20);
        t.kill(1, orc);
        assert_eq!((t.count(1, DIARY), t.count(2, DIARY)), (0, DIARY_MAX_COUNT));
        assert!(t.take_writes().is_empty());
    }
}
