//! Quest 403: Path of the Rogue (Captain Bezique and Neti).
// Java: dist/game/data/scripts/quests/Q00403_PathOfTheRogue/Q00403_PathOfTheRogue.java
//
// Pattern: a kill chain with the attack as the gate (`onAttack` sets the NPC's script value to 1
// when the attacker holds the Netis' bow or dagger, see `docs/design/scripts.md`, "Quest
// patterns"): spartoi bones from the undead, the horseshoe from Neti, then the four stolen items
// from the quest monster Cat's Eye Bandit, one random item per kill.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CAPTAIN_BEZIQUE: i32 = 30379;
const NETI: i32 = 30425;

// Items
const BEZIQUES_LETTER: i32 = 1180;
const NETIS_BOW: i32 = 1181;
const NETIS_DAGGER: i32 = 1182;
const SPARTOIS_BONES: i32 = 1183;
const HORSESHOE_OF_LIGHT: i32 = 1184;
const MOST_WANTED_LIST: i32 = 1185;
const STOLEN_JEWELRY: i32 = 1186;
const STOLEN_TOMES: i32 = 1187;
const STOLEN_RING: i32 = 1188;
const STOLEN_NECKLACE: i32 = 1189;
const STOLEN_ITEMS: [i32; 4] = [STOLEN_JEWELRY, STOLEN_TOMES, STOLEN_RING, STOLEN_NECKLACE];

// Reward
const BEZIQUES_RECOMMENDATION: i32 = 1190;

// Misc
const MIN_LEVEL: i32 = 18;
const REQUIRED_ITEM_COUNT: i64 = 10;

// Quest monster
const CATS_EYE_BANDIT: i32 = 27038;

/// Java `MONSTER_DROPS`: (monster, item, chance out of 10 per kill).
const MONSTER_DROPS: &[(i32, i32, i32)] = &[
    (20035, SPARTOIS_BONES, 2), // Tracker Skeleton
    (20042, SPARTOIS_BONES, 3), // Tracker Skeleton Leader
    (20045, SPARTOIS_BONES, 2), // Skeleton Scout
    (20051, SPARTOIS_BONES, 2), // Skeleton Bowman
    (20054, SPARTOIS_BONES, 8), // Ruin Spartoi
    (20060, SPARTOIS_BONES, 8), // Raging Spartoi
];

/// Java `Q00403_PathOfTheRogue`.
pub struct Q00403PathOfTheRogue;

impl Script for Q00403PathOfTheRogue {
    fn name(&self) -> &'static str {
        "Q00403_PathOfTheRogue"
    }

    fn path(&self) -> &'static str {
        "quests/Q00403_PathOfTheRogue"
    }

    fn id(&self) -> i32 {
        403
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CAPTAIN_BEZIQUE]);
        s.add_talk_id(&[CAPTAIN_BEZIQUE, NETI]);
        let mobs: Vec<i32> = MONSTER_DROPS.iter().map(|d| d.0).collect();
        s.add_attack_id(&mobs);
        s.add_attack_id(&[CATS_EYE_BANDIT]);
        s.add_kill_id(&mobs);
        s.add_kill_id(&[CATS_EYE_BANDIT]);
        s.register_quest_items(&[
            BEZIQUES_LETTER,
            NETIS_BOW,
            NETIS_DAGGER,
            SPARTOIS_BONES,
            HORSESHOE_OF_LIGHT,
            MOST_WANTED_LIST,
            STOLEN_JEWELRY,
            STOLEN_TOMES,
            STOLEN_RING,
            STOLEN_NECKLACE,
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
                            if ctx.has_quest_items(player, BEZIQUES_RECOMMENDATION) {
                                "30379-04.htm"
                            } else {
                                "30379-05.htm"
                            }
                        } else {
                            "30379-03.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::ROGUE {
                        "30379-02a.htm"
                    } else {
                        "30379-02.htm"
                    }
                    .to_owned(),
                );
            }
            "30379-06.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, BEZIQUES_LETTER, 1);
                htmltext = Some(event.to_owned());
            }
            "30425-02.html" | "30425-03.html" | "30425-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "30425-05.html" => {
                if ctx.has_quest_items(player, BEZIQUES_LETTER) {
                    ctx.take_items(player, BEZIQUES_LETTER, 1);
                    if !ctx.has_quest_items(player, NETIS_BOW) {
                        ctx.give_items(player, NETIS_BOW, 1);
                    }
                    if !ctx.has_quest_items(player, NETIS_DAGGER) {
                        ctx.give_items(player, NETIS_DAGGER, 1);
                    }
                    qs.set_cond(ctx, 2, true);
                }
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
                    // NOTE(java-quirk): Java also sends the attacker a private NpcSay ("You
                    // childish fool...") from the bandit. FOLLOW-UP(engine): no script helper
                    // sends an NpcSay to one player yet.
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
        if !(qs.is_started(ctx)
            && ctx.is_script_value(npc, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        if npc.id == CATS_EYE_BANDIT {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::IMustDoSomethingAboutThisShamefulIncident,
                &[],
            );
            if ctx.has_quest_items(killer, MOST_WANTED_LIST) {
                if let Some(random_item) = ctx.get_random_entry(&STOLEN_ITEMS)
                    && !ctx.has_quest_items(killer, random_item)
                {
                    ctx.give_items(killer, random_item, 1);
                    if ctx.has_all_quest_items(killer, &STOLEN_ITEMS) {
                        qs.set_cond(ctx, 6, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
        } else if let Some(&(_, item, chance)) = MONSTER_DROPS.iter().find(|d| d.0 == npc.id)
            && ctx.get_quest_items_count(killer, item) < REQUIRED_ITEM_COUNT
            && ctx.is_script_value(npc, 1)
            && i64::from(ctx.get_random(10)) < i64::from(chance)
        {
            ctx.give_items(killer, item, 1);
            if ctx.get_quest_items_count(killer, item) >= REQUIRED_ITEM_COUNT {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == CAPTAIN_BEZIQUE {
                htmltext = "30379-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                CAPTAIN_BEZIQUE => {
                    if ctx.has_all_quest_items(player, &STOLEN_ITEMS) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.take_items(player, NETIS_BOW, 1);
                        ctx.take_items(player, NETIS_DAGGER, 1);
                        ctx.take_items(player, MOST_WANTED_LIST, 1);
                        ctx.take_items(player, STOLEN_JEWELRY, 1);
                        ctx.take_items(player, STOLEN_TOMES, 1);
                        ctx.take_items(player, STOLEN_RING, 1);
                        ctx.take_items(player, STOLEN_NECKLACE, 1);
                        ctx.give_items(player, BEZIQUES_RECOMMENDATION, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 20_232);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 26_930);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 33_628);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30379-09.html".to_owned();
                    } else if !ctx.has_quest_items(player, HORSESHOE_OF_LIGHT)
                        && ctx.has_quest_items(player, BEZIQUES_LETTER)
                    {
                        htmltext = "30379-07.html".to_owned();
                    } else if ctx.has_quest_items(player, HORSESHOE_OF_LIGHT) {
                        ctx.take_items(player, HORSESHOE_OF_LIGHT, 1);
                        ctx.give_items(player, MOST_WANTED_LIST, 1);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30379-08.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[NETIS_BOW, NETIS_DAGGER])
                        && !ctx.has_quest_items(player, MOST_WANTED_LIST)
                    {
                        htmltext = "30379-10.html".to_owned();
                    } else if ctx.has_quest_items(player, MOST_WANTED_LIST) {
                        htmltext = "30379-11.html".to_owned();
                    }
                }
                NETI => {
                    if ctx.has_quest_items(player, BEZIQUES_LETTER) {
                        htmltext = "30425-01.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(player, &[HORSESHOE_OF_LIGHT, BEZIQUES_LETTER])
                    {
                        if ctx.has_quest_items(player, MOST_WANTED_LIST) {
                            htmltext = "30425-08.html".to_owned();
                        } else if ctx.get_quest_items_count(player, SPARTOIS_BONES) < REQUIRED_ITEM_COUNT {
                            htmltext = "30425-06.html".to_owned();
                        } else {
                            // Java takes exactly REQUIRED_ITEM_COUNT bones, not all of them.
                            ctx.take_items(player, SPARTOIS_BONES, REQUIRED_ITEM_COUNT);
                            ctx.give_items(player, HORSESHOE_OF_LIGHT, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30425-07.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, HORSESHOE_OF_LIGHT) {
                        htmltext = "30425-08.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `checkWeapon(player)`: the Netis' bow or dagger is the weapon in the right hand.
fn check_weapon(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    matches!(ctx.active_weapon_item_id(player), Some(NETIS_BOW | NETIS_DAGGER))
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00403_PathOfTheRogue";

    const NPCS: &str = r#"
        <npc id="30379" type="Folk" level="70" name="Captain Bezique"/>
        <npc id="30425" type="Folk" level="70" name="Neti"/>
        <npc id="20035" type="Monster" level="20" name="Tracker Skeleton"/>
        <npc id="27038" type="Monster" level="30" name="Cat's Eye Bandit"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1180" type="EtcItem" name="Bezique's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1181" type="Weapon" name="Neti's Bow"><set name="is_questitem" val="true"/></item>
        <item id="1182" type="Weapon" name="Neti's Dagger"><set name="is_questitem" val="true"/></item>
        <item id="1183" type="EtcItem" name="Spartoi's Bones"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1184" type="EtcItem" name="Horseshoe of Light"><set name="is_questitem" val="true"/></item>
        <item id="1185" type="EtcItem" name="Most Wanted List"><set name="is_questitem" val="true"/></item>
        <item id="1186" type="EtcItem" name="Stolen Jewelry"><set name="is_questitem" val="true"/></item>
        <item id="1187" type="EtcItem" name="Stolen Tomes"><set name="is_questitem" val="true"/></item>
        <item id="1188" type="EtcItem" name="Stolen Ring"><set name="is_questitem" val="true"/></item>
        <item id="1189" type="EtcItem" name="Stolen Necklace"><set name="is_questitem" val="true"/></item>
        <item id="1190" type="EtcItem" name="Bezique's Recommendation"/>"#;

    struct Npcs {
        bezique: Oid,
        neti: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00403PathOfTheRogue)
            .page("30379-01.htm", &["ACCEPT"])
            .page("30379-05.htm", &["30379-06.htm"])
            .page("30425-01.html", &["30425-02.html", "30425-03.html"])
            .page("30425-02.html", &["30425-05.html"])
            .page("30425-03.html", &["30425-04.html"])
            .page("30425-04.html", &["30425-05.html"])
            .plain(&[
                "30379-02.htm",
                "30379-02a.htm",
                "30379-03.htm",
                "30379-04.htm",
                "30379-06.htm",
                "30379-07.html",
                "30379-08.html",
                "30379-09.html",
                "30379-10.html",
                "30379-11.html",
                "30425-05.html",
                "30425-06.html",
                "30425-07.html",
                "30425-08.html",
            ])
            .npcs(&[CAPTAIN_BEZIQUE, NETI])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00403PathOfTheRogue);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            bezique: t.spawn(CAPTAIN_BEZIQUE, 20),
            neti: t.spawn(NETI, 40),
        };
        (t, npcs)
    }

    /// Hits `npc_id` as the player (`onAttack`), then kills it.
    fn attack_and_kill(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 80);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00403PathOfTheRogue.on_attack(ctx, npc, Oid(1), 1, false, None);
            }
        });
        t.kill(1, mob);
    }

    #[test]
    fn the_accept_page_depends_on_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.bezique).contains("30379-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::ROGUE, vec![]);
        t.quest_page(1, npcs.bezique);
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::MAGE, vec![]);
        t.quest_page(1, npcs.bezique);
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-02.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.bezique);
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-05.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(5000, 1190, 1)],
        );
        t.quest_page(1, npcs.bezique);
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_letter_to_the_recommendation() {
        let (mut t, npcs) = setup();
        // The dagger is worn from the start: the weapon the quest's `onAttack` checks for.
        let mut dagger = item(5001, NETIS_DAGGER, 1);
        dagger.loc = "PAPERDOLL".to_owned();
        dagger.loc_data = 5;
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![dagger]);
        assert!(t.quest_page(1, npcs.bezique).contains("30379-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30379-05.htm"));
        assert!(t.event(1, NAME, "30379-06.htm").contains("30379-06.htm"));
        assert_eq!(t.count(1, BEZIQUES_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.bezique).contains("30379-07.html"));

        // Neti takes the letter and gives the bow (the dagger is already in the bag): cond 2.
        assert!(t.quest_page(1, npcs.neti).contains("30425-01.html"));
        assert!(t.event(1, NAME, "30425-02.html").contains("30425-02.html"));
        assert!(t.event(1, NAME, "30425-05.html").contains("30425-05.html"));
        assert_eq!(
            (
                t.count(1, BEZIQUES_LETTER),
                t.count(1, NETIS_BOW),
                t.count(1, NETIS_DAGGER)
            ),
            (0, 1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Ten spartoi bones (the attack with the dagger counts); the tenth sets cond 3.
        for _ in 0..2000 {
            if t.count(1, SPARTOIS_BONES) >= REQUIRED_ITEM_COUNT {
                break;
            }
            attack_and_kill(&mut t, 20035);
        }
        assert_eq!(t.count(1, SPARTOIS_BONES), REQUIRED_ITEM_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Neti takes the bones and gives the horseshoe: cond 4.
        assert!(t.quest_page(1, npcs.neti).contains("30425-07.html"));
        assert_eq!(
            (t.count(1, SPARTOIS_BONES), t.count(1, HORSESHOE_OF_LIGHT)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Bezique takes the horseshoe and gives the most wanted list: cond 5.
        assert!(t.quest_page(1, npcs.bezique).contains("30379-08.html"));
        assert_eq!(
            (t.count(1, HORSESHOE_OF_LIGHT), t.count(1, MOST_WANTED_LIST)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.neti).contains("30425-08.html"));
        assert!(t.quest_page(1, npcs.bezique).contains("30379-11.html"));

        // The four stolen items from the cat's eye bandit, one random item per kill; the fourth
        // sets cond 6.
        for _ in 0..2000 {
            if STOLEN_ITEMS.iter().all(|&i| t.count(1, i) >= 1) {
                break;
            }
            attack_and_kill(&mut t, CATS_EYE_BANDIT);
        }
        assert!(STOLEN_ITEMS.iter().all(|&i| t.count(1, i) == 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Bezique's reward: adena, the recommendation, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.bezique).contains("30379-09.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, BEZIQUES_RECOMMENDATION), 1);
        assert_eq!(t.count(1, MOST_WANTED_LIST), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_kill_without_the_netis_weapon_counts_for_nothing() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.bezique);
        t.event(1, NAME, "ACCEPT");
        t.event(1, NAME, "30379-06.htm");
        t.take_writes();
        // No weapon: the attack sets the script value to 2, the kill gives nothing.
        for _ in 0..10 {
            attack_and_kill(&mut t, 20035);
        }
        assert_eq!(t.count(1, SPARTOIS_BONES), 0);
        assert!(t.take_writes().is_empty());
    }
}
