//! Quest 633: In the Forgotten Village (Mina in the Gludin area; the undead of the Forgotten
//! Village drop the rib bones of a black magus and the zombie livers; 200 ribs buy the adena and
//! the experience and send the character back to cond 1).
// Java: dist/game/data/scripts/quests/Q00633_InTheForgottenVillage/Q00633_InTheForgottenVillage.java
//
// Pattern: two kinds of drop per monster: the ribs are a limited count (cond 2 at 200, only from
// cond 1), the livers have no limit. The reward takes the ribs and sets cond 1 again (with the
// sound), so the quest can be done again while it is still open.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MINA: i32 = 31388;

// Items
const RIB_BONE_OF_A_BLACK_MAGUS: i32 = 7544;
const ZOMBIES_LIVER: i32 = 7545;

// Misc
const MIN_LEVEL: i32 = 65;
const RIB_BONE_REQUIRED_COUNT: i64 = 200;

/// Java `MOBS_DROP_CHANCES`: the item and the drop chance of each undead.
const MOBS_DROP_CHANCES: [(i32, i32, f64); 27] = [
    (21553, ZOMBIES_LIVER, 0.417),
    (21554, ZOMBIES_LIVER, 0.417),
    (21557, RIB_BONE_OF_A_BLACK_MAGUS, 0.394),
    (21558, RIB_BONE_OF_A_BLACK_MAGUS, 0.394),
    (21559, RIB_BONE_OF_A_BLACK_MAGUS, 0.436),
    (21560, RIB_BONE_OF_A_BLACK_MAGUS, 0.430),
    (21561, ZOMBIES_LIVER, 0.538),
    (21563, RIB_BONE_OF_A_BLACK_MAGUS, 0.436),
    (21564, RIB_BONE_OF_A_BLACK_MAGUS, 0.414),
    (21565, RIB_BONE_OF_A_BLACK_MAGUS, 0.420),
    (21566, RIB_BONE_OF_A_BLACK_MAGUS, 0.460),
    (21567, RIB_BONE_OF_A_BLACK_MAGUS, 0.549),
    (21570, ZOMBIES_LIVER, 0.508),
    (21572, RIB_BONE_OF_A_BLACK_MAGUS, 0.465),
    (21574, RIB_BONE_OF_A_BLACK_MAGUS, 0.586),
    (21575, RIB_BONE_OF_A_BLACK_MAGUS, 0.329),
    (21578, ZOMBIES_LIVER, 0.649),
    (21580, RIB_BONE_OF_A_BLACK_MAGUS, 0.462),
    (21581, RIB_BONE_OF_A_BLACK_MAGUS, 0.505),
    (21583, RIB_BONE_OF_A_BLACK_MAGUS, 0.475),
    (21584, RIB_BONE_OF_A_BLACK_MAGUS, 0.475),
    (21596, RIB_BONE_OF_A_BLACK_MAGUS, 0.543),
    (21597, ZOMBIES_LIVER, 0.510),
    (21598, ZOMBIES_LIVER, 0.572),
    (21599, RIB_BONE_OF_A_BLACK_MAGUS, 0.580),
    (21600, ZOMBIES_LIVER, 0.561),
    (21601, RIB_BONE_OF_A_BLACK_MAGUS, 0.677),
];

/// Java `Q00633_InTheForgottenVillage`.
pub struct Q00633InTheForgottenVillage;

impl Script for Q00633InTheForgottenVillage {
    fn name(&self) -> &'static str {
        "Q00633_InTheForgottenVillage"
    }

    fn path(&self) -> &'static str {
        "quests/Q00633_InTheForgottenVillage"
    }

    fn id(&self) -> i32 {
        633
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MINA]);
        s.add_talk_id(&[MINA]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|(id, _, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[RIB_BONE_OF_A_BLACK_MAGUS, ZOMBIES_LIVER]);
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
            "31388-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31388-04.html" | "31388-05.html" | "31388-06.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31388-07.html" => {
                if qs.is_cond(ctx, 2) {
                    if ctx.get_quest_items_count(player, RIB_BONE_OF_A_BLACK_MAGUS) >= RIB_BONE_REQUIRED_COUNT
                    {
                        ctx.give_adena(player, 25_000, true);
                        ctx.add_exp_and_sp(player, 305_235, 0);
                        ctx.take_items(player, RIB_BONE_OF_A_BLACK_MAGUS, -1);
                        qs.set_cond(ctx, 1, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31388-08.html".to_owned());
                    }
                }
            }
            "31388-09.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 3, npc)`.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, item, chance)) = MOBS_DROP_CHANCES.iter().find(|(id, _, _)| *id == npc.id) else {
            return;
        };
        let member = qs.player();
        match item {
            RIB_BONE_OF_A_BLACK_MAGUS => {
                if qs.is_cond(ctx, 1)
                    && ctx.give_item_randomly(
                        member,
                        Some(npc),
                        item,
                        1,
                        1,
                        RIB_BONE_REQUIRED_COUNT,
                        chance,
                        true,
                    )
                {
                    // Java `setCond(2)`: no sound.
                    qs.set_cond(ctx, 2, false);
                }
            }
            ZOMBIES_LIVER => {
                ctx.give_item_randomly(member, Some(npc), item, 1, 1, 0, chance, true);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31388-01.htm".to_owned()
            } else {
                "31388-02.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext =
                if ctx.get_quest_items_count(player, RIB_BONE_OF_A_BLACK_MAGUS) >= RIB_BONE_REQUIRED_COUNT {
                    "31388-04.html".to_owned()
                } else {
                    "31388-05.html".to_owned()
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

    const NAME: &str = "Q00633_InTheForgottenVillage";

    const NPCS: &str = r#"
        <npc id="31388" type="Folk" level="70" name="Mina"/>
        <npc id="21553" type="Monster" level="60" name="Trampled Man"/>
        <npc id="21557" type="Monster" level="60" name="Bone Snatcher"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7544" type="EtcItem" name="Rib Bone of a Black Magus"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7545" type="EtcItem" name="Zombie's Liver"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00633InTheForgottenVillage);
        let pages = Pages::of(&Q00633InTheForgottenVillage)
            .page("31388-01.htm", &["31388-03.htm"])
            .plain(&[
                "31388-02.htm",
                "31388-03.htm",
                "31388-06.html",
                "31388-08.html",
                "31388-09.html",
            ])
            .page("31388-04.html", &["31388-07.html", "31388-09.html"])
            .page("31388-05.html", &["31388-06.html", "31388-09.html"])
            .page("31388-07.html", &["31388-06.html", "31388-09.html"])
            .npcs(&[MINA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mina = t.spawn(MINA, 20);
        (t, mina)
    }

    #[test]
    fn a_character_below_level_65_is_sent_away() {
        let (mut t, mina) = setup();
        t.enter(1, 64, vec![]);
        assert!(t.quest_page(1, mina).contains("31388-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_livers_come_without_a_limit_and_200_ribs_pay_and_return_to_cond_1() {
        let (mut t, mina) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, mina).contains("31388-01.htm"));
        assert!(t.event(1, NAME, "31388-03.htm").contains("31388-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The livers have no limit (a trampled man drops them 41.7 % of the time).
        let mut kills = 0;
        while t.count(1, ZOMBIES_LIVER) == 0 {
            let mob = t.spawn(21553, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no liver dropped");
        }
        assert!(t.take_writes().is_empty());

        // 199 ribs by the script; the next rib (limit 200) sets cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RIB_BONE_OF_A_BLACK_MAGUS, 199);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, RIB_BONE_OF_A_BLACK_MAGUS) < RIB_BONE_REQUIRED_COUNT {
            let mob = t.spawn(21557, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no rib dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: adena, exp, the ribs gone, and cond 1 again (with the sound).
        assert!(t.quest_page(1, mina).contains("31388-04.html"));
        assert!(t.event(1, NAME, "31388-07.html").contains("31388-07.html"));
        assert_eq!(t.count(1, 57), 25_000);
        assert_eq!(t.count(1, RIB_BONE_OF_A_BLACK_MAGUS), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "1")]);

        // The livers are left and the quest ends from the page.
        assert!(t.event(1, NAME, "31388-09.html").contains("31388-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_reward_refuses_with_fewer_than_200_ribs() {
        let (mut t, mina) = setup();
        t.enter(1, 65, vec![]);
        t.quest_page(1, mina);
        t.event(1, NAME, "31388-03.htm");
        // Cond 2 with 200 ribs: the page offers the reward; one rib goes before the click.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RIB_BONE_OF_A_BLACK_MAGUS, 200);
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        t.take();
        t.take_writes();
        assert!(t.quest_page(1, mina).contains("31388-04.html"));
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), RIB_BONE_OF_A_BLACK_MAGUS, 1);
        });
        t.take();
        assert!(t.event(1, NAME, "31388-07.html").contains("31388-08.html"));
        assert_eq!(t.count(1, RIB_BONE_OF_A_BLACK_MAGUS), 199);
        assert!(t.take_writes().is_empty());
    }
}
