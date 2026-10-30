//! Quest 501: Proof of Clan Alliance (the clan leader's alliance manifesto: loyalty symbols, a
//! poison, herbs and Athrea's boxes).
// Java: dist/game/data/scripts/quests/Q00501_ProofOfClanAlliance/Q00501_ProofOfClanAlliance.java
//
// Pattern: a clan quest. The leader starts it and keeps the memo state; the clan members work on
// the leader's quest state (`getLeaderQuestState`) while holding their own state for the symbols.
// Athrea spawns sixteen boxes (summoned NPCs) and counts the kills in its script value.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SIR_KRISTOF_RODEMAI: i32 = 30756;
const STATUE_OF_OFFERING: i32 = 30757;
const ATHREA: i32 = 30758;
const KALIS: i32 = 30759;

// Monsters
const OEL_MAHUM_WITCH_DOCTOR: i32 = 20576;
const HARIT_LIZARDMAN_SHAMAN: i32 = 20644;
const VANOR_SILENOS_SHAMAN: i32 = 20685;
const BOX_OF_ATHREA_1: i32 = 27173;
const BOX_OF_ATHREA_2: i32 = 27174;
const BOX_OF_ATHREA_3: i32 = 27175;
const BOX_OF_ATHREA_4: i32 = 27176;
const BOX_OF_ATHREA_5: i32 = 27177;

// Items
const HERB_OF_HARIT: i32 = 3832;
const HERB_OF_VANOR: i32 = 3833;
const HERB_OF_OEL_MAHUM: i32 = 3834;
const BLOOD_OF_EVA: i32 = 3835;
const ATHREAS_COIN: i32 = 3836;
const SYMBOL_OF_LOYALTY: i32 = 3837;
const ANTIDOTE_RECIPE_LIST: i32 = 3872;
const VOUCHER_OF_FAITH: i32 = 3873;
const ALLIANCE_MANIFESTO: i32 = 3874;
const POTION_OF_RECOVERY: i32 = 3889;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Skills
const POISON_OF_DEATH: SkillKey = SkillKey { id: 4082, level: 1 };
const DIE_YOU_FOOL: SkillKey = SkillKey { id: 4083, level: 1 };

// Locations
const LOCS: [(i32, i32, i32); 16] = [
    (102273, 103433, -3512),
    (102190, 103379, -3524),
    (102107, 103325, -3533),
    (102024, 103271, -3500),
    (102327, 103350, -3511),
    (102244, 103296, -3518),
    (102161, 103242, -3529),
    (102078, 103188, -3500),
    (102381, 103267, -3538),
    (102298, 103213, -3532),
    (102215, 103159, -3520),
    (102132, 103105, -3513),
    (102435, 103184, -3515),
    (102352, 103130, -3522),
    (102269, 103076, -3533),
    (102186, 103022, -3541),
];

// Misc
const CLAN_MIN_LEVEL: i32 = 3;
const CLAN_MEMBER_MIN_LEVEL: i32 = 40;
const ADENA_TO_RESTART_GAME: i64 = 10000;

/// Java `Q00501_ProofOfClanAlliance`.
pub struct Q00501ProofOfClanAlliance;

/// Java `hasAbnormal(player)`: the player has the `FATAL_POISON` abnormal.
fn has_abnormal(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().script_buff_abnormal_level(player, "FATAL_POISON").is_some()
}

/// Java `getLeaderQuestState(player, getName())`: the quest state of the clan leader, who has to
/// be online. Returns the leader too (Java's `lqs.getPlayer()`).
fn leader_quest_state(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<(Qs, ObjectId)> {
    let clan_id = ctx.world().clan_id_of(player);
    if clan_id == 0 {
        return None;
    }
    let leader = ctx.world().clan_online_leader(clan_id)?;
    // Java `leader.getQuestState(quest)`: no state is created.
    ctx.get_quest_state(leader, false).map(|qs| (qs, leader))
}

/// Java's override of `getRandomPartyMemberState(player, condition, playerChance, target)`: the
/// condition is ignored, the killer counts `player_chance` times, the pick must be in range.
fn random_party_member_state(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    player_chance: i32,
    target: ScriptNpc,
) -> Option<Qs> {
    if ctx.world().player(player).is_none() || player_chance < 1 {
        return None;
    }

    let qs = ctx.get_quest_state(player, false);
    if !ctx.is_in_party(player) {
        if !ctx.check_if_in_range(ctx.alt_party_range(), player, target.object_id, true) {
            return None;
        }

        return qs;
    }

    let mut candidates: Vec<Qs> = Vec::new();
    if let Some(qs) = qs {
        for _ in 0..player_chance {
            candidates.push(qs);
        }
    }

    for member in ctx.world().party_members_of(player) {
        if member == player {
            continue;
        }

        if let Some(qs) = ctx.get_quest_state(member, false) {
            candidates.push(qs);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let qs = candidates[ctx.get_random(candidates.len() as i32) as usize];
    if !ctx.check_if_in_range(ctx.alt_party_range(), qs.player(), target.object_id, true) {
        return None;
    }

    Some(qs)
}

impl Script for Q00501ProofOfClanAlliance {
    fn name(&self) -> &'static str {
        "Q00501_ProofOfClanAlliance"
    }

    fn path(&self) -> &'static str {
        "quests/Q00501_ProofOfClanAlliance"
    }

    fn id(&self) -> i32 {
        501
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIR_KRISTOF_RODEMAI, STATUE_OF_OFFERING]);
        s.add_talk_id(&[SIR_KRISTOF_RODEMAI, STATUE_OF_OFFERING, ATHREA, KALIS]);
        s.add_kill_id(&[
            OEL_MAHUM_WITCH_DOCTOR,
            HARIT_LIZARDMAN_SHAMAN,
            VANOR_SILENOS_SHAMAN,
            BOX_OF_ATHREA_1,
            BOX_OF_ATHREA_2,
            BOX_OF_ATHREA_3,
            BOX_OF_ATHREA_4,
            BOX_OF_ATHREA_5,
        ]);
        s.register_quest_items(&[
            ANTIDOTE_RECIPE_LIST,
            VOUCHER_OF_FAITH,
            HERB_OF_HARIT,
            HERB_OF_VANOR,
            HERB_OF_OEL_MAHUM,
            BLOOD_OF_EVA,
            ATHREAS_COIN,
            SYMBOL_OF_LOYALTY,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "30756-06.html" | "30756-08.html" | "30757-05.html" | "30758-02.html" | "30758-04.html"
            | "30759-02.html" | "30759-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "30756-07.html" => {
                if qs.is_created(ctx)
                    && ctx.is_clan_leader(player)
                    && ctx.clan_level_of(player) == Some(CLAN_MIN_LEVEL)
                {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30757-04.html" => {
                let npc = npc?;
                if ctx.get_random(10) > 5 {
                    if qs.get_int(ctx, "flag") != 2501 {
                        ctx.give_items(player, SYMBOL_OF_LOYALTY, 1);
                        qs.set_int(ctx, "flag", 2501);
                    }

                    htmltext = Some(event.to_owned());
                } else {
                    ctx.cast_skill(npc, player, DIE_YOU_FOOL);
                    ctx.start_quest_timer("SYMBOL_OF_LOYALTY", 4000, Some(npc), Some(player), false);
                    htmltext = Some("30757-03.html".to_owned());
                }
            }
            "30758-03.html" => {
                let npc = npc?;
                if let Some((lqs, _)) = leader_quest_state(ctx, player) {
                    if ctx.summoned_npc_count(npc) < 4 {
                        lqs.set_memo_state(ctx, 4);
                        lqs.set_int(ctx, "flag", 0);
                        ctx.set_script_value(npc, 0);
                        for (x, y, z) in LOCS {
                            let id = ctx.get_random_range(BOX_OF_ATHREA_1, BOX_OF_ATHREA_5);
                            if let Some(boxed) = ctx.add_spawn_summoned(npc, id, x, y, z, 0, false, 300_000, 0) {
                                ctx.disable_core_ai(boxed, true);
                                ctx.set_random_walking(boxed, false);
                            }
                        }

                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30758-03a.html".to_owned());
                    }
                }
            }
            "30758-07.html" => {
                let npc = npc?;
                if ctx.adena_of(player) >= ADENA_TO_RESTART_GAME {
                    if ctx.summoned_npc_count(npc) < 4 {
                        ctx.take_items(player, ADENA_ID, ADENA_TO_RESTART_GAME);
                    }

                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30758-06.html".to_owned());
                }
            }
            "30759-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some(event.to_owned());
                }
            }
            "30759-07.html" => {
                let npc = npc?;
                if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(player, SYMBOL_OF_LOYALTY) >= 3 {
                    ctx.take_items(player, SYMBOL_OF_LOYALTY, -1);
                    ctx.give_items(player, ANTIDOTE_RECIPE_LIST, 1);
                    ctx.cast_skill(npc, player, POISON_OF_DEATH);
                    qs.set_cond(ctx, 3, true);
                    qs.set_memo_state(ctx, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "SYMBOL_OF_LOYALTY" => {
                if ctx.is_dead(player) && qs.get_int(ctx, "flag") != 2501 {
                    ctx.give_items(player, SYMBOL_OF_LOYALTY, 1);
                    qs.set_int(ctx, "flag", 2501);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = random_party_member_state(ctx, killer, 3, npc) else {
            return;
        };

        let player = qs.player();
        let Some((lqs, _)) = leader_quest_state(ctx, player) else {
            return;
        };
        match npc.id {
            OEL_MAHUM_WITCH_DOCTOR | HARIT_LIZARDMAN_SHAMAN | VANOR_SILENOS_SHAMAN => {
                let herb = match npc.id {
                    OEL_MAHUM_WITCH_DOCTOR => HERB_OF_OEL_MAHUM,
                    HARIT_LIZARDMAN_SHAMAN => HERB_OF_HARIT,
                    _ => HERB_OF_VANOR,
                };
                if ctx.get_random(10) == 1 && lqs.get_memo_state(ctx) >= 3 && lqs.get_memo_state(ctx) < 6 {
                    ctx.give_item_randomly(player, Some(npc), herb, 1, 1, 0, 1.0, true);
                }
            }
            BOX_OF_ATHREA_1 | BOX_OF_ATHREA_2 | BOX_OF_ATHREA_3 | BOX_OF_ATHREA_4 | BOX_OF_ATHREA_5 => {
                let summoner = ctx.summoner(npc).and_then(|s| ctx.script_npc(s));
                if let Some(arthea) = summoner
                    && lqs.is_memo_state(ctx, 4)
                {
                    let flag = lqs.get_int(ctx, "flag");
                    if flag == 3 && ctx.is_script_value(arthea, 15)
                        || flag == 2 && ctx.is_script_value(arthea, 14)
                        || flag == 1 && ctx.is_script_value(arthea, 13)
                        || flag == 0 && ctx.is_script_value(arthea, 12)
                        || flag < 4 && ctx.get_random(4) == 0
                    {
                        // The Java chain has a branch per flag with the same body; the random
                        // branch only runs when none of the exact ones matched.
                        lqs.set_int(ctx, "flag", flag + 1);
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::Bingo, &[]);
                    }

                    let value = ctx.get_script_value(arthea);
                    ctx.set_script_value(arthea, value + 1);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let lqs = leader_quest_state(ctx, player);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            SIR_KRISTOF_RODEMAI => match qs.state(ctx) {
                CREATED => {
                    if ctx.is_clan_leader(player) {
                        // `isClanLeader()` holds, so the clan exists.
                        let level = ctx.clan_level_of(player).unwrap_or(0);
                        if level < CLAN_MIN_LEVEL {
                            htmltext = "30756-01.html".to_owned();
                        } else if level == CLAN_MIN_LEVEL {
                            if ctx.has_quest_items(player, ALLIANCE_MANIFESTO) {
                                htmltext = "30756-03.html".to_owned();
                            } else {
                                htmltext = "30756-04.html".to_owned();
                            }
                        } else {
                            htmltext = "30756-02.html".to_owned();
                        }
                    } else {
                        htmltext = "30756-05.html".to_owned();
                    }
                }
                STARTED => {
                    if qs.is_memo_state(ctx, 6) && ctx.has_quest_items(player, VOUCHER_OF_FAITH) {
                        ctx.take_items(player, VOUCHER_OF_FAITH, -1);
                        ctx.give_items(player, ALLIANCE_MANIFESTO, 1);
                        ctx.add_exp_and_sp(player, 0, 120000);
                        qs.exit_quest(ctx, false, false);
                        htmltext = "30756-09.html".to_owned();
                    } else {
                        htmltext = "30756-10.html".to_owned();
                    }
                }
                _ => {}
            },
            STATUE_OF_OFFERING => {
                if let Some((lqs, _)) = lqs
                    && lqs.is_memo_state(ctx, 2)
                {
                    if !ctx.is_clan_leader(player) {
                        if ctx.player_level(player) >= CLAN_MEMBER_MIN_LEVEL {
                            htmltext = if qs.get_int(ctx, "flag") != 2501 {
                                "30757-01.html"
                            } else {
                                "30757-01b.html"
                            }
                            .to_owned();
                        } else {
                            htmltext = "30757-02.html".to_owned();
                        }
                    } else {
                        htmltext = "30757-01a.html".to_owned();
                    }
                } else {
                    htmltext = "30757-06.html".to_owned();
                }
            }
            ATHREA => {
                if let Some((lqs, leader)) = lqs {
                    match lqs.get_memo_state(ctx) {
                        3 => {
                            if ctx.has_quest_items(leader, ANTIDOTE_RECIPE_LIST)
                                && !ctx.has_quest_items(leader, BLOOD_OF_EVA)
                            {
                                lqs.set_int(ctx, "flag", 0);
                                htmltext = "30758-01.html".to_owned();
                            }
                        }
                        4 => {
                            if lqs.get_int(ctx, "flag") < 4 {
                                htmltext = "30758-05.html".to_owned();
                            } else {
                                ctx.give_items(player, BLOOD_OF_EVA, 1);
                                lqs.set_memo_state(ctx, 5);
                                htmltext = "30758-08.html".to_owned();
                            }
                        }
                        5 => htmltext = "30758-09.html".to_owned(),
                        _ => {}
                    }
                }
            }
            KALIS => {
                let symbols = ctx.get_quest_items_count(player, SYMBOL_OF_LOYALTY);
                if qs.is_memo_state(ctx, 1) && !ctx.has_quest_items(player, SYMBOL_OF_LOYALTY) {
                    htmltext = "30759-01.html".to_owned();
                } else if qs.is_memo_state(ctx, 2) && symbols < 3 {
                    htmltext = "30759-05.html".to_owned();
                } else if symbols >= 3 && !has_abnormal(ctx, player) {
                    htmltext = "30759-06.html".to_owned();
                } else if qs.is_memo_state(ctx, 5)
                    && ctx.has_quest_items(player, BLOOD_OF_EVA)
                    && ctx.has_quest_items(player, HERB_OF_VANOR)
                    && ctx.has_quest_items(player, HERB_OF_HARIT)
                    && ctx.has_quest_items(player, HERB_OF_OEL_MAHUM)
                    && has_abnormal(ctx, player)
                {
                    ctx.give_items(player, VOUCHER_OF_FAITH, 1);
                    ctx.give_items(player, POTION_OF_RECOVERY, 1);
                    ctx.take_items(player, BLOOD_OF_EVA, -1);
                    ctx.take_items(player, ANTIDOTE_RECIPE_LIST, -1);
                    ctx.take_items(player, HERB_OF_OEL_MAHUM, -1);
                    ctx.take_items(player, HERB_OF_HARIT, -1);
                    ctx.take_items(player, HERB_OF_VANOR, -1);
                    qs.set_cond(ctx, 4, true);
                    qs.set_memo_state(ctx, 6);
                    htmltext = "30759-08.html".to_owned();
                } else if (qs.is_memo_state(ctx, 3) || qs.is_memo_state(ctx, 4) || qs.is_memo_state(ctx, 5))
                    && !has_abnormal(ctx, player)
                {
                    ctx.take_items(player, ANTIDOTE_RECIPE_LIST, -1);
                    qs.set_memo_state(ctx, 1);
                    htmltext = "30759-09.html".to_owned();
                } else if qs.get_memo_state(ctx) < 6
                    && symbols >= 3
                    && !ctx.has_at_least_one_quest_item(
                        player,
                        &[BLOOD_OF_EVA, HERB_OF_VANOR, HERB_OF_HARIT, HERB_OF_OEL_MAHUM],
                    )
                    && has_abnormal(ctx, player)
                {
                    htmltext = "30759-10.html".to_owned();
                } else if qs.is_memo_state(ctx, 6) {
                    htmltext = "30759-11.html".to_owned();
                } else if lqs.is_some() && !ctx.is_clan_leader(player) {
                    htmltext = "30759-12.html".to_owned();
                }
            }
            _ => {}
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

    const NAME: &str = "Q00501_ProofOfClanAlliance";

    const NPCS: &str = r#"
        <npc id="30756" type="Folk" level="70" name="Sir Kristof Rodemai"/>
        <npc id="30757" type="Folk" level="70" name="Statue of Offering"/>
        <npc id="30758" type="Folk" level="70" name="Athrea"/>
        <npc id="30759" type="Folk" level="70" name="Kalis"/>
        <npc id="20576" type="Monster" level="30" name="Oel Mahum Witch Doctor"/>
        <npc id="27173" type="Monster" level="30" name="Box of Athrea"/>
        <npc id="27174" type="Monster" level="30" name="Box of Athrea"/>
        <npc id="27175" type="Monster" level="30" name="Box of Athrea"/>
        <npc id="27176" type="Monster" level="30" name="Box of Athrea"/>
        <npc id="27177" type="Monster" level="30" name="Box of Athrea"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3834" type="EtcItem" name="Herb of Oel Mahum"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3835" type="EtcItem" name="Blood of Eva"><set name="is_questitem" val="true"/></item>
        <item id="3837" type="EtcItem" name="Symbol of Loyalty"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3872" type="EtcItem" name="Antidote Recipe List"><set name="is_questitem" val="true"/></item>
        <item id="3873" type="EtcItem" name="Voucher of Faith"><set name="is_questitem" val="true"/></item>
        <item id="3874" type="EtcItem" name="Alliance Manifesto"/>
        <item id="3889" type="EtcItem" name="Potion of Recovery"/>"#;

    fn setup() -> (ScriptTest, Vec<Oid>) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00501ProofOfClanAlliance);
        let pages = Pages::of(&Q00501ProofOfClanAlliance)
            .page("30756-04.html", &["30756-06.html"])
            .page("30756-06.html", &["30756-07.html"])
            .page("30759-01.html", &["30759-03.html"])
            .page("30758-01.html", &["30758-03.html"])
            .plain(&[
                "30756-01.html",
                "30756-05.html",
                "30756-07.html",
                "30756-09.html",
                "30756-10.html",
                "30757-01a.html",
                "30757-06.html",
                "30758-03.html",
                "30758-03a.html",
                "30759-03.html",
            ])
            .npcs(&[30756, 30757, 30758, 30759]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = [30756, 30757, 30758, 30759]
            .iter()
            .enumerate()
            .map(|(i, id)| t.spawn(*id, 20 + 5 * i as i32))
            .collect();
        (t, npcs)
    }

    /// A level 40 character who leads a clan of the given level.
    fn leader(t: &mut ScriptTest, clan_level: i32) {
        t.enter(1, 40, vec![]);
        let clan = t.world.create_clan(Oid(1), "TestClan").expect("clan");
        t.world.change_clan_level(clan, clan_level);
    }

    #[test]
    fn a_member_who_is_not_the_leader_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, npcs[0]).contains("30756-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_low_level_clan_is_refused() {
        let (mut t, npcs) = setup();
        leader(&mut t, 2);
        assert!(t.quest_page(1, npcs[0]).contains("30756-01.html"));
    }

    #[test]
    fn the_leader_of_a_level_3_clan_starts_and_goes_on_to_the_poison() {
        let (mut t, npcs) = setup();
        let (kristof, statue, _athrea, kalis) = (npcs[0], npcs[1], npcs[2], npcs[3]);
        leader(&mut t, 3);
        assert!(t.quest_page(1, kristof).contains("30756-04.html"));
        t.event(1, NAME, "30756-06.html");
        assert!(t.event(1, NAME, "30756-07.html").contains("30756-07.html"));
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "<state>", "Started")), "{writes:?}");

        // Kalis sends the leader to the statues (memo 1 -> 2, cond 2).
        assert!(t.quest_page(1, kalis).contains("30759-01.html"));
        assert!(t.event(1, NAME, "30759-03.html").contains("30759-03.html"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            assert_eq!((qs.get_memo_state(ctx), qs.get_cond(ctx)), (2, 2));
        });

        // The leader at the statue: his own page.
        assert!(t.quest_page(1, statue).contains("30757-01a.html"));

        // Three symbols: Kalis poisons, the recipe comes, cond 3 memo 3.
        t.world.run_as_script(NAME, |ctx| ctx.give_items(Oid(1), SYMBOL_OF_LOYALTY, 3));
        t.take_writes();
        let _ = t.quest_page(1, kalis);
        t.world.run_as_script(NAME, |ctx| {
            let npc = ScriptNpc {
                object_id: kalis,
                id: KALIS,
            };
            let page = Q00501ProofOfClanAlliance.on_event(ctx, "30759-07.html", Some(npc), Some(Oid(1)));
            assert_eq!(page, Some("30759-07.html".to_owned()));
        });
        assert_eq!(
            (t.count(1, SYMBOL_OF_LOYALTY), t.count(1, ANTIDOTE_RECIPE_LIST)),
            (0, 1)
        );
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            assert_eq!((qs.get_memo_state(ctx), qs.get_cond(ctx)), (3, 3));
        });
    }

    #[test]
    fn athrea_spawns_sixteen_boxes_once_and_herbs_drop_in_memo_3_to_5() {
        let (mut t, npcs) = setup();
        let athrea = npcs[2];
        leader(&mut t, 3);
        t.quest_page(1, npcs[0]);
        t.event(1, NAME, "30756-06.html");
        t.event(1, NAME, "30756-07.html");
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 3);
            ctx.give_items(Oid(1), ANTIDOTE_RECIPE_LIST, 1);
        });
        t.take_writes();

        let npc = ScriptNpc {
            object_id: athrea,
            id: ATHREA,
        };
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00501ProofOfClanAlliance.on_event(ctx, "30758-03.html", Some(npc), Some(Oid(1)));
            assert_eq!(page, Some("30758-03.html".to_owned()));
            assert_eq!(ctx.summoned_npc_count(npc), 16);
            // A second time: the boxes are still there.
            let page = Q00501ProofOfClanAlliance.on_event(ctx, "30758-03.html", Some(npc), Some(Oid(1)));
            assert_eq!(page, Some("30758-03a.html".to_owned()));
        });

        // The boxes are far from the character (outside the party range): a kill does nothing.
        let boxes: Vec<_> = (27173..=27177).flat_map(|id| t.npcs_of(id)).collect();
        assert_eq!(boxes.len(), 16);
        t.kill(1, boxes[0]);
        assert_eq!(t.world.npc_script_value(athrea), 0);

        // A box next to the character: Athrea's script value counts the kill.
        let near = t.spawn(27173, 60);
        t.world.add_summoned_npc(athrea, near);
        t.kill(1, near);
        assert_eq!(t.world.npc_script_value(athrea), 1);

        // Herbs: a Witch Doctor drops one with 10% while the memo state is 3..5.
        for _ in 0..300 {
            let mob = t.spawn(20576, 60);
            t.kill(1, mob);
            if t.count(1, HERB_OF_OEL_MAHUM) > 0 {
                return;
            }
        }
        panic!("no herb in 300 kills");
    }

    #[test]
    fn the_voucher_buys_the_manifesto_and_ends_the_quest() {
        let (mut t, npcs) = setup();
        leader(&mut t, 3);
        t.quest_page(1, npcs[0]);
        t.event(1, NAME, "30756-06.html");
        t.event(1, NAME, "30756-07.html");
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 6);
            ctx.give_items(Oid(1), VOUCHER_OF_FAITH, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs[0]).contains("30756-09.html"));
        assert_eq!((t.count(1, ALLIANCE_MANIFESTO), t.count(1, VOUCHER_OF_FAITH)), (1, 0));
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
    }
}
