//! Quest 661: Making the Harvest Grounds Safe (Warehouse Keeper Norman, the Fellmere Harvesting Grounds monsters).
// Java: dist/game/data/scripts/quests/Q00661_MakingTheHarvestGroundsSafe/Q00661_MakingTheHarvestGroundsSafe.java
//
// Pattern: a repeatable kill-and-collect quest with one drop per monster (`MONSTER_CHANCES`), a
// party member drawn by `get_random_party_member_state` and paid per item, with a bonus at ten
// items. Java's varargs `hasQuestItems(player, a, b, c)` means "all of them".

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NORMAN: i32 = 30210;

// Items
const BIG_HORNET_STING: i32 = 8283;
const CLOUD_GEM: i32 = 8284;
const YOUNG_ARANEID_CLAW: i32 = 8285;

// Misc
const MIN_LEVEL: i32 = 21;

/// Java `MONSTER_CHANCES`: the monster, the item it drops and the drop chance.
const MONSTER_CHANCES: [(i32, i32, f64); 3] = [
    (21095, BIG_HORNET_STING, 0.508),   // Giant Poison Bee
    (21096, CLOUD_GEM, 0.5),            // Cloudy Beast
    (21097, YOUNG_ARANEID_CLAW, 0.516), // Young Araneid
];

/// Java `Q00661_MakingTheHarvestGroundsSafe`.
pub struct Q00661MakingTheHarvestGroundsSafe;

impl Script for Q00661MakingTheHarvestGroundsSafe {
    fn name(&self) -> &'static str {
        "Q00661_MakingTheHarvestGroundsSafe"
    }

    fn path(&self) -> &'static str {
        "quests/Q00661_MakingTheHarvestGroundsSafe"
    }

    fn id(&self) -> i32 {
        661
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NORMAN]);
        s.add_talk_id(&[NORMAN]);
        s.add_kill_id(&MONSTER_CHANCES.map(|(id, _, _)| id));
        s.register_quest_items(&[BIG_HORNET_STING, CLOUD_GEM, YOUNG_ARANEID_CLAW]);
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
            "30210-01.htm" | "30210-02.htm" | "30210-04.html" | "30210-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "30210-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30210-08.html" => {
                let sting_count = ctx.get_quest_items_count(player, BIG_HORNET_STING);
                let gem_count = ctx.get_quest_items_count(player, CLOUD_GEM);
                let claw_count = ctx.get_quest_items_count(player, YOUNG_ARANEID_CLAW);
                let mut reward = 57 * sting_count + 56 * gem_count + 60 * claw_count;
                if sting_count + gem_count + claw_count >= 10 {
                    reward += 5773;
                }

                ctx.take_items(player, BIG_HORNET_STING, -1);
                ctx.take_items(player, CLOUD_GEM, -1);
                ctx.take_items(player, YOUNG_ARANEID_CLAW, -1);
                ctx.give_adena(player, reward, true);
                htmltext = Some(event.to_owned());
            }
            "30210-09.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30210-01.htm"
                } else {
                    "30210-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext =
                    if ctx.has_all_quest_items(player, &[BIG_HORNET_STING, CLOUD_GEM, YOUNG_ARANEID_CLAW]) {
                        "30210-04.html"
                    } else {
                        "30210-05.html"
                    }
                    .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if let Some((_, item, chance)) = MONSTER_CHANCES.iter().find(|(id, _, _)| *id == npc.id) {
            // `ItemChanceHolder` with its default count of 1, no limit.
            ctx.give_item_randomly(qs.player(), Some(npc), *item, 1, 1, 0, *chance, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00661_MakingTheHarvestGroundsSafe";

    const NPCS: &str = r#"
        <npc id="30210" type="Folk" level="1" name="Norman"/>
        <npc id="21095" type="Monster" level="20" name="Giant Poison Bee"/>
        <npc id="21096" type="Monster" level="20" name="Cloudy Beast"/>
        <npc id="21097" type="Monster" level="20" name="Young Araneid"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8283" type="EtcItem" name="Big Hornet's Sting"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8284" type="EtcItem" name="Cloud Gem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8285" type="EtcItem" name="Young Araneid Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00661MakingTheHarvestGroundsSafe);
        let pages = Pages::of(&Q00661MakingTheHarvestGroundsSafe)
            .page("30210-01.htm", &["30210-03.htm"])
            .plain(&["30210-02.htm", "30210-03.htm", "30210-05.html", "30210-06.html"])
            .page("30210-04.html", &["30210-08.html"])
            .page("30210-08.html", &["30210-06.html", "30210-09.html"])
            .plain(&["30210-09.html"])
            .npcs(&[NORMAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let norman = t.spawn(NORMAN, 20);
        (t, norman)
    }

    #[test]
    fn a_level_20_character_is_told_to_come_back() {
        let (mut t, norman) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, norman).contains("30210-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_21_character_starts_the_quest() {
        let (mut t, norman) = setup();
        t.enter(1, 21, vec![]);
        assert!(t.quest_page(1, norman).contains("30210-01.htm"));
        assert!(t.event(1, NAME, "30210-03.htm").contains("30210-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_monsters_drop_their_items_and_the_reward_has_the_bonus() {
        let (mut t, norman) = setup();
        t.enter(1, 25, vec![]);
        t.quest_page(1, norman);
        t.event(1, NAME, "30210-03.htm");
        t.take_writes();

        // Until the player holds all three kinds and ten items (the bonus).
        let mut kills = 0;
        loop {
            let kinds = [BIG_HORNET_STING, CLOUD_GEM, YOUNG_ARANEID_CLAW]
                .iter()
                .all(|item| t.count(1, *item) > 0);
            let total: i64 = [BIG_HORNET_STING, CLOUD_GEM, YOUNG_ARANEID_CLAW]
                .iter()
                .map(|item| t.count(1, *item))
                .sum();
            if kinds && total >= 10 {
                break;
            }
            let monster = [21095, 21096, 21097][kills % 3];
            let mob = t.spawn(monster, 60);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 300, "the monsters never dropped the items");
        }
        assert!(t.take_writes().is_empty());

        // The proof page (04) links the reward; the adena is the sum of the items and the bonus.
        assert!(t.quest_page(1, norman).contains("30210-04.html"));
        let (sting, gem, claw) = (
            t.count(1, BIG_HORNET_STING),
            t.count(1, CLOUD_GEM),
            t.count(1, YOUNG_ARANEID_CLAW),
        );
        let expected = 57 * sting + 56 * gem + 60 * claw + 5773;
        assert!(t.event(1, NAME, "30210-08.html").contains("30210-08.html"));
        assert_eq!(t.count(1, 57), expected);
        assert_eq!(
            (
                t.count(1, BIG_HORNET_STING),
                t.count(1, CLOUD_GEM),
                t.count(1, YOUNG_ARANEID_CLAW)
            ),
            (0, 0, 0)
        );
        assert!(t.take_writes().is_empty());

        // Quit (09) ends the quest.
        assert!(t.event(1, NAME, "30210-09.html").contains("30210-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
