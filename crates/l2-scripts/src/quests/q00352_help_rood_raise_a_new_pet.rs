//! Quest 352: Help Rood Raise A New Pet! (Rood in Heine, Lienrik eggs).
// Java: dist/game/data/scripts/quests/Q00352_HelpRoodRaiseANewPet/Q00352_HelpRoodRaiseANewPet.java
//
// Pattern: a kill-drop quest (two eggs with separate chances, no limit) that pays per egg when the
// NPC is talked to; the quest is repeatable (`exitQuest(true, true)`).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ROOD: i32 = 31067;

// Items
const LIENRIK_EGG1: i32 = 5860;
const LIENRIK_EGG2: i32 = 5861;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `DropInfo`: the chances (percent) of the first and of the second egg.
struct DropInfo {
    first_chance: i32,
    second_chance: i32,
}

/// Java `MOBS`: the Lienrik, the Lienrik (A) and the Lienrik Lad (A).
const MOBS: [(i32, DropInfo); 3] = [
    (
        20786,
        DropInfo {
            first_chance: 46,
            second_chance: 48,
        },
    ),
    (
        21644,
        DropInfo {
            first_chance: 46,
            second_chance: 48,
        },
    ),
    (
        21645,
        DropInfo {
            first_chance: 69,
            second_chance: 71,
        },
    ),
];

fn drop_info(npc_id: i32) -> Option<&'static DropInfo> {
    MOBS.iter().find(|(id, _)| *id == npc_id).map(|(_, info)| info)
}

/// Java `Q00352_HelpRoodRaiseANewPet`.
pub struct Q00352HelpRoodRaiseANewPet;

impl Script for Q00352HelpRoodRaiseANewPet {
    fn name(&self) -> &'static str {
        "Q00352_HelpRoodRaiseANewPet"
    }

    fn path(&self) -> &'static str {
        "quests/Q00352_HelpRoodRaiseANewPet"
    }

    fn id(&self) -> i32 {
        352
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROOD]);
        s.add_talk_id(&[ROOD]);
        let ids: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[LIENRIK_EGG1, LIENRIK_EGG2]);
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
            "31067-02.htm" | "31067-03.htm" | "31067-07.html" | "31067-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "31067-04.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31067-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_none() || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        let Some(info) = drop_info(npc.id) else {
            return;
        };
        let random = ctx.get_random(100);
        if random < info.first_chance {
            ctx.give_item_randomly(killer, Some(npc), LIENRIK_EGG1, 1, 1, 0, 1.0, true);
        } else if random < info.second_chance {
            ctx.give_item_randomly(killer, Some(npc), LIENRIK_EGG2, 1, 1, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31067-01.htm"
            } else {
                "31067-05.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let egg1_count = ctx.get_quest_items_count(player, LIENRIK_EGG1);
            let egg2_count = ctx.get_quest_items_count(player, LIENRIK_EGG2);
            if egg1_count == 0 && egg2_count == 0 {
                htmltext = "31067-06.html".to_owned();
            } else if egg1_count >= 1 && egg2_count == 0 {
                if egg1_count >= 10 {
                    ctx.give_adena(player, egg1_count * 34 + 4000, true);
                } else {
                    ctx.give_adena(player, egg1_count * 34 + 2000, true);
                }

                ctx.take_items(player, LIENRIK_EGG1, -1);
                htmltext = "31067-10.html".to_owned();
            } else if egg1_count >= 1 {
                ctx.give_adena(player, 4000 + (egg1_count * 34 + egg2_count * 1025), true);
                ctx.take_items(player, LIENRIK_EGG1, -1);
                ctx.take_items(player, LIENRIK_EGG2, -1);
                htmltext = "31067-11.html".to_owned();
            }
            // NOTE(java-quirk): with only second eggs in the bag none of the branches above
            // matches, so the player gets the "no quest" message and keeps the eggs.
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, rows};

    const NAME: &str = "Q00352_HelpRoodRaiseANewPet";

    const NPCS: &str = r#"
        <npc id="31067" type="Folk" level="50" name="Rood"/>
        <npc id="20786" type="Monster" level="40" name="Lienrik"/>
        <npc id="21644" type="Monster" level="40" name="Lienrik A"/>
        <npc id="21645" type="Monster" level="40" name="Lienrik Lad A"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5860" type="EtcItem" name="Lienrik Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5861" type="EtcItem" name="Lienrik Lad Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00352HelpRoodRaiseANewPet);
        let pages = Pages::of(&Q00352HelpRoodRaiseANewPet)
            .page("31067-01.htm", &["31067-02.htm"])
            .page("31067-02.htm", &["31067-03.htm"])
            .page("31067-03.htm", &["31067-04.htm"])
            .plain(&[
                "31067-04.htm",
                "31067-05.html",
                "31067-07.html",
                "31067-08.html",
                "31067-10.html",
            ])
            .page("31067-06.html", &["31067-07.html", "31067-08.html"])
            .page(
                "31067-11.html",
                &["31067-07.html", "31067-08.html", "31067-10.html"],
            )
            .npcs(&[ROOD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rood = t.spawn(ROOD, 20);
        (t, rood)
    }

    /// Kills Lienrik until the player has `count` of `item_id`.
    fn hunt(t: &mut ScriptTest, item_id: i32, count: i64) {
        for _ in 0..400 {
            if t.count(1, item_id) >= count {
                return;
            }
            let lienrik = t.spawn(20786, 60);
            t.kill(1, lienrik);
        }
        panic!("no {count} of {item_id} after 400 kills");
    }

    fn start(t: &mut ScriptTest, rood: Oid) {
        assert!(t.quest_page(1, rood).contains("31067-01.htm"));
        assert!(t.event(1, NAME, "31067-02.htm").contains("31067-02.htm"));
        assert!(t.event(1, NAME, "31067-03.htm").contains("31067-03.htm"));
        assert!(t.event(1, NAME, "31067-04.htm").contains("31067-04.htm"));
    }

    #[test]
    fn a_character_below_level_thirty_nine_is_sent_away() {
        let (mut t, rood) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, rood).contains("31067-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_writes_the_memo_state_and_the_cond() {
        let (mut t, rood) = setup();
        t.enter(1, 39, vec![]);
        t.quest_page(1, rood);
        t.event(1, NAME, "31067-02.htm");
        t.event(1, NAME, "31067-03.htm");
        t.take_writes();
        assert!(t.event(1, NAME, "31067-04.htm").contains("31067-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, rood).contains("31067-06.html"));
    }

    #[test]
    fn quitting_at_the_start_ends_the_repeatable_quest() {
        let (mut t, rood) = setup();
        t.enter(1, 39, vec![]);
        start(&mut t, rood);
        t.take_writes();
        assert!(t.quest_page(1, rood).contains("31067-06.html"));
        t.bypass(1, &format!("Script {NAME} 31067-08.html"));
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, rood).contains("31067-01.htm"));
    }

    #[test]
    fn an_egg_drops_from_a_lienrik_and_the_lienrik_pays_per_egg() {
        let (mut t, rood) = setup();
        t.enter(1, 39, vec![]);
        start(&mut t, rood);
        t.take_writes();

        // A Lienrik out of range of the killer gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                20786,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, LIENRIK_EGG1), 0);

        hunt(&mut t, LIENRIK_EGG1, 10);
        let eggs = t.count(1, LIENRIK_EGG1);
        assert!(eggs >= 10);
        assert!(t.take_writes().is_empty());

        // Ten or more eggs: 34 each plus 4000.
        assert!(t.quest_page(1, rood).contains("31067-10.html"));
        assert_eq!(t.count(1, 57), 34 * eggs + 4000);
        assert_eq!(t.count(1, LIENRIK_EGG1), 0);
        assert!(t.take_writes().is_empty());
        // The quest goes on: the next visit says there are no eggs.
        assert!(t.quest_page(1, rood).contains("31067-06.html"));
    }

    #[test]
    fn fewer_than_ten_eggs_pay_34_each_plus_2000() {
        let (mut t, rood) = setup();
        t.enter(1, 39, vec![item(100, LIENRIK_EGG1, 3)]);
        start(&mut t, rood);
        assert!(t.quest_page(1, rood).contains("31067-10.html"));
        assert_eq!(t.count(1, 57), 3 * 34 + 2000);
        assert_eq!(t.count(1, LIENRIK_EGG1), 0);
    }

    #[test]
    fn eggs_of_both_kinds_pay_for_the_second_kind_too() {
        let (mut t, rood) = setup();
        t.enter(
            1,
            39,
            vec![item(100, LIENRIK_EGG1, 3), item(101, LIENRIK_EGG2, 2)],
        );
        start(&mut t, rood);
        assert!(t.quest_page(1, rood).contains("31067-11.html"));
        assert_eq!(t.count(1, 57), 4000 + (3 * 34 + 2 * 1025));
        assert_eq!((t.count(1, LIENRIK_EGG1), t.count(1, LIENRIK_EGG2)), (0, 0));
    }

    #[test]
    fn second_eggs_alone_get_the_no_quest_message() {
        let (mut t, rood) = setup();
        t.enter(1, 39, vec![item(100, LIENRIK_EGG2, 1)]);
        start(&mut t, rood);
        let page = t.quest_page(1, rood);
        assert!(!page.contains("31067-1"), "{page}");
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(t.count(1, LIENRIK_EGG2), 1);
    }
}
