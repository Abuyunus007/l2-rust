//! Quest 171: Acts of Evil (Guard Alvah and five NPCs, blade molds, reports and Ol Mahum heads).
// Java: dist/game/data/scripts/quests/Q00171_ActsOfEvil/Q00171_ActsOfEvil.java
//
// Pattern: a long chain driven by the memo state (set together with the cond), kills that give
// items by memo state, and a quest monster that is spawned at the killer and despawns by timer.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TRADER_ARODIN: i32 = 30207;
const GUARD_ALVAH: i32 = 30381;
const TYRA: i32 = 30420;
const NETI: i32 = 30425;
const TRADER_ROLENTO: i32 = 30437;
const TUREK_CHIEF_BURAI: i32 = 30617;

// Items
const BLADE_MOLD: i32 = 4239;
const TYRAS_BILL: i32 = 4240;
const RANGERS_REPORT1: i32 = 4241;
const RANGERS_REPORT2: i32 = 4242;
const RANGERS_REPORT3: i32 = 4243;
const RANGERS_REPORT4: i32 = 4244;
const WEAPONS_TRADE_CONTRACT: i32 = 4245;
const ATTACK_DIRECTIVES: i32 = 4246;
const CERTIFICATE_OF_THE_SILVER_GUILD: i32 = 4247;
const ROLENTOS_CARGOBOX: i32 = 4248;
const OL_MAHUM_CAPTAINS_HEAD: i32 = 4249;

// Monster
const TUMRAN_BUGBEAR: i32 = 20062;
const TUMRAN_BUGBEAR_WARRIOR: i32 = 20064;
const OL_MAHUM_CAPTAIN: i32 = 20066;
const OL_MAHUM_GENERAL: i32 = 20438;
const TUREK_ORC_ARCHER: i32 = 20496;
const TUREK_ORC_SKIRMISHER: i32 = 20497;
const TUREK_ORC_SUPPLIER: i32 = 20498;
const TUREK_ORC_FOOTMAN: i32 = 20499;

// Quest Monster
const OL_MAHUM_SUPPORT_TROOP: i32 = 27190;

// Misc
const MIN_LEVEL: i32 = 27;

/// Java `Q00171_ActsOfEvil`.
pub struct Q00171ActsOfEvil;

/// The blade mold part of `onKill`, the same for the four Turek orcs: a drop with the chance
/// already rolled by the caller, the sound, then the support troop at 5 molds and from 10 molds
/// on with 25 %.
fn blade_mold(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, dropped: bool) {
    if dropped {
        ctx.give_items(killer, BLADE_MOLD, 1);
        if ctx.get_quest_items_count(killer, BLADE_MOLD) == 20 {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
        } else {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    if ctx.get_quest_items_count(killer, BLADE_MOLD) == 5 {
        support_troop(ctx, npc, killer);
    }

    if ctx.get_quest_items_count(killer, BLADE_MOLD) >= 10 && ctx.get_random(100) <= 24 {
        support_troop(ctx, npc, killer);
    }
}

/// `addAttackDesire(addSpawn(OL_MAHUM_SUPPORT_TROOP, npc, true, 0, false), killer)`.
fn support_troop(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId) {
    if let Some(troop) = ctx.add_spawn_at(OL_MAHUM_SUPPORT_TROOP, npc.object_id, true, 0) {
        ctx.add_attack_desire(troop, killer, 999);
    }
}

impl Script for Q00171ActsOfEvil {
    fn name(&self) -> &'static str {
        "Q00171_ActsOfEvil"
    }

    fn path(&self) -> &'static str {
        "quests/Q00171_ActsOfEvil"
    }

    fn id(&self) -> i32 {
        171
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_ALVAH]);
        s.add_talk_id(&[
            GUARD_ALVAH,
            TRADER_ARODIN,
            TYRA,
            NETI,
            TRADER_ROLENTO,
            TUREK_CHIEF_BURAI,
        ]);
        s.add_kill_id(&[
            TUMRAN_BUGBEAR,
            TUMRAN_BUGBEAR_WARRIOR,
            OL_MAHUM_CAPTAIN,
            OL_MAHUM_GENERAL,
            TUREK_ORC_ARCHER,
            TUREK_ORC_SKIRMISHER,
            TUREK_ORC_SUPPLIER,
            TUREK_ORC_FOOTMAN,
            OL_MAHUM_SUPPORT_TROOP,
        ]);
        s.add_spawn_id(&[OL_MAHUM_SUPPORT_TROOP]);
        s.register_quest_items(&[
            BLADE_MOLD,
            TYRAS_BILL,
            RANGERS_REPORT1,
            RANGERS_REPORT2,
            RANGERS_REPORT3,
            RANGERS_REPORT4,
            WEAPONS_TRADE_CONTRACT,
            ATTACK_DIRECTIVES,
            CERTIFICATE_OF_THE_SILVER_GUILD,
            ROLENTOS_CARGOBOX,
            OL_MAHUM_CAPTAINS_HEAD,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN" {
            if let Some(npc) = npc {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::YouShouldConsiderGoingBack,
                    &[],
                );
                ctx.delete_npc(npc);
            }

            // `super.onEvent(event, npc, player)`: the default answers nothing.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30381-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30381-07.html" => {
                qs.set_memo_state(ctx, 5);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30381-12.html" => {
                qs.set_memo_state(ctx, 7);
                qs.set_cond(ctx, 7, true);
                htmltext = Some(event.to_owned());
            }
            "30437-04.html" => {
                ctx.take_items(player, WEAPONS_TRADE_CONTRACT, 1);
                ctx.give_items(player, CERTIFICATE_OF_THE_SILVER_GUILD, 1);
                ctx.give_items(player, ROLENTOS_CARGOBOX, 1);
                qs.set_memo_state(ctx, 9);
                qs.set_cond(ctx, 9, true);
                htmltext = Some(event.to_owned());
            }
            "30207-01a.html" | "30437-02.html" | "30437-03.html" | "30617-03.html" | "30617-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "30617-05.html" => {
                ctx.take_items(player, ATTACK_DIRECTIVES, 1);
                ctx.take_items(player, CERTIFICATE_OF_THE_SILVER_GUILD, 1);
                ctx.take_items(player, ROLENTOS_CARGOBOX, 1);
                qs.set_memo_state(ctx, 10);
                qs.set_cond(ctx, 10, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
        {
            match npc.id {
                TUMRAN_BUGBEAR | TUMRAN_BUGBEAR_WARRIOR => {
                    if qs.is_memo_state(ctx, 5) {
                        if !ctx.has_quest_items(killer, RANGERS_REPORT1) {
                            ctx.give_items(killer, RANGERS_REPORT1, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        } else if ctx.has_quest_items(killer, RANGERS_REPORT1)
                            && !ctx.has_quest_items(killer, RANGERS_REPORT2)
                        {
                            if ctx.get_random(100) <= 19 {
                                ctx.give_items(killer, RANGERS_REPORT2, 1);
                                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                            }
                        } else if ctx.has_all_quest_items(killer, &[RANGERS_REPORT1, RANGERS_REPORT2])
                            && !ctx.has_quest_items(killer, RANGERS_REPORT3)
                        {
                            if ctx.get_random(100) <= 19 {
                                ctx.give_items(killer, RANGERS_REPORT3, 1);
                                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                            }
                        } else if ctx
                            .has_all_quest_items(killer, &[RANGERS_REPORT1, RANGERS_REPORT2, RANGERS_REPORT3])
                            && !ctx.has_quest_items(killer, RANGERS_REPORT4)
                            && ctx.get_random(100) <= 19
                        {
                            ctx.give_items(killer, RANGERS_REPORT4, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                OL_MAHUM_CAPTAIN => {
                    if qs.is_memo_state(ctx, 10)
                        && ctx.get_quest_items_count(killer, OL_MAHUM_CAPTAINS_HEAD) < 30
                        && ctx.get_random(100) <= 49
                    {
                        ctx.give_items(killer, OL_MAHUM_CAPTAINS_HEAD, 1);
                        if ctx.get_quest_items_count(killer, OL_MAHUM_CAPTAINS_HEAD) == 30 {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        } else {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                OL_MAHUM_GENERAL => {
                    if qs.is_memo_state(ctx, 6) && ctx.get_random(100) <= 9 {
                        if !ctx.has_quest_items(killer, WEAPONS_TRADE_CONTRACT) {
                            ctx.give_items(killer, WEAPONS_TRADE_CONTRACT, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }

                        if !ctx.has_quest_items(killer, ATTACK_DIRECTIVES) {
                            ctx.give_items(killer, ATTACK_DIRECTIVES, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                TUREK_ORC_ARCHER => {
                    if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, BLADE_MOLD) < 20 {
                        let dropped = ctx.get_random(100) < 53;
                        blade_mold(ctx, npc, killer, dropped);
                    }
                }
                TUREK_ORC_SKIRMISHER => {
                    if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, BLADE_MOLD) < 20 {
                        let dropped = ctx.get_random(100) < 55;
                        blade_mold(ctx, npc, killer, dropped);
                    }
                }
                TUREK_ORC_SUPPLIER => {
                    if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, BLADE_MOLD) < 20 {
                        let dropped = ctx.get_random(100) < 51;
                        blade_mold(ctx, npc, killer, dropped);
                    }
                }
                TUREK_ORC_FOOTMAN => {
                    if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, BLADE_MOLD) < 20 {
                        let dropped = ctx.get_random(2) < 1;
                        blade_mold(ctx, npc, killer, dropped);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == GUARD_ALVAH {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30381-01.htm".to_owned()
                } else {
                    "30381-02.htm".to_owned()
                };
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                GUARD_ALVAH => match qs.get_cond(ctx) {
                    1 => htmltext = "30381-04.html".to_owned(),
                    2 | 3 => htmltext = "30381-05.html".to_owned(),
                    4 => htmltext = "30381-06.html".to_owned(),
                    5 => {
                        if ctx.has_all_quest_items(
                            player,
                            &[RANGERS_REPORT1, RANGERS_REPORT2, RANGERS_REPORT3, RANGERS_REPORT4],
                        ) {
                            ctx.take_items(player, RANGERS_REPORT1, 1);
                            ctx.take_items(player, RANGERS_REPORT2, 1);
                            ctx.take_items(player, RANGERS_REPORT3, 1);
                            ctx.take_items(player, RANGERS_REPORT4, 1);
                            qs.set_memo_state(ctx, 6);
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30381-09.html".to_owned();
                        } else {
                            htmltext = "30381-08.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.has_all_quest_items(player, &[WEAPONS_TRADE_CONTRACT, ATTACK_DIRECTIVES]) {
                            htmltext = "30381-11.html".to_owned();
                        } else {
                            htmltext = "30381-10.html".to_owned();
                        }
                    }
                    7 => htmltext = "30381-13.html".to_owned(),
                    8 => htmltext = "30381-14.html".to_owned(),
                    9 => htmltext = "30381-15.html".to_owned(),
                    10 => htmltext = "30381-16.html".to_owned(),
                    11 => {
                        ctx.give_adena(player, 95_000, true);
                        ctx.add_exp_and_sp(player, 159_820, 9_182);
                        htmltext = "30381-17.html".to_owned();
                        qs.exit_quest(ctx, false, true);
                    }
                    _ => {}
                },
                TRADER_ARODIN => {
                    if memo_state == 1 {
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30207-01.html".to_owned();
                    } else if memo_state == 2 {
                        if ctx.get_quest_items_count(player, BLADE_MOLD) < 20 {
                            htmltext = "30207-02.html".to_owned();
                        } else {
                            htmltext = "30207-03.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        ctx.take_items(player, TYRAS_BILL, 1);
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30207-04.html".to_owned();
                    } else if memo_state >= 4 {
                        htmltext = "30207-05.html".to_owned();
                    }
                }
                TYRA => {
                    if memo_state == 2 {
                        if ctx.get_quest_items_count(player, BLADE_MOLD) < 20 {
                            htmltext = "30420-01.html".to_owned();
                        } else {
                            ctx.take_items(player, BLADE_MOLD, -1);
                            ctx.give_items(player, TYRAS_BILL, 1);
                            qs.set_memo_state(ctx, 3);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "30420-02.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        htmltext = "30420-03.html".to_owned();
                    } else if memo_state >= 4 {
                        htmltext = "30420-04.html".to_owned();
                    }
                }
                NETI => {
                    if memo_state == 7 {
                        qs.set_memo_state(ctx, 8);
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30425-01.html".to_owned();
                    } else if memo_state == 8 {
                        htmltext = "30425-02.html".to_owned();
                    } else if memo_state >= 9 {
                        htmltext = "30425-03.html".to_owned();
                    }
                }
                TRADER_ROLENTO => {
                    if memo_state == 8 {
                        htmltext = "30437-02.html".to_owned();
                    } else if memo_state == 9 {
                        htmltext = "30437-05.html".to_owned();
                    } else if memo_state >= 10 {
                        htmltext = "30437-06.html".to_owned();
                    }
                }
                TUREK_CHIEF_BURAI => {
                    if memo_state < 9 {
                        htmltext = "30617-01.html".to_owned();
                    } else if memo_state == 9 {
                        htmltext = "30617-02.html".to_owned();
                    } else if memo_state == 10 {
                        if ctx.get_quest_items_count(player, OL_MAHUM_CAPTAINS_HEAD) < 30 {
                            htmltext = "30617-06.html".to_owned();
                        } else {
                            ctx.give_adena(player, 8_000, true);
                            ctx.take_items(player, OL_MAHUM_CAPTAINS_HEAD, -1);
                            qs.set_memo_state(ctx, 11);
                            qs.set_cond(ctx, 11, true);
                            htmltext = "30617-07.html".to_owned();
                        }
                    } else if memo_state == 11 {
                        htmltext = "30617-08.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == GUARD_ALVAH {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("DESPAWN", 200_000, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00171_ActsOfEvil";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30207" type="Folk" level="1" name="Arodin"/>
        <npc id="30381" type="Folk" level="1" name="Alvah"/>
        <npc id="30420" type="Folk" level="1" name="Tyra"/>
        <npc id="30425" type="Folk" level="1" name="Neti"/>
        <npc id="30437" type="Folk" level="1" name="Rolento"/>
        <npc id="30617" type="Folk" level="1" name="Burai"/>
        <npc id="20062" type="Monster" level="27" name="Tumran Bugbear"/>
        <npc id="20066" type="Monster" level="27" name="Ol Mahum Captain"/>
        <npc id="20438" type="Monster" level="27" name="Ol Mahum General"/>
        <npc id="20499" type="Monster" level="27" name="Turek Orc Footman"/>
        <npc id="27190" type="Monster" level="27" name="Ol Mahum Support Troop"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4239" type="EtcItem" name="Blade Mold"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4240" type="EtcItem" name="Tyra's Bill"><set name="is_questitem" val="true"/></item>
        <item id="4241" type="EtcItem" name="Ranger's Report 1"><set name="is_questitem" val="true"/></item>
        <item id="4242" type="EtcItem" name="Ranger's Report 2"><set name="is_questitem" val="true"/></item>
        <item id="4243" type="EtcItem" name="Ranger's Report 3"><set name="is_questitem" val="true"/></item>
        <item id="4244" type="EtcItem" name="Ranger's Report 4"><set name="is_questitem" val="true"/></item>
        <item id="4245" type="EtcItem" name="Weapons Trade Contract"><set name="is_questitem" val="true"/></item>
        <item id="4246" type="EtcItem" name="Attack Directives"><set name="is_questitem" val="true"/></item>
        <item id="4247" type="EtcItem" name="Certificate of the Silver Guild"><set name="is_questitem" val="true"/></item>
        <item id="4248" type="EtcItem" name="Rolento's Cargobox"><set name="is_questitem" val="true"/></item>
        <item id="4249" type="EtcItem" name="Ol Mahum Captain's Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        alvah: Oid,
        arodin: Oid,
        tyra: Oid,
        neti: Oid,
        rolento: Oid,
        burai: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00171ActsOfEvil);
        let pages = Pages::of(&Q00171ActsOfEvil)
            .page("30207-01.html", &["30207-01a.html"])
            .page("30207-02.html", &["30207-01a.html"])
            .page("30381-02.htm", &["30381-03.htm"])
            .page("30381-06.html", &["30381-07.html"])
            .page("30381-11.html", &["30381-12.html"])
            .page("30437-02.html", &["30437-03.html"])
            .page("30437-03.html", &["30437-04.html"])
            .page("30617-02.html", &["30617-03.html"])
            .page("30617-03.html", &["30617-04.html"])
            .page("30617-04.html", &["30617-05.html"])
            .plain(&[
                "30207-01a.html",
                "30207-03.html",
                "30207-04.html",
                "30207-05.html",
                "30381-01.htm",
                "30381-03.htm",
                "30381-04.html",
                "30381-05.html",
                "30381-07.html",
                "30381-08.html",
                "30381-09.html",
                "30381-10.html",
                "30381-12.html",
                "30381-13.html",
                "30381-14.html",
                "30381-15.html",
                "30381-16.html",
                "30381-17.html",
                "30420-01.html",
                "30420-02.html",
                "30420-03.html",
                "30420-04.html",
                "30425-01.html",
                "30425-02.html",
                "30425-03.html",
                "30437-04.html",
                "30437-05.html",
                "30437-06.html",
                "30617-01.html",
                "30617-05.html",
                "30617-06.html",
                "30617-07.html",
                "30617-08.html",
            ])
            .npcs(&[
                GUARD_ALVAH,
                TRADER_ARODIN,
                TYRA,
                NETI,
                TRADER_ROLENTO,
                TUREK_CHIEF_BURAI,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            alvah: t.spawn(GUARD_ALVAH, 20),
            arodin: t.spawn(TRADER_ARODIN, 25),
            tyra: t.spawn(TYRA, 30),
            neti: t.spawn(NETI, 35),
            rolento: t.spawn(TRADER_ROLENTO, 40),
            burai: t.spawn(TUREK_CHIEF_BURAI, 45),
        };
        (t, npcs)
    }

    /// Kills `npc_id` until `done` holds (the drops are rolls).
    fn kill_until(t: &mut ScriptTest, npc_id: i32, done: impl Fn(&ScriptTest) -> bool) {
        let mut kills = 0;
        while !done(t) {
            let mob = t.spawn(npc_id, 60);
            t.kill(1, mob);
            // The support troop of the molds attacks the test character: remove it.
            for troop in t.npcs_of(OL_MAHUM_SUPPORT_TROOP) {
                t.world.delete_npc(troop);
            }
            kills += 1;
            assert!(kills < 2000, "never done with {npc_id}");
        }
    }

    #[test]
    fn a_character_below_level_27_does_not_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 26, vec![]);
        assert!(t.quest_page(1, npcs.alvah).contains("30381-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_chain_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 27, vec![]);
        assert!(t.quest_page(1, npcs.alvah).contains("30381-02.htm"));
        assert!(t.event(1, NAME, "30381-03.htm").contains("30381-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1")
            ]
        );
        assert!(t.quest_page(1, npcs.alvah).contains("30381-04.html"));

        // Arodin: memo 2, cond 2.
        assert!(t.quest_page(1, npcs.arodin).contains("30207-01.html"));
        assert!(t.event(1, NAME, "30207-01a.html").contains("30207-01a.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.arodin).contains("30207-02.html"));
        assert!(t.quest_page(1, npcs.tyra).contains("30420-01.html"));

        // Twenty blade molds from the footmen; the support troop appears at 5 (and later).
        kill_until(&mut t, TUREK_ORC_FOOTMAN, |t| t.count(1, BLADE_MOLD) >= 20);
        assert_eq!(t.count(1, BLADE_MOLD), 20);
        t.take_writes();
        assert!(t.quest_page(1, npcs.arodin).contains("30207-03.html"));

        // Tyra swaps the molds for a bill, Arodin takes the bill.
        assert!(t.quest_page(1, npcs.tyra).contains("30420-02.html"));
        assert_eq!(t.count(1, TYRAS_BILL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );
        assert!(t.quest_page(1, npcs.arodin).contains("30207-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );
        assert!(t.quest_page(1, npcs.alvah).contains("30381-06.html"));
        assert!(t.event(1, NAME, "30381-07.html").contains("30381-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // Four ranger reports from the bugbears.
        assert!(t.quest_page(1, npcs.alvah).contains("30381-08.html"));
        kill_until(&mut t, TUMRAN_BUGBEAR, |t| t.count(1, RANGERS_REPORT4) == 1);
        t.take_writes();
        assert!(t.quest_page(1, npcs.alvah).contains("30381-09.html"));
        assert_eq!(t.count(1, RANGERS_REPORT1), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "6")
            ]
        );

        // The general drops the contract and the directives.
        assert!(t.quest_page(1, npcs.alvah).contains("30381-10.html"));
        kill_until(&mut t, OL_MAHUM_GENERAL, |t| t.count(1, ATTACK_DIRECTIVES) == 1);
        t.take_writes();
        assert!(t.quest_page(1, npcs.alvah).contains("30381-11.html"));
        assert!(t.event(1, NAME, "30381-12.html").contains("30381-12.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "7")
            ]
        );
        assert!(t.quest_page(1, npcs.alvah).contains("30381-13.html"));

        // Neti, then Rolento hands the certificate and the cargo box.
        assert!(t.quest_page(1, npcs.neti).contains("30425-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "8"),
                rows::update(NAME, "cond", "8")
            ]
        );
        assert!(t.quest_page(1, npcs.rolento).contains("30437-02.html"));
        assert!(t.event(1, NAME, "30437-03.html").contains("30437-03.html"));
        assert!(t.event(1, NAME, "30437-04.html").contains("30437-04.html"));
        assert_eq!(t.count(1, CERTIFICATE_OF_THE_SILVER_GUILD), 1);
        assert_eq!(t.count(1, ROLENTOS_CARGOBOX), 1);
        assert_eq!(t.count(1, WEAPONS_TRADE_CONTRACT), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "9"),
                rows::update(NAME, "cond", "9")
            ]
        );

        // Burai: three pages, memo 10; then thirty heads for 8000 adena.
        assert!(t.quest_page(1, npcs.burai).contains("30617-02.html"));
        assert!(t.event(1, NAME, "30617-03.html").contains("30617-03.html"));
        assert!(t.event(1, NAME, "30617-04.html").contains("30617-04.html"));
        assert!(t.event(1, NAME, "30617-05.html").contains("30617-05.html"));
        assert_eq!(t.count(1, ATTACK_DIRECTIVES), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "10"),
                rows::update(NAME, "cond", "10")
            ]
        );
        kill_until(&mut t, OL_MAHUM_CAPTAIN, |t| {
            t.count(1, OL_MAHUM_CAPTAINS_HEAD) >= 30
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.burai).contains("30617-07.html"));
        assert_eq!(t.count(1, ADENA), 8_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "11"),
                rows::update(NAME, "cond", "11")
            ]
        );

        // Alvah pays the reward.
        assert!(t.quest_page(1, npcs.alvah).contains("30381-17.html"));
        assert_eq!(t.count(1, ADENA), 8_000 + 95_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn the_support_troop_says_goodbye_and_leaves_after_200_seconds() {
        let (mut t, _) = setup();
        t.enter(1, 27, vec![]);
        let troop = t.spawn(OL_MAHUM_SUPPORT_TROOP, 80);
        t.take();
        t.wait_ms(100_000);
        assert!(t.in_world(troop));
        t.wait_ms(101_000);
        assert!(!t.in_world(troop));
    }
}
