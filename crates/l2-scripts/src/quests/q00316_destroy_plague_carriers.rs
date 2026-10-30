//! Quest 316: Destroy Plague Carriers (Ellenia in the Elven village, Sukar Wererats and Varool).
// Java: dist/game/data/scripts/quests/Q00316_DestroyPlagueCarriers/Q00316_DestroyPlagueCarriers.java
//
// Pattern: a kill-count quest over a party (`docs/design/scripts.md`, "Quest patterns"): the
// fangs drop with `giveItemRandomly` to a random member started on the quest (`-1`, killer three
// times as likely), Varool's fang only once. The NPC pays for all fangs at once, with a bonus at
// ten fangs. Varool says a line the first time it is hit (`onAttack` + script value).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::race;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ELLENIA: i32 = 30155;

// Items
const WERERAT_FANG: i32 = 1042;
const VAROOL_FOULCLAW_FANG: i32 = 1043;

// Misc
const MIN_LEVEL: i32 = 18;

// Monsters
const VAROOL_FOULCLAW: i32 = 27020;
/// (monster, the fang it drops, the holder's count). The drop chance is `10.0 / count` as a
/// fraction, as Java computes it: 2.0 for the wererat fangs (always a drop), 1.43 for Varool's.
const MONSTER_DROPS: [(i32, i32, i32); 3] = [
    (20040, WERERAT_FANG, 5),                   // Sukar Wererat
    (20047, WERERAT_FANG, 5),                   // Sukar Wererat Leader
    (VAROOL_FOULCLAW, VAROOL_FOULCLAW_FANG, 7), // Varool Foulclaw
];

/// Java `Q00316_DestroyPlagueCarriers`.
pub struct Q00316DestroyPlagueCarriers;

impl Script for Q00316DestroyPlagueCarriers {
    fn name(&self) -> &'static str {
        "Q00316_DestroyPlagueCarriers"
    }

    fn path(&self) -> &'static str {
        "quests/Q00316_DestroyPlagueCarriers"
    }

    fn id(&self) -> i32 {
        316
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ELLENIA]);
        s.add_talk_id(&[ELLENIA]);
        s.add_attack_id(&[VAROOL_FOULCLAW]);
        let monsters: Vec<i32> = MONSTER_DROPS.iter().map(|(id, _, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[WERERAT_FANG, VAROOL_FOULCLAW_FANG]);
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
            "30155-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30155-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30155-09.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        if ctx.is_script_value(npc, 0) {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::WhyDoYouOppressUsSo, &[]);
            ctx.set_script_value(npc, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): the `checkPartyMember(Player, Npc)` override of Java (Varool's fang
        // only for members without it) is never called on this path: `getRandomPartyMemberState`
        // asks `checkPartyMember(QuestState, Npc)`, which this quest does not override. The
        // limit of 1 on Varool's fang is what keeps it to one.
        if let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) {
            let drop = MONSTER_DROPS.iter().find(|(id, _, _)| *id == npc.id).copied();
            if let Some((_, item, count)) = drop {
                let limit = if npc.id == VAROOL_FOULCLAW { 1 } else { 0 };
                ctx.give_item_randomly(
                    qs.player(),
                    Some(npc),
                    item,
                    1,
                    1,
                    limit,
                    10.0 / f64::from(count),
                    true,
                );
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_race(player) != race::ELF {
                "30155-00.htm"
            } else if ctx.player_level(player) < MIN_LEVEL {
                "30155-02.htm"
            } else {
                "30155-03.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let fangs = [WERERAT_FANG, VAROOL_FOULCLAW_FANG];
            if ctx.has_at_least_one_quest_item(player, &fangs) {
                let wererats = ctx.get_quest_items_count(player, WERERAT_FANG);
                let foulclaws = ctx.get_quest_items_count(player, VAROOL_FOULCLAW_FANG);
                let bonus = if wererats + foulclaws >= 10 { 5000 } else { 0 };
                ctx.give_adena(player, wererats * 30 + foulclaws * 10000 + bonus, true);
                ctx.take_items_many(player, -1, &fangs);
                htmltext = "30155-07.html".to_owned();
            } else {
                htmltext = "30155-05.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00316_DestroyPlagueCarriers";

    const NPCS: &str = r#"
        <npc id="30155" type="Folk" level="1" name="Ellenia"/>
        <npc id="20040" type="Monster" level="15" name="Sukar Wererat"/>
        <npc id="20047" type="Monster" level="16" name="Sukar Wererat Leader"/>
        <npc id="27020" type="RaidBoss" level="20" name="Varool Foulclaw"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1042" type="EtcItem" name="Wererat Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1043" type="EtcItem" name="Varool Foulclaw's Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00316DestroyPlagueCarriers);
        let pages = Pages::of(&Q00316DestroyPlagueCarriers)
            .page("30155-03.htm", &["30155-04.htm"])
            .plain(&["30155-00.htm", "30155-02.htm", "30155-04.htm", "30155-09.html"])
            .page("30155-05.html", &["30155-09.html", "30155-08.html"])
            .page("30155-07.html", &["30155-08.html", "30155-09.html"])
            .plain(&["30155-08.html"])
            .npcs(&[ELLENIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ellenia = t.spawn(ELLENIA, 20);
        (t, ellenia)
    }

    fn wererat(t: &mut ScriptTest) {
        let mob = t.spawn(20040, 60);
        t.kill(1, mob);
    }

    #[test]
    fn only_elves_of_level_eighteen_start_it() {
        let (mut t, ellenia) = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, ellenia).contains("30155-00.htm"));
        t.enter_as(2, 17, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, ellenia).contains("30155-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_and_the_fangs_then_the_reward_with_the_bonus() {
        let (mut t, ellenia) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, ellenia).contains("30155-03.htm"));
        assert!(t.event(1, NAME, "30155-04.htm").contains("30155-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ellenia).contains("30155-05.html"));

        // Wererats drop a fang every time (the chance is 200%): nine of them, then Varool's
        // fang (one only: the limit).
        for _ in 0..9 {
            wererat(&mut t);
        }
        assert_eq!(t.count(1, WERERAT_FANG), 9);
        let varool = t.spawn(VAROOL_FOULCLAW, 60);
        t.kill(1, varool);
        t.kill(1, varool);
        assert_eq!(
            (t.count(1, WERERAT_FANG), t.count(1, VAROOL_FOULCLAW_FANG)),
            (9, 1)
        );

        // 9 * 30 + 1 * 10000 + 5000 for ten fangs in all.
        t.take();
        t.talk(1, ellenia);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30155-07.html"));
        assert_eq!(t.count(1, 57), 15_270);
        assert_eq!(
            (t.count(1, WERERAT_FANG), t.count(1, VAROOL_FOULCLAW_FANG)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());

        // The end: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 30155-08.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30155-08.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn fewer_than_ten_fangs_get_no_bonus() {
        let (mut t, ellenia) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, ellenia);
        t.event(1, NAME, "30155-04.htm");
        t.take_writes();
        // Nothing to hand in yet.
        assert!(t.quest_page(1, ellenia).contains("30155-05.html"));
        wererat(&mut t);
        wererat(&mut t);
        // Two fangs: 2 * 30 adena, no bonus (below ten), the fangs go.
        assert!(t.quest_page(1, ellenia).contains("30155-07.html"));
        assert_eq!(t.count(1, 57), 60);
        assert_eq!(t.count(1, WERERAT_FANG), 0);
    }

    #[test]
    fn varool_says_its_line_once_when_it_is_hit() {
        let (mut t, _ellenia) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        let varool = t.spawn(VAROOL_FOULCLAW, 60);
        let npc = ScriptNpc {
            object_id: varool,
            id: VAROOL_FOULCLAW,
        };
        t.world.run_as_script(NAME, |ctx| {
            Q00316DestroyPlagueCarriers.on_attack(ctx, npc, Oid(1), 10, false, None);
        });
        assert!(
            t.take()
                .iter()
                .any(|(_, b)| crate::testing::name_of(b) == "NPC_SAY")
        );
        t.world.run_as_script(NAME, |ctx| {
            Q00316DestroyPlagueCarriers.on_attack(ctx, npc, Oid(1), 10, false, None);
        });
        assert!(t.take().is_empty());
    }
}
