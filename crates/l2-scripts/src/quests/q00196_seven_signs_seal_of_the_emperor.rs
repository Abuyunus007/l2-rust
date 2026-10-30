//! Quest 196: Seven Signs, Seal of the Emperor (Iason Heine, Merchant of Mammon, Shunaiman, Wood, Court Magician).
// Java: dist/game/data/scripts/quests/Q00196_SevenSignsSealOfTheEmperor/Q00196_SevenSignsSealOfTheEmperor.java

use l2_protocol_game::server::ChatType;
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const IASON_HEINE: i32 = 30969;
const MERCHANT_OF_MAMMON: i32 = 32584;
const SHUNAIMAN: i32 = 32586;
const WOOD: i32 = 32593;
const COURT_MAGICIAN: i32 = 32598;

// Items
const ELMOREDEN_HOLY_WATER: i32 = 13808;
const COURT_MAGICIANS_MAGIC_STAFF: i32 = 13809;
const SEAL_OF_BINDING: i32 = 13846;
const SACRED_SWORD_OF_EINHASAD: i32 = 15310;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Q00195_SevenSignsSecretRitualOfThePriests.class.getSimpleName()`: the quest this one follows.
const SECRET_RITUAL: &str = "Q00195_SevenSignsSecretRitualOfThePriests";

/// Java `private boolean isBusy` of the script.
#[derive(Default)]
struct SealState {
    is_busy: bool,
}

/// Java `Q00196_SevenSignsSealOfTheEmperor`.
pub struct Q00196SevenSignsSealOfTheEmperor;

impl Script for Q00196SevenSignsSealOfTheEmperor {
    fn name(&self) -> &'static str {
        "Q00196_SevenSignsSealOfTheEmperor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00196_SevenSignsSealOfTheEmperor"
    }

    fn id(&self) -> i32 {
        196
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[MERCHANT_OF_MAMMON]);
        s.add_start_npc(&[IASON_HEINE]);
        s.add_talk_id(&[IASON_HEINE, MERCHANT_OF_MAMMON, SHUNAIMAN, WOOD, COURT_MAGICIAN]);
        s.register_quest_items(&[
            ELMOREDEN_HOLY_WATER,
            COURT_MAGICIANS_MAGIC_STAFF,
            SEAL_OF_BINDING,
            SACRED_SWORD_OF_EINHASAD,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc
            && npc.id == MERCHANT_OF_MAMMON
            && event == "DESPAWN"
        {
            ctx.state::<SealState>().is_busy = false;
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::TheAncientPromiseToTheEmperorHasBeenFulfilled,
                &[],
            );
            ctx.delete_npc(npc);
            // Java `super.onEvent(...)`: the base quest has no answer.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30969-02.htm" | "30969-03.htm" | "30969-04.htm" => htmltext = Some(event.to_owned()),
            "30969-05.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "ssq_mammon" => {
                if qs.is_cond(ctx, 1) {
                    if !ctx.state::<SealState>().is_busy {
                        let npc = npc?;
                        ctx.state::<SealState>().is_busy = true;
                        ctx.set_script_value(npc, 1);
                        if let Some(merchant) =
                            ctx.add_spawn(MERCHANT_OF_MAMMON, 109_743, 219_975, -3_512, 0, false, 0, 0)
                        {
                            ctx.broadcast_npc_say(
                                merchant,
                                ChatType::NpcGeneral,
                                NpcStringId::WhoDaresSummonTheMerchantOfMammon,
                                &[],
                            );
                            ctx.start_quest_timer("DESPAWN", 120_000, Some(merchant), None, false);
                        }
                        htmltext = Some("30969-06.html".to_owned());
                    } else {
                        htmltext = Some("30969-07.html".to_owned());
                    }
                }
            }
            "30969-13.html" => {
                if qs.is_cond(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30969-14.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32584-02.html" | "32584-03.html" | "32584-04.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32584-05.html" => {
                if qs.is_cond(ctx, 1) {
                    let npc = npc?;
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                    ctx.cancel_quest_timers("DESPAWN");
                    ctx.delete_npc(npc);
                    ctx.state::<SealState>().is_busy = false;
                }
            }
            "32586-02.html" | "32586-03.html" | "32586-04.html" | "32586-06.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32586-07.html" => {
                if qs.is_cond(ctx, 3) {
                    ctx.give_items(player, ELMOREDEN_HOLY_WATER, 1);
                    ctx.give_items(player, SACRED_SWORD_OF_EINHASAD, 1);
                    qs.set_cond(ctx, 4, true);
                    ctx.send_system_message(
                        player,
                        SystemMessageId::ByUsingTheSkillOfEinhasadSHolySwordDefeatTheEvilLilims,
                    );
                    ctx.send_system_message(
                        player,
                        SystemMessageId::ByUsingTheHolyWaterOfEinhasadOpenTheDoorPossessedByTheCurseOfFlames,
                    );
                    htmltext = Some(event.to_owned());
                }
            }
            "32586-11.html" | "32586-12.html" | "32586-13.html" => {
                if qs.is_cond(ctx, 4) && ctx.get_quest_items_count(player, SEAL_OF_BINDING) >= 4 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32586-14.html" => {
                if qs.is_cond(ctx, 4) && ctx.get_quest_items_count(player, SEAL_OF_BINDING) >= 4 {
                    ctx.take_items_many(
                        player,
                        -1,
                        &[
                            ELMOREDEN_HOLY_WATER,
                            COURT_MAGICIANS_MAGIC_STAFF,
                            SEAL_OF_BINDING,
                            SACRED_SWORD_OF_EINHASAD,
                        ],
                    );
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "finish" => {
                if qs.is_cond(ctx, 6) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some("32593-02.html".to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
                }
            }
            "32598-02.html" => {
                if qs.is_cond(ctx, 3) || qs.is_cond(ctx, 4) {
                    ctx.give_items(player, COURT_MAGICIANS_MAGIC_STAFF, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32584.htm".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == IASON_HEINE {
                    let done = ctx
                        .player_quest_state(player, SECRET_RITUAL)
                        .is_some_and(|q195| q195.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && done {
                        "30969-01.htm"
                    } else {
                        "30969-08.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                IASON_HEINE => match qs.get_cond(ctx) {
                    1 => htmltext = "30969-09.html".to_owned(),
                    2 => {
                        qs.set_cond(ctx, 3, true);
                        ctx.set_script_value(npc, 0);
                        htmltext = "30969-10.html".to_owned();
                    }
                    3 | 4 => htmltext = "30969-11.html".to_owned(),
                    5 => htmltext = "30969-12.html".to_owned(),
                    6 => htmltext = "30969-15.html".to_owned(),
                    _ => {}
                },
                MERCHANT_OF_MAMMON => {
                    if qs.is_cond(ctx, 1) {
                        if ctx.is_script_value(npc, 0) {
                            ctx.set_script_value(npc, player.0);
                        }
                        htmltext = if ctx.is_script_value(npc, player.0) {
                            "32584-01.html"
                        } else {
                            "32584-06.html"
                        }
                        .to_owned();
                    }
                }
                SHUNAIMAN => match qs.get_cond(ctx) {
                    3 => htmltext = "32586-01.html".to_owned(),
                    4 => {
                        if ctx.get_quest_items_count(player, SEAL_OF_BINDING) < 4 {
                            let water = ctx.has_quest_items(player, ELMOREDEN_HOLY_WATER);
                            let sword = ctx.has_quest_items(player, SACRED_SWORD_OF_EINHASAD);
                            if water && sword {
                                htmltext = "32586-08.html".to_owned();
                            } else if !water && sword {
                                htmltext = "32586-09.html".to_owned();
                                ctx.give_items(player, ELMOREDEN_HOLY_WATER, 1);
                            } else if water && !sword {
                                htmltext = "32586-09.html".to_owned();
                                ctx.give_items(player, SACRED_SWORD_OF_EINHASAD, 1);
                            }
                            ctx.send_system_message(
                                player,
                                SystemMessageId::ByUsingTheSkillOfEinhasadSHolySwordDefeatTheEvilLilims,
                            );
                            ctx.send_system_message(
                                player,
                                SystemMessageId::ByUsingTheHolyWaterOfEinhasadOpenTheDoorPossessedByTheCurseOfFlames,
                            );
                        } else {
                            htmltext = "32586-10.html".to_owned();
                        }
                    }
                    5 => htmltext = "32586-15.html".to_owned(),
                    _ => {}
                },
                WOOD => {
                    if qs.is_cond(ctx, 6) {
                        htmltext = "32593-01.html".to_owned();
                    }
                }
                COURT_MAGICIAN => {
                    if qs.is_cond(ctx, 3) || qs.is_cond(ctx, 4) {
                        htmltext = if !ctx.has_quest_items(player, COURT_MAGICIANS_MAGIC_STAFF) {
                            "32598-01.html"
                        } else {
                            "32598-03.html"
                        }
                        .to_owned();
                        ctx.send_system_message(
                            player,
                            SystemMessageId::ByUsingTheCourtMagicianSMagicStaffOpenTheDoorOnWhichTheMagicianSBarrierIsPlaced,
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00196_SevenSignsSealOfTheEmperor";

    const NPCS: &str = r#"
        <npc id="30969" type="Folk" level="80" name="Iason Heine"/>
        <npc id="32584" type="Folk" level="80" name="Merchant of Mammon"/>
        <npc id="32586" type="Folk" level="80" name="Shunaiman"/>
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32598" type="Folk" level="80" name="Court Magician"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13808" type="EtcItem" name="Holy Water"><set name="is_questitem" val="true"/></item>
        <item id="13809" type="EtcItem" name="Staff"><set name="is_questitem" val="true"/></item>
        <item id="13846" type="EtcItem" name="Seal of Binding"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15310" type="EtcItem" name="Sword"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00195_SevenSignsSecretRitualOfThePriests` as far as this quest reads it.
    struct SecretRitual;

    impl Script for SecretRitual {
        fn name(&self) -> &'static str {
            SECRET_RITUAL
        }
        fn path(&self) -> &'static str {
            "quests/Q00195_SevenSignsSecretRitualOfThePriests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        iason: Oid,
        shunaiman: Oid,
        wood: Oid,
        magician: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SecretRitual);
        registry.add(Q00196SevenSignsSealOfTheEmperor);
        let pages = Pages::of(&Q00196SevenSignsSealOfTheEmperor)
            .page("30969-01.htm", &["30969-02.htm"])
            .page("30969-02.htm", &["30969-03.htm"])
            .page("30969-03.htm", &["30969-04.htm"])
            .page("30969-04.htm", &["30969-05.html"])
            .plain(&["30969-05.html", "30969-08.html", "30969-10.html", "30969-11.html"])
            .page("30969-09.html", &["ssq_mammon"])
            .plain(&["30969-06.html", "30969-07.html"])
            .page("30969-12.html", &["30969-13.html"])
            .page("30969-13.html", &["30969-14.html"])
            .plain(&["30969-14.html", "30969-15.html"])
            .page("32586-01.html", &["32586-02.html"])
            .page("32586-02.html", &["32586-03.html"])
            .page("32586-03.html", &["32586-04.html"])
            .page("32586-04.html", &["32586-06.html"])
            .page("32586-06.html", &["32586-07.html"])
            .plain(&["32586-07.html", "32586-08.html", "32586-09.html", "32586-15.html"])
            .page("32586-10.html", &["32586-11.html"])
            .page("32586-11.html", &["32586-12.html"])
            .page("32586-12.html", &["32586-13.html"])
            .page("32586-13.html", &["32586-14.html"])
            .plain(&["32586-14.html"])
            .page("32593-01.html", &["finish"])
            .plain(&["32593-02.html", "level_check.html"])
            .page("32598-01.html", &["32598-02.html"])
            .plain(&["32598-02.html", "32598-03.html"])
            .npcs(&[IASON_HEINE, SHUNAIMAN, WOOD, COURT_MAGICIAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            iason: t.spawn(IASON_HEINE, 20),
            shunaiman: t.spawn(SHUNAIMAN, 40),
            wood: t.spawn(WOOD, 60),
            magician: t.spawn(COURT_MAGICIAN, 80),
        };
        (t, npcs)
    }

    fn finish_secret_ritual(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(SECRET_RITUAL, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_level_79_and_the_secret_ritual() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.iason).contains("30969-08.html"));
        finish_secret_ritual(&mut t, 1);
        assert!(t.quest_page(1, npcs.iason).contains("30969-01.htm"));
        t.enter_as(2, 78, race::HUMAN, 0, vec![]);
        finish_secret_ritual(&mut t, 2);
        assert!(t.quest_page(2, npcs.iason).contains("30969-08.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_merchant_of_mammon_through_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        finish_secret_ritual(&mut t, 1);
        assert!(t.quest_page(1, npcs.iason).contains("30969-01.htm"));
        for e in ["30969-02.htm", "30969-03.htm", "30969-04.htm", "30969-05.html"] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Summon the merchant; the timer despawns him later.
        assert!(t.quest_page(1, npcs.iason).contains("30969-09.html"));
        assert!(t.event(1, NAME, "ssq_mammon").contains("30969-06.html"));
        let merchant = *t.npcs_of(MERCHANT_OF_MAMMON).first().unwrap();
        assert!(t.quest_page(1, npcs.iason).contains("30969-09.html"));
        assert!(t.event(1, NAME, "ssq_mammon").contains("30969-07.html"));

        // The merchant is far away in the test world: his event runs from his own place.
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(merchant).unwrap();
            let player = Oid(1);
            let qs = ctx.get_quest_state(player, false).unwrap();
            let answer =
                Q00196SevenSignsSealOfTheEmperor.on_event(ctx, "32584-05.html", Some(npc), Some(player));
            assert_eq!(answer.as_deref(), Some("32584-05.html"));
            assert!(qs.is_cond(ctx, 2));
        });
        assert!(!t.in_world(merchant));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Back to Iason: cond 3.
        assert!(t.quest_page(1, npcs.iason).contains("30969-10.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Court magician's staff and Shunaiman's items, cond 4.
        assert!(t.quest_page(1, npcs.magician).contains("32598-01.html"));
        assert!(t.event(1, NAME, "32598-02.html").contains("32598-02.html"));
        assert_eq!(t.count(1, COURT_MAGICIANS_MAGIC_STAFF), 1);
        assert!(t.quest_page(1, npcs.shunaiman).contains("32586-01.html"));
        for e in [
            "32586-02.html",
            "32586-03.html",
            "32586-04.html",
            "32586-06.html",
            "32586-07.html",
        ] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert_eq!(t.count(1, ELMOREDEN_HOLY_WATER), 1);
        assert_eq!(t.count(1, SACRED_SWORD_OF_EINHASAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Four seals of binding: all four items go, cond 5.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), SEAL_OF_BINDING, 4));
        t.take_writes();
        assert!(t.quest_page(1, npcs.shunaiman).contains("32586-10.html"));
        for e in ["32586-11.html", "32586-12.html", "32586-13.html", "32586-14.html"] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert_eq!(t.count(1, SEAL_OF_BINDING), 0);
        assert_eq!(t.count(1, ELMOREDEN_HOLY_WATER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Iason, then Wood pays.
        assert!(t.quest_page(1, npcs.iason).contains("30969-12.html"));
        assert!(t.event(1, NAME, "30969-13.html").contains("30969-13.html"));
        assert!(t.event(1, NAME, "30969-14.html").contains("30969-14.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.wood).contains("32593-01.html"));
        assert!(t.event(1, NAME, "finish").contains("32593-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
