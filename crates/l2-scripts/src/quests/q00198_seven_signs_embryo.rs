//! Quest 198: Seven Signs, Embryo (Wood and Franz at the Hideout of the Dawn; Shilen's Evil Thoughts).
// Java: dist/game/data/scripts/quests/Q00198_SevenSignsEmbryo/Q00198_SevenSignsEmbryo.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SHILENS_EVIL_THOUGHTS: i32 = 27346;
const WOOD: i32 = 32593;
const FRANZ: i32 = 32597;

// Items
const SCULPTURE_OF_DOUBT: i32 = 14355;
const DAWNS_BRACELET: i32 = 15312;
/// Java `Inventory.ANCIENT_ADENA_ID`.
const ANCIENT_ADENA: i32 = 5575;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Movie.SSQ_EMBRYO`.
const SSQ_EMBRYO: i32 = 14;
/// Java `NPC_HEAL = new SkillHolder(4065, 8)`.
const NPC_HEAL: SkillKey = SkillKey { id: 4065, level: 8 };
/// Java `Q00197_SevenSignsTheSacredBookOfSeal.class.getSimpleName()`: the quest this one follows.
const SACRED_BOOK_OF_SEAL: &str = "Q00197_SevenSignsTheSacredBookOfSeal";
/// Where Shilen's Evil Thoughts appear (Java `addSpawn(SHILENS_EVIL_THOUGHTS, -23734, -9184, -5384, ...)`).
const SHILEN_X: i32 = -23_734;
const SHILEN_Y: i32 = -9_184;
const SHILEN_Z: i32 = -5_384;

/// Java `private boolean isBusy` of the script: the fight of Shilen's Evil Thoughts runs.
#[derive(Default)]
struct EmbryoState {
    is_busy: bool,
}

/// Java `Q00198_SevenSignsEmbryo`.
pub struct Q00198SevenSignsEmbryo;

impl Script for Q00198SevenSignsEmbryo {
    fn name(&self) -> &'static str {
        "Q00198_SevenSignsEmbryo"
    }

    fn path(&self) -> &'static str {
        "quests/Q00198_SevenSignsEmbryo"
    }

    fn id(&self) -> i32 {
        198
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WOOD]);
        s.add_talk_id(&[WOOD, FRANZ]);
        s.add_kill_id(&[SHILENS_EVIL_THOUGHTS]);
        s.register_quest_items(&[SCULPTURE_OF_DOUBT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(monster) = npc
            && monster.id == SHILENS_EVIL_THOUGHTS
            && event == "despawn"
        {
            if !ctx.is_dead(monster.object_id) {
                ctx.state::<EmbryoState>().is_busy = false;
                ctx.broadcast_npc_say(monster, ChatType::NpcGeneral, NpcStringId::NextTimeYouWillNotEscape, &[]);
                ctx.delete_npc(monster);
            }
            // Java `super.onEvent(...)`: the base quest has no answer.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32593-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32597-02.html" | "32597-03.html" | "32597-04.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "fight" => {
                htmltext = Some("32597-05.html".to_owned());
                if qs.is_cond(ctx, 1) {
                    // Java `npc` is Franz here (the bypass of his page).
                    let Some(franz) = npc else {
                        return htmltext;
                    };
                    ctx.state::<EmbryoState>().is_busy = true;
                    let name = ctx.player_name(player);
                    ctx.broadcast_npc_say(
                        franz,
                        ChatType::NpcGeneral,
                        NpcStringId::S1ThatStrangerMustBeDefeatedHereIsTheUltimateHelp,
                        &[&name],
                    );
                    let heal_in = 30_000 - i64::from(ctx.get_random(20_000));
                    ctx.start_quest_timer("heal", heal_in, Some(franz), Some(player), false);
                    let instance = ctx.world().instance_id_of(franz.object_id);
                    if let Some(monster) = ctx.add_spawn(
                        SHILENS_EVIL_THOUGHTS,
                        SHILEN_X,
                        SHILEN_Y,
                        SHILEN_Z,
                        0,
                        false,
                        0,
                        instance,
                    ) {
                        ctx.broadcast_npc_say(monster, ChatType::NpcGeneral, NpcStringId::YouAreNotTheOwnerOfThatItem, &[]);
                        ctx.set_running(monster);
                        ctx.add_attack_desire(monster, player, 999);
                        ctx.set_intention_attack(monster, player);
                        ctx.start_quest_timer("despawn", 300_000, Some(monster), None, false);
                    }
                }
            }
            "heal" => {
                if let Some(franz) = npc {
                    let inside = match ctx.world().position_of(player) {
                        Some((x, y, z)) => ctx.is_inside_radius_3d(franz, x, y, z, 600),
                        None => false,
                    };
                    if !inside {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            franz,
                            ChatType::NpcGeneral,
                            NpcStringId::LookHereS1DonTFallTooFarBehind,
                            &[&name],
                        );
                    } else if !ctx.is_dead(player) {
                        ctx.cast_skill(franz, player, NPC_HEAL);
                    }
                    let heal_in = 30_000 - i64::from(ctx.get_random(20_000));
                    ctx.start_quest_timer("heal", heal_in, Some(franz), Some(player), false);
                }
            }
            "32597-08.html" | "32597-09.html" | "32597-10.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32597-11.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    ctx.take_items(player, SCULPTURE_OF_DOUBT, -1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                    if let Some(franz) = npc {
                        ctx.broadcast_npc_say(franz, ChatType::NpcGeneral, NpcStringId::WeWillBeWithYouAlways, &[]);
                    }
                }
            }
            "32617-02.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let in_range = match ctx.world().position_of(member) {
            Some((x, y, z)) => ctx.is_inside_radius_3d(npc, x, y, z, ctx.alt_party_range()),
            None => false,
        };
        if in_range {
            ctx.give_items(member, SCULPTURE_OF_DOUBT, 1);
            if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 2, true);
            }
        }
        ctx.state::<EmbryoState>().is_busy = false;
        ctx.cancel_quest_timers("despawn");
        ctx.cancel_quest_timers("heal");
        let name = ctx.player_name(member);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::S1YouMayHaveWonThisTimeButNextTimeIWillSurelyCaptureYou,
            &[&name],
        );
        ctx.delete_npc(npc);
        ctx.play_movie(member, SSQ_EMBRYO);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == WOOD {
                    let book_done = ctx
                        .player_quest_state(player, SACRED_BOOK_OF_SEAL)
                        .is_some_and(|qs197| qs197.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && book_done {
                        "32593-01.htm"
                    } else {
                        "32593-03.html"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                if npc.id == WOOD {
                    let cond = qs.get_cond(ctx);
                    if cond > 0 && cond < 3 {
                        htmltext = "32593-04.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            ctx.add_exp_and_sp(player, 315_108_090, 34_906_059);
                            ctx.give_items(player, DAWNS_BRACELET, 1);
                            ctx.give_items(player, ANCIENT_ADENA, 1_500_000);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "32593-05.html".to_owned();
                        } else {
                            htmltext = "level_check.html".to_owned();
                        }
                    }
                } else if npc.id == FRANZ {
                    match qs.get_cond(ctx) {
                        1 => {
                            htmltext = if ctx.state::<EmbryoState>().is_busy {
                                "32597-06.html"
                            } else {
                                "32597-01.html"
                            }
                            .to_owned();
                        }
                        2 => {
                            if ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                                htmltext = "32597-07.html".to_owned();
                            }
                        }
                        3 => htmltext = "32597-12.html".to_owned(),
                        _ => {}
                    }
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00198_SevenSignsEmbryo";

    const NPCS: &str = r#"
        <npc id="27346" type="Monster" level="80" name="Shilen's Evil Thoughts"/>
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32597" type="Folk" level="80" name="Franz"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5575" type="EtcItem" name="Ancient Adena"><set name="is_stackable" val="true"/></item>
        <item id="14355" type="EtcItem" name="Sculpture of Doubt"><set name="is_questitem" val="true"/></item>
        <item id="15312" type="Armor" name="Dawn's Bracelet"><set name="bodypart" val="rbracelet"/></item>"#;

    /// Java `Q00197_SevenSignsTheSacredBookOfSeal` as far as this quest reads it: a script of that name.
    struct SacredBookOfSeal;

    impl Script for SacredBookOfSeal {
        fn name(&self) -> &'static str {
            SACRED_BOOK_OF_SEAL
        }
        fn path(&self) -> &'static str {
            "quests/Q00197_SevenSignsTheSacredBookOfSeal"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        wood: Oid,
        franz: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SacredBookOfSeal);
        registry.add(Q00198SevenSignsEmbryo);
        let pages = Pages::of(&Q00198SevenSignsEmbryo)
            .page("32593-01.htm", &["32593-02.html"])
            .plain(&["32593-02.html", "32593-03.html", "32593-04.html", "32593-05.html", "level_check.html"])
            .page("32597-01.html", &["32597-02.html"])
            .page("32597-02.html", &["32597-03.html"])
            .page("32597-03.html", &["32597-04.html"])
            .page("32597-04.html", &["fight"])
            .plain(&["32597-05.html", "32597-06.html"])
            .page("32597-07.html", &["32597-08.html"])
            .page("32597-08.html", &["32597-09.html"])
            .page("32597-09.html", &["32597-10.html"])
            .page("32597-10.html", &["32597-11.html"])
            .plain(&["32597-11.html", "32597-12.html", "32617-02.html"])
            .npcs(&[WOOD, FRANZ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            wood: t.spawn(WOOD, 20),
            franz: t.spawn(FRANZ, 40),
        };
        (t, npcs)
    }

    fn finish_sacred_book(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(SACRED_BOOK_OF_SEAL, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_level_79_and_the_sacred_book() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.wood).contains("32593-03.html"));
        finish_sacred_book(&mut t, 1);
        assert!(t.quest_page(1, npcs.wood).contains("32593-01.htm"));
        t.enter_as(2, 78, race::HUMAN, 0, vec![]);
        finish_sacred_book(&mut t, 2);
        assert!(t.quest_page(2, npcs.wood).contains("32593-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fight_the_sculpture_and_the_bracelet() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        finish_sacred_book(&mut t, 1);
        assert!(t.quest_page(1, npcs.wood).contains("32593-01.htm"));
        assert!(t.event(1, NAME, "32593-02.html").contains("32593-02.html"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, npcs.franz).contains("32597-01.html"));
        assert!(t.event(1, NAME, "32597-02.html").contains("32597-02.html"));
        assert!(t.event(1, NAME, "32597-03.html").contains("32597-03.html"));
        assert!(t.event(1, NAME, "32597-04.html").contains("32597-04.html"));

        // The fight: Shilen's Evil Thoughts appear at the hideout; they are brought in reach and
        // killed (the sculpture, cond 2, the movie).
        assert!(t.event(1, NAME, "fight").contains("32597-05.html"));
        let shilen = *t.npcs_of(SHILENS_EVIL_THOUGHTS).first().unwrap();
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(shilen).unwrap();
            ctx.tele_npc_to(npc, crate::testing::X, crate::testing::Y, crate::testing::Z);
        });
        t.kill(1, shilen);
        assert_eq!(t.count(1, SCULPTURE_OF_DOUBT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.franz).contains("32597-07.html"));

        // Franz takes the sculpture (cond 3).
        assert!(t.event(1, NAME, "32597-08.html").contains("32597-08.html"));
        assert!(t.event(1, NAME, "32597-09.html").contains("32597-09.html"));
        assert!(t.event(1, NAME, "32597-10.html").contains("32597-10.html"));
        assert!(t.event(1, NAME, "32597-11.html").contains("32597-11.html"));
        assert_eq!(t.count(1, SCULPTURE_OF_DOUBT), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Wood pays: exp, sp, the bracelet, ancient adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.wood).contains("32593-05.html"));
        assert_eq!(t.count(1, DAWNS_BRACELET), 1);
        assert_eq!(t.count(1, ANCIENT_ADENA), 1_500_000);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, npcs.wood).contains("already been completed"));
    }
}
