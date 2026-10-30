//! Quest 401: Path of the Warrior (Master Auron and Trader Simplon).
// Java: dist/game/data/scripts/quests/Q00401_PathOfTheWarrior/Q00401_PathOfTheWarrior.java
//
// Pattern: a kill chain with the monsters' state kept on the NPC (`onAttack` sets the NPC's
// script value to 1 when the attacker holds the rusted bronze sword 3 and keeps hitting it, see
// `docs/design/scripts.md`, "Quest patterns"): tracker skeletons drop the swords, the spiders
// the legs, and only a spider hit with the sword counts.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const MASTER_AURON: i32 = 30010;
const TRADER_SIMPLON: i32 = 30253;

// Items
const AURONS_LETTER: i32 = 1138;
const WARRIOR_GUILD_MARK: i32 = 1139;
const RUSTED_BRONZE_SWORD1: i32 = 1140;
const RUSTED_BRONZE_SWORD2: i32 = 1141;
const RUSTED_BRONZE_SWORD3: i32 = 1142;
const SIMPLONS_LETTER: i32 = 1143;
const VENOMOUS_SPIDERS_LEG: i32 = 1144;

// Reward
const MEDALLION_OF_WARRIOR: i32 = 1145;

// Monsters
const TRACKER_SKELETON: i32 = 20035;
const VENOMOUS_SPIDERS: i32 = 20038;
const TRACKER_SKELETON_LIDER: i32 = 20042;
const ARACHNID_TRACKER: i32 = 20043;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00401_PathOfTheWarrior`.
pub struct Q00401PathOfTheWarrior;

impl Script for Q00401PathOfTheWarrior {
    fn name(&self) -> &'static str {
        "Q00401_PathOfTheWarrior"
    }

    fn path(&self) -> &'static str {
        "quests/Q00401_PathOfTheWarrior"
    }

    fn id(&self) -> i32 {
        401
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_AURON]);
        s.add_talk_id(&[MASTER_AURON, TRADER_SIMPLON]);
        s.add_attack_id(&[VENOMOUS_SPIDERS, ARACHNID_TRACKER]);
        s.add_kill_id(&[
            TRACKER_SKELETON,
            VENOMOUS_SPIDERS,
            TRACKER_SKELETON_LIDER,
            ARACHNID_TRACKER,
        ]);
        s.register_quest_items(&[
            AURONS_LETTER,
            WARRIOR_GUILD_MARK,
            RUSTED_BRONZE_SWORD1,
            RUSTED_BRONZE_SWORD2,
            RUSTED_BRONZE_SWORD3,
            SIMPLONS_LETTER,
            VENOMOUS_SPIDERS_LEG,
        ]);
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
            "ACCEPT" => {
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::FIGHTER {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, MEDALLION_OF_WARRIOR) {
                                "30010-04.htm"
                            } else {
                                "30010-05.htm"
                            }
                        } else {
                            "30010-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::WARRIOR {
                        "30010-02a.htm"
                    } else {
                        "30010-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30010-06.htm" => {
                if !ctx.has_quest_items(player, AURONS_LETTER) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, AURONS_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30010-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "30010-11.html" => {
                if ctx.has_all_quest_items(player, &[SIMPLONS_LETTER, RUSTED_BRONZE_SWORD2]) {
                    ctx.take_items(player, RUSTED_BRONZE_SWORD2, 1);
                    ctx.give_items(player, RUSTED_BRONZE_SWORD3, 1);
                    ctx.take_items(player, SIMPLONS_LETTER, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30253-02.html" => {
                if ctx.has_quest_items(player, AURONS_LETTER) {
                    ctx.take_items(player, AURONS_LETTER, 1);
                    ctx.give_items(player, WARRIOR_GUILD_MARK, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match ctx.get_script_value(npc) {
            0 => {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("lastAttacker", attacker.0);
                if !check_weapon(ctx, attacker) {
                    ctx.set_script_value(npc, 2);
                } else {
                    ctx.set_script_value(npc, 1);
                }
            }
            1 => {
                if !check_weapon(ctx, attacker) {
                    ctx.set_script_value(npc, 2);
                } else {
                    let last_attacker = ctx
                        .world()
                        .npc_variables(npc.object_id)
                        .map_or(0, |v| v.get_int("lastAttacker", 0));
                    if last_attacker != attacker.0 {
                        ctx.set_script_value(npc, 2);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            TRACKER_SKELETON | TRACKER_SKELETON_LIDER => {
                if ctx.has_quest_items(killer, WARRIOR_GUILD_MARK)
                    && ctx.get_quest_items_count(killer, RUSTED_BRONZE_SWORD1) < 10
                    && ctx.get_random(10) < 4
                {
                    ctx.give_items(killer, RUSTED_BRONZE_SWORD1, 1);
                    if ctx.get_quest_items_count(killer, RUSTED_BRONZE_SWORD1) == 10 {
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            VENOMOUS_SPIDERS | ARACHNID_TRACKER => {
                if ctx.get_quest_items_count(killer, VENOMOUS_SPIDERS_LEG) < 20 && ctx.is_script_value(npc, 1)
                {
                    ctx.give_items(killer, VENOMOUS_SPIDERS_LEG, 1);
                    if ctx.get_quest_items_count(killer, VENOMOUS_SPIDERS_LEG) == 20 {
                        qs.set_cond(ctx, 6, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == MASTER_AURON {
                htmltext = "30010-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                MASTER_AURON => {
                    if ctx.has_quest_items(player, AURONS_LETTER) {
                        htmltext = "30010-07.html".to_owned();
                    } else if ctx.has_quest_items(player, WARRIOR_GUILD_MARK) {
                        htmltext = "30010-08.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[SIMPLONS_LETTER, RUSTED_BRONZE_SWORD2])
                        && !ctx.has_at_least_one_quest_item(player, &[WARRIOR_GUILD_MARK, AURONS_LETTER])
                    {
                        htmltext = "30010-09.html".to_owned();
                    } else if ctx.has_quest_items(player, RUSTED_BRONZE_SWORD3)
                        && !ctx.has_at_least_one_quest_item(player, &[WARRIOR_GUILD_MARK, AURONS_LETTER])
                    {
                        if ctx.get_quest_items_count(player, VENOMOUS_SPIDERS_LEG) < 20 {
                            htmltext = "30010-12.html".to_owned();
                        } else {
                            ctx.give_adena(player, 163_800, true);
                            ctx.give_items(player, MEDALLION_OF_WARRIOR, 1);
                            let level = ctx.player_level(player);
                            if level >= 20 {
                                ctx.add_exp_and_sp(player, 320_534, 21_012);
                            } else if level == 19 {
                                ctx.add_exp_and_sp(player, 456_128, 27_710);
                            } else {
                                ctx.add_exp_and_sp(player, 160_267, 34_408);
                            }
                            qs.exit_quest(ctx, false, true);
                            ctx.world_mut().send_social_action(player, 3);
                            htmltext = "30010-13.html".to_owned();
                        }
                    }
                }
                TRADER_SIMPLON => {
                    if ctx.has_quest_items(player, AURONS_LETTER) {
                        htmltext = "30253-01.html".to_owned();
                    } else if ctx.has_quest_items(player, WARRIOR_GUILD_MARK) {
                        if !ctx.has_quest_items(player, RUSTED_BRONZE_SWORD1) {
                            htmltext = "30253-03.html".to_owned();
                        } else if ctx.get_quest_items_count(player, RUSTED_BRONZE_SWORD1) < 10 {
                            htmltext = "30253-04.html".to_owned();
                        } else {
                            ctx.take_items(player, WARRIOR_GUILD_MARK, 1);
                            ctx.take_items(player, RUSTED_BRONZE_SWORD1, -1);
                            ctx.give_items(player, RUSTED_BRONZE_SWORD2, 1);
                            ctx.give_items(player, SIMPLONS_LETTER, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30253-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, SIMPLONS_LETTER) {
                        htmltext = "30253-06.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `checkWeapon(player)`: the rusted bronze sword 3 is the weapon in the right hand.
fn check_weapon(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.active_weapon_item_id(player) == Some(RUSTED_BRONZE_SWORD3)
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00401_PathOfTheWarrior";

    const NPCS: &str = r#"
        <npc id="30010" type="Folk" level="70" name="Master Auron"/>
        <npc id="30253" type="Folk" level="70" name="Trader Simplon"/>
        <npc id="20035" type="Monster" level="20" name="Tracker Skeleton"/>
        <npc id="20038" type="Monster" level="20" name="Venomous Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1138" type="EtcItem" name="Auron's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1139" type="EtcItem" name="Warrior Guild Mark"><set name="is_questitem" val="true"/></item>
        <item id="1140" type="Weapon" name="Rusted Bronze Sword 1"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1141" type="Weapon" name="Rusted Bronze Sword 2"><set name="is_questitem" val="true"/></item>
        <item id="1142" type="Weapon" name="Rusted Bronze Sword 3"><set name="is_questitem" val="true"/></item>
        <item id="1143" type="EtcItem" name="Simplon's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1144" type="EtcItem" name="Venomous Spider's Leg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1145" type="EtcItem" name="Medallion of Warrior"/>"#;

    struct Npcs {
        auron: Oid,
        simplon: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00401PathOfTheWarrior)
            .page("30010-01.htm", &["ACCEPT"])
            .page("30010-05.htm", &["30010-06.htm"])
            .page("30010-09.html", &["30010-10.html", "30010-11.html"])
            .page("30010-10.html", &["30010-11.html"])
            .page("30253-01.html", &["30253-02.html"])
            .plain(&[
                "30010-02.htm",
                "30010-02a.htm",
                "30010-03.htm",
                "30010-04.htm",
                "30010-06.htm",
                "30010-07.html",
                "30010-08.html",
                "30010-11.html",
                "30010-12.html",
                "30010-13.html",
                "30253-02.html",
                "30253-03.html",
                "30253-04.html",
                "30253-05.html",
                "30253-06.html",
            ])
            .npcs(&[MASTER_AURON, TRADER_SIMPLON])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00401PathOfTheWarrior);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            auron: t.spawn(MASTER_AURON, 20),
            simplon: t.spawn(TRADER_SIMPLON, 40),
        };
        (t, npcs)
    }

    /// A fighter of level 18 with the rusted bronze sword 3 in the right hand.
    fn enter_with_sword(t: &mut ScriptTest, level: i32) {
        let mut sword = item(5001, RUSTED_BRONZE_SWORD3, 1);
        sword.loc = "PAPERDOLL".to_owned();
        sword.loc_data = 5;
        t.enter_as(1, level, race::HUMAN, player_class::FIGHTER, vec![sword]);
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    /// Hits `npc_id` as the player (`onAttack`), then kills it.
    fn attack_and_kill(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 80);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00401PathOfTheWarrior.on_attack(ctx, npc, Oid(1), 1, false, None);
            }
        });
        t.kill(1, mob);
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_medallion() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.auron).contains("30010-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, npcs.auron);
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.auron);
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.auron);
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-05.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(5000, 1145, 1)],
        );
        t.quest_page(1, npcs.auron);
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_letter_to_the_medallion() {
        let (mut t, npcs) = setup();
        enter_with_sword(&mut t, 18);
        assert!(t.quest_page(1, npcs.auron).contains("30010-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30010-05.htm"));
        assert!(t.event(1, NAME, "30010-06.htm").contains("30010-06.htm"));
        assert_eq!(t.count(1, AURONS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.auron).contains("30010-07.html"));

        // Simplon takes Auron's letter and gives the guild mark: cond 2.
        assert!(t.quest_page(1, npcs.simplon).contains("30253-01.html"));
        assert!(t.event(1, NAME, "30253-02.html").contains("30253-02.html"));
        assert_eq!(
            (t.count(1, AURONS_LETTER), t.count(1, WARRIOR_GUILD_MARK)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.simplon).contains("30253-03.html"));

        // Ten rusted swords from the tracker skeletons (a 40% drop); the tenth sets cond 3.
        hunt(&mut t, TRACKER_SKELETON, RUSTED_BRONZE_SWORD1, 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.simplon).contains("30253-05.html"));

        // Simplon takes the swords and gives the second sword and his letter: cond 4.
        assert_eq!(
            (
                t.count(1, RUSTED_BRONZE_SWORD1),
                t.count(1, RUSTED_BRONZE_SWORD2),
                t.count(1, SIMPLONS_LETTER)
            ),
            (0, 1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.simplon).contains("30253-06.html"));

        // Auron takes the second sword and gives the third: cond 5.
        assert!(t.quest_page(1, npcs.auron).contains("30010-09.html"));
        assert!(t.event(1, NAME, "30010-10.html").contains("30010-10.html"));
        assert!(t.event(1, NAME, "30010-11.html").contains("30010-11.html"));
        // The sword the test character wears from the start is a second sword 3 in the bag.
        assert_eq!(t.count(1, RUSTED_BRONZE_SWORD2), 0);
        assert_eq!(t.count(1, RUSTED_BRONZE_SWORD3), 2);
        assert_eq!(t.count(1, SIMPLONS_LETTER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.auron).contains("30010-12.html"));

        // Twenty venomous spider legs, hit with the third sword; the twentieth sets cond 6.
        hunt_with_hits(&mut t, VENOMOUS_SPIDERS, VENOMOUS_SPIDERS_LEG, 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Auron's reward: adena, the medallion, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.auron).contains("30010-13.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, MEDALLION_OF_WARRIOR), 1);
        assert_eq!(t.count(1, VENOMOUS_SPIDERS_LEG), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Auron shows his first page again.
        assert!(t.quest_page(1, npcs.auron).contains("30010-01.htm"));
    }

    /// Hits and kills `npc_id` until the player has `count` of `item_id`.
    fn hunt_with_hits(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) {
        for _ in 0..2000 {
            if t.count(1, item_id) >= count {
                return;
            }
            attack_and_kill(t, npc_id);
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    #[test]
    fn a_spider_hit_without_the_third_sword_gives_no_leg() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.auron);
        t.event(1, NAME, "ACCEPT");
        t.event(1, NAME, "30010-06.htm");
        t.take_writes();
        for _ in 0..5 {
            attack_and_kill(&mut t, VENOMOUS_SPIDERS);
        }
        assert_eq!(t.count(1, VENOMOUS_SPIDERS_LEG), 0);
        assert!(t.take_writes().is_empty());
    }
}
