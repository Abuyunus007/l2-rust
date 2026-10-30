//! Quest 644: Grave Robber Annihilation (Karuda in the Ruins of Batur; the grave robbers drop
//! orc goods, 120 of them buy one of six materials).
// Java: dist/game/data/scripts/quests/Q00644_GraveRobberAnnihilation/Q00644_GraveRobberAnnihilation.java
//
// Pattern: a party kill count (the member at cond 1 counts three times) with a drop limit: the
// `giveItemRandomly` that reaches the limit moves the quest to cond 2, and the reward is one
// material chosen from the page (`REWARDS`), then the quest ends.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KARUDA: i32 = 32017;

// Item
const ORC_GOODS: i32 = 8088;

// Misc
const MIN_LEVEL: i32 = 20;
const ORC_GOODS_REQUIRED_COUNT: i64 = 120;

/// Java `MONSTER_DROP_CHANCES`: the orc goods drop chance of each grave robber.
const MONSTER_DROP_CHANCES: [(i32, f64); 5] = [
    (22003, 0.714), // Grave Robber Scout
    (22004, 0.841), // Grave Robber Lookout
    (22005, 0.778), // Grave Robber Ranger
    (22006, 0.746), // Grave Robber Guard
    (22008, 0.810), // Grave Robber Fighter
];

/// Java `REWARDS`: the event of each material and its item and count.
const REWARDS: [(&str, i32, i64); 6] = [
    ("varnish", 1865, 30),    // Varnish
    ("animalskin", 1867, 40), // Animal Skin
    ("animalbone", 1872, 40), // Animal Bone
    ("charcoal", 1871, 30),   // Charcoal
    ("coal", 1870, 30),       // Coal
    ("ironore", 1869, 30),    // Iron Ore
];

/// Java `Q00644_GraveRobberAnnihilation`.
pub struct Q00644GraveRobberAnnihilation;

impl Script for Q00644GraveRobberAnnihilation {
    fn name(&self) -> &'static str {
        "Q00644_GraveRobberAnnihilation"
    }

    fn path(&self) -> &'static str {
        "quests/Q00644_GraveRobberAnnihilation"
    }

    fn id(&self) -> i32 {
        644
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KARUDA]);
        s.add_talk_id(&[KARUDA]);
        let mobs: Vec<i32> = MONSTER_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[ORC_GOODS]);
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
            "32017-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32017-06.html" => {
                if qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, ORC_GOODS) >= ORC_GOODS_REQUIRED_COUNT
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "varnish" | "animalskin" | "animalbone" | "charcoal" | "coal" | "ironore" => {
                if qs.is_cond(ctx, 2)
                    && let Some(&(_, item, count)) = REWARDS.iter().find(|(name, _, _)| *name == event)
                {
                    ctx.reward_items(player, item, count);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("32017-07.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, 1, 3, npc)`: a member at cond 1.
        let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, chance)) = MONSTER_DROP_CHANCES.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        // The item goes to the killer; the limit (120) is reached when the call says so.
        if ctx.give_item_randomly(
            killer,
            Some(npc),
            ORC_GOODS,
            1,
            1,
            ORC_GOODS_REQUIRED_COUNT,
            chance,
            true,
        ) {
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32017-01.htm".to_owned()
            } else {
                "32017-02.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext = if qs.is_cond(ctx, 2)
                && ctx.get_quest_items_count(player, ORC_GOODS) >= ORC_GOODS_REQUIRED_COUNT
            {
                "32017-04.html".to_owned()
            } else {
                "32017-05.html".to_owned()
            };
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00644_GraveRobberAnnihilation";

    const NPCS: &str = r#"
        <npc id="32017" type="Folk" level="70" name="Karuda"/>
        <npc id="22003" type="Monster" level="50" name="Grave Robber Scout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8088" type="EtcItem" name="Orc Goods"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1865" type="EtcItem" name="Varnish"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00644GraveRobberAnnihilation);
        let pages = Pages::of(&Q00644GraveRobberAnnihilation)
            .page("32017-01.htm", &["32017-03.htm"])
            .plain(&["32017-02.htm", "32017-03.htm", "32017-05.html", "32017-07.html"])
            .page("32017-04.html", &["32017-06.html"])
            .page(
                "32017-06.html",
                &[
                    "varnish",
                    "animalskin",
                    "animalbone",
                    "charcoal",
                    "coal",
                    "ironore",
                ],
            )
            .npcs(&[KARUDA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let karuda = t.spawn(KARUDA, 20);
        (t, karuda)
    }

    /// A grave robber scout dies next to the player and the kill delay passes.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(22003, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_20_is_sent_away() {
        let (mut t, karuda) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, karuda).contains("32017-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_goods_at_120_move_to_cond_2_and_a_material_ends_the_quest() {
        let (mut t, karuda) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, karuda).contains("32017-01.htm"));
        assert!(t.event(1, NAME, "32017-03.htm").contains("32017-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, karuda).contains("32017-05.html"));

        // Goods come from the kills; the one that reaches 120 sets cond 2 (`giveItemRandomly`
        // returns true at the limit).
        let mut kills = 0;
        while t.count(1, ORC_GOODS) < ORC_GOODS_REQUIRED_COUNT {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 400, "no goods dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, karuda).contains("32017-04.html"));
        assert!(t.event(1, NAME, "32017-06.html").contains("32017-06.html"));
        assert!(t.event(1, NAME, "varnish").contains("32017-07.html"));
        assert_eq!(t.count(1, 1865), 30);
        assert_eq!(t.count(1, ORC_GOODS), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
