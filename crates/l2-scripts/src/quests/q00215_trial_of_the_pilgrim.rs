//! Quest 215: Trial of the Pilgrim (Hermit Santiago, a healer's second-class trial).
// Java: dist/game/data/scripts/quests/Q00215_TrialOfThePilgrim/Q00215_TrialOfThePilgrim.java
//
// Pattern: a long memo-state chain over many NPCs with three kill steps (`docs/design/scripts.md`,
// "Quest patterns"). The memo state is the quest's own step counter (`qs.setMemoState`), the
// condition is the journal step. One-time: `exit_quest(ctx, false, true)`.

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PRIEST_PETRON: i32 = 30036;
const PRIEST_PRIMOS: i32 = 30117;
const ANDELLIA: i32 = 30362;
const GAURI_TWINKLEROCK: i32 = 30550;
const SEER_TANAPI: i32 = 30571;
const ELDER_CASIAN: i32 = 30612;
const HERMIT_SANTIAGO: i32 = 30648;
const ANCESTOR_MARTANKUS: i32 = 30649;
const PRIEST_OF_THE_EARTH_GERALD: i32 = 30650;
const WANDERER_DORF: i32 = 30651;
const URUHA: i32 = 30652;

// Items
const ADENA: i32 = 57;
const BOOK_OF_SAGE: i32 = 2722;
const VOUCHER_OF_TRIAL: i32 = 2723;
const SPIRIT_OF_FLAME: i32 = 2724;
const ESSENSE_OF_FLAME: i32 = 2725;
const BOOK_OF_GERALD: i32 = 2726;
const GREY_BADGE: i32 = 2727;
const PICTURE_OF_NAHIR: i32 = 2728;
const HAIR_OF_NAHIR: i32 = 2729;
const STATUE_OF_EINHASAD: i32 = 2730;
const BOOK_OF_DARKNESS: i32 = 2731;
const DEBRIS_OF_WILLOW: i32 = 2732;
const TAG_OF_RUMOR: i32 = 2733;

// Reward
const MARK_OF_PILGRIM: i32 = 2721;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Quest monsters
const LAVA_SALAMANDER: i32 = 27116;
const NAHIR: i32 = 27117;
const BLACK_WILLOW: i32 = 27118;

// Misc
const MIN_LEVEL: i32 = 35;
/// Java `CategoryType.HEAL_GROUP` (`CategoryData.xml`: cleric, oracle, shillien oracle, orc
/// shaman).
// FOLLOW-UP(engine): `l2-world/src/subclass/categories.rs` has no `HEAL_GROUP` row, so
// `player.isInCategory(HEAL_GROUP)` is checked against these ids here. Move the row into the
// table and call `ctx.world().is_in_category(player, CategoryType::HealGroup)` instead.
const HEAL_GROUP: [i32; 4] = [15, 29, 42, 50];
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00215_TrialOfThePilgrim`.
pub struct Q00215TrialOfThePilgrim;

fn in_heal_group(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    HEAL_GROUP.contains(&ctx.player_class_id(player))
}

impl Script for Q00215TrialOfThePilgrim {
    fn name(&self) -> &'static str {
        "Q00215_TrialOfThePilgrim"
    }

    fn path(&self) -> &'static str {
        "quests/Q00215_TrialOfThePilgrim"
    }

    fn id(&self) -> i32 {
        215
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HERMIT_SANTIAGO]);
        s.add_talk_id(&[
            HERMIT_SANTIAGO,
            PRIEST_PETRON,
            PRIEST_PRIMOS,
            ANDELLIA,
            GAURI_TWINKLEROCK,
            SEER_TANAPI,
            ELDER_CASIAN,
            ANCESTOR_MARTANKUS,
            PRIEST_OF_THE_EARTH_GERALD,
            WANDERER_DORF,
            URUHA,
        ]);
        s.add_kill_id(&[LAVA_SALAMANDER, NAHIR, BLACK_WILLOW]);
        s.register_quest_items(&[
            BOOK_OF_SAGE,
            VOUCHER_OF_TRIAL,
            SPIRIT_OF_FLAME,
            ESSENSE_OF_FLAME,
            BOOK_OF_GERALD,
            GREY_BADGE,
            PICTURE_OF_NAHIR,
            HAIR_OF_NAHIR,
            STATUE_OF_EINHASAD,
            BOOK_OF_DARKNESS,
            DEBRIS_OF_WILLOW,
            TAG_OF_RUMOR,
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
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, VOUCHER_OF_TRIAL, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 49);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30648-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30648-04.htm".to_owned());
                    }
                }
            }
            "30648-05.html" | "30648-06.html" | "30648-07.html" | "30648-08.html" => {
                htmltext = Some(event.to_owned());
            }
            "30362-05.html" => {
                if qs.is_memo_state(ctx, 15) && ctx.has_quest_items(player, BOOK_OF_DARKNESS) {
                    ctx.take_items(player, BOOK_OF_DARKNESS, 1);
                    qs.set_memo_state(ctx, 16);
                    qs.set_cond(ctx, 16, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30362-04.html" => {
                if qs.is_memo_state(ctx, 15) && ctx.has_quest_items(player, BOOK_OF_DARKNESS) {
                    qs.set_memo_state(ctx, 16);
                    qs.set_cond(ctx, 16, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30649-04.html" => {
                if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(player, ESSENSE_OF_FLAME) {
                    ctx.give_items(player, SPIRIT_OF_FLAME, 1);
                    ctx.take_items(player, ESSENSE_OF_FLAME, 1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30650-02.html" => {
                if qs.is_memo_state(ctx, 6) && ctx.has_quest_items(player, TAG_OF_RUMOR) {
                    if ctx.get_quest_items_count(player, ADENA) >= 100_000 {
                        ctx.give_items(player, BOOK_OF_GERALD, 1);
                        ctx.take_items(player, ADENA, 100_000);
                        qs.set_memo_state(ctx, 7);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30650-03.html".to_owned());
                    }
                }
            }
            "30650-03.html" => {
                if qs.is_memo_state(ctx, 6) && ctx.has_quest_items(player, TAG_OF_RUMOR) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30652-02.html" => {
                if qs.is_memo_state(ctx, 14) && ctx.has_quest_items(player, DEBRIS_OF_WILLOW) {
                    ctx.give_items(player, BOOK_OF_DARKNESS, 1);
                    ctx.take_items(player, DEBRIS_OF_WILLOW, 1);
                    qs.set_memo_state(ctx, 15);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
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
                LAVA_SALAMANDER => {
                    if qs.is_memo_state(ctx, 3) && !ctx.has_quest_items(killer, ESSENSE_OF_FLAME) {
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 4, true);
                        ctx.give_items(killer, ESSENSE_OF_FLAME, 1);
                    }
                }
                NAHIR => {
                    if qs.is_memo_state(ctx, 10) && !ctx.has_quest_items(killer, HAIR_OF_NAHIR) {
                        qs.set_memo_state(ctx, 11);
                        qs.set_cond(ctx, 11, true);
                        ctx.give_items(killer, HAIR_OF_NAHIR, 1);
                    }
                }
                BLACK_WILLOW => {
                    if qs.is_memo_state(ctx, 13) && !ctx.has_quest_items(killer, DEBRIS_OF_WILLOW) {
                        qs.set_memo_state(ctx, 14);
                        qs.set_cond(ctx, 14, true);
                        ctx.give_items(killer, DEBRIS_OF_WILLOW, 1);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        // Read once, like Java's `final int memoState`: the branches below compare this value.
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == HERMIT_SANTIAGO {
                if !in_heal_group(ctx, player) {
                    htmltext = "30648-02.html".to_owned();
                } else if ctx.player_level(player) < MIN_LEVEL {
                    htmltext = "30648-01.html".to_owned();
                } else {
                    htmltext = "30648-03.htm".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                HERMIT_SANTIAGO => {
                    if memo_state >= 1 {
                        if !ctx.has_quest_items(player, BOOK_OF_SAGE) {
                            htmltext = "30648-09.html".to_owned();
                        } else {
                            ctx.give_adena(player, 229_298, true);
                            ctx.give_items(player, MARK_OF_PILGRIM, 1);
                            ctx.add_exp_and_sp(player, 1_258_250, 81_606);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                player,
                                &SocialAction {
                                    object_id: player.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30648-10.html".to_owned();
                        }
                    }
                }
                PRIEST_PETRON => {
                    if memo_state == 9 {
                        ctx.give_items(player, PICTURE_OF_NAHIR, 1);
                        qs.set_memo_state(ctx, 10);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "30036-01.html".to_owned();
                    } else if memo_state == 10 {
                        htmltext = "30036-02.html".to_owned();
                    } else if memo_state == 11 {
                        ctx.take_items(player, PICTURE_OF_NAHIR, 1);
                        ctx.take_items(player, HAIR_OF_NAHIR, 1);
                        ctx.give_items(player, STATUE_OF_EINHASAD, 1);
                        qs.set_memo_state(ctx, 12);
                        qs.set_cond(ctx, 12, true);
                        htmltext = "30036-03.html".to_owned();
                    } else if memo_state == 12 && ctx.has_quest_items(player, STATUE_OF_EINHASAD) {
                        htmltext = "30036-04.html".to_owned();
                    }
                }
                PRIEST_PRIMOS => {
                    if memo_state == 8 {
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30117-01.html".to_owned();
                    } else if memo_state == 9 {
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30117-02.html".to_owned();
                    }
                }
                ANDELLIA => {
                    if memo_state == 12 {
                        // NOTE(java-quirk): the level check is `>= 0`, so it always passes.
                        if ctx.player_level(player) >= 0 {
                            qs.set_memo_state(ctx, 13);
                            qs.set_cond(ctx, 13, true);
                            htmltext = "30362-01.html".to_owned();
                        } else {
                            htmltext = "30362-01a.html".to_owned();
                        }
                    } else if memo_state == 13 {
                        htmltext = "30362-02.html".to_owned();
                    } else if memo_state == 14 {
                        htmltext = "30362-02a.html".to_owned();
                    } else if memo_state == 15 {
                        if ctx.has_quest_items(player, BOOK_OF_DARKNESS) {
                            htmltext = "30362-03.html".to_owned();
                        } else {
                            htmltext = "30362-07.html".to_owned();
                        }
                    } else if memo_state == 16 {
                        htmltext = "30362-06.html".to_owned();
                    }
                }
                GAURI_TWINKLEROCK => {
                    if memo_state == 5 {
                        if ctx.has_quest_items(player, SPIRIT_OF_FLAME) {
                            ctx.take_items(player, SPIRIT_OF_FLAME, 1);
                            ctx.give_items(player, TAG_OF_RUMOR, 1);
                            qs.set_memo_state(ctx, 6);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30550-01.html".to_owned();
                        }
                    } else if memo_state == 6 {
                        htmltext = "30550-02.html".to_owned();
                    }
                }
                SEER_TANAPI => {
                    if memo_state == 1 {
                        if ctx.has_quest_items(player, VOUCHER_OF_TRIAL) {
                            ctx.take_items(player, VOUCHER_OF_TRIAL, 1);
                            qs.set_memo_state(ctx, 2);
                            qs.set_cond(ctx, 2, true);
                            htmltext = "30571-01.html".to_owned();
                        }
                    } else if memo_state == 2 {
                        htmltext = "30571-02.html".to_owned();
                    } else if memo_state == 5 && ctx.has_quest_items(player, SPIRIT_OF_FLAME) {
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30571-03.html".to_owned();
                    }
                }
                ELDER_CASIAN => {
                    if memo_state == 16 {
                        qs.set_memo_state(ctx, 17);
                        if !ctx.has_quest_items(player, BOOK_OF_SAGE) {
                            ctx.give_items(player, BOOK_OF_SAGE, 1);
                        }

                        ctx.take_items(player, GREY_BADGE, 1);
                        ctx.take_items(player, SPIRIT_OF_FLAME, 1);
                        ctx.take_items(player, STATUE_OF_EINHASAD, 1);
                        if ctx.has_quest_items(player, BOOK_OF_DARKNESS) {
                            ctx.add_exp_and_sp(player, 5000, 500);
                            ctx.take_items(player, BOOK_OF_DARKNESS, 1);
                        }

                        htmltext = "30612-01.html".to_owned();
                    } else if memo_state == 17 {
                        qs.set_cond(ctx, 17, true);
                        htmltext = "30612-02.html".to_owned();
                    }
                }
                ANCESTOR_MARTANKUS => {
                    if memo_state == 2 {
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30649-01.html".to_owned();
                    } else if memo_state == 3 {
                        htmltext = "30649-02.html".to_owned();
                    } else if memo_state == 4 && ctx.has_quest_items(player, ESSENSE_OF_FLAME) {
                        htmltext = "30649-03.html".to_owned();
                    }
                }
                PRIEST_OF_THE_EARTH_GERALD => {
                    if memo_state == 6 {
                        if ctx.has_quest_items(player, TAG_OF_RUMOR) {
                            htmltext = "30650-01.html".to_owned();
                        }
                    } else if ctx.has_all_quest_items(player, &[GREY_BADGE, BOOK_OF_GERALD]) {
                        ctx.give_adena(player, 100_000, true);
                        ctx.take_items(player, BOOK_OF_GERALD, 1);
                        htmltext = "30650-04.html".to_owned();
                    }
                }
                WANDERER_DORF => {
                    if memo_state == 6 {
                        if ctx.has_quest_items(player, TAG_OF_RUMOR) {
                            ctx.give_items(player, GREY_BADGE, 1);
                            ctx.take_items(player, TAG_OF_RUMOR, 1);
                            qs.set_memo_state(ctx, 8);
                            htmltext = "30651-01.html".to_owned();
                        }
                    } else if memo_state == 7 {
                        if ctx.has_quest_items(player, TAG_OF_RUMOR) {
                            ctx.give_items(player, GREY_BADGE, 1);
                            ctx.take_items(player, TAG_OF_RUMOR, 1);
                            qs.set_memo_state(ctx, 8);
                            htmltext = "30651-02.html".to_owned();
                        }
                    } else if memo_state == 8 {
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30651-03.html".to_owned();
                    }
                }
                URUHA => {
                    if memo_state == 14 {
                        if ctx.has_quest_items(player, DEBRIS_OF_WILLOW) {
                            htmltext = "30652-01.html".to_owned();
                        }
                    } else if memo_state == 15 && ctx.has_quest_items(player, BOOK_OF_DARKNESS) {
                        htmltext = "30652-03.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == HERMIT_SANTIAGO {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00215_TrialOfThePilgrim";

    const NPCS: &str = r#"
        <npc id="30036" type="Folk" level="70" name="Priest Petron"/>
        <npc id="30117" type="Folk" level="70" name="Priest Primos"/>
        <npc id="30362" type="Folk" level="70" name="Andellia"/>
        <npc id="30550" type="Folk" level="70" name="Gauri Twinklerock"/>
        <npc id="30571" type="Folk" level="70" name="Seer Tanapi"/>
        <npc id="30612" type="Folk" level="70" name="Elder Casian"/>
        <npc id="30648" type="Folk" level="70" name="Hermit Santiago"/>
        <npc id="30649" type="Folk" level="70" name="Ancestor Martankus"/>
        <npc id="30650" type="Folk" level="70" name="Priest of the Earth Gerald"/>
        <npc id="30651" type="Folk" level="70" name="Wanderer Dorf"/>
        <npc id="30652" type="Folk" level="70" name="Uruha"/>
        <npc id="27116" type="Monster" level="60" name="Lava Salamander"/>
        <npc id="27117" type="Monster" level="60" name="Nahir"/>
        <npc id="27118" type="Monster" level="60" name="Black Willow"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="2721" type="EtcItem" name="Mark of Pilgrim"/>
        <item id="2722" type="EtcItem" name="Book of Sage"><set name="is_questitem" val="true"/></item>
        <item id="2723" type="EtcItem" name="Voucher of Trial"><set name="is_questitem" val="true"/></item>
        <item id="2724" type="EtcItem" name="Spirit of Flame"><set name="is_questitem" val="true"/></item>
        <item id="2725" type="EtcItem" name="Essence of Flame"><set name="is_questitem" val="true"/></item>
        <item id="2726" type="EtcItem" name="Book of Gerald"><set name="is_questitem" val="true"/></item>
        <item id="2727" type="EtcItem" name="Grey Badge"><set name="is_questitem" val="true"/></item>
        <item id="2728" type="EtcItem" name="Picture of Nahir"><set name="is_questitem" val="true"/></item>
        <item id="2729" type="EtcItem" name="Hair of Nahir"><set name="is_questitem" val="true"/></item>
        <item id="2730" type="EtcItem" name="Statue of Einhasad"><set name="is_questitem" val="true"/></item>
        <item id="2731" type="EtcItem" name="Book of Darkness"><set name="is_questitem" val="true"/></item>
        <item id="2732" type="EtcItem" name="Debris of Willow"><set name="is_questitem" val="true"/></item>
        <item id="2733" type="EtcItem" name="Tag of Rumor"><set name="is_questitem" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>"#;

    /// `__compltdStateFlags`: the bookkeeping of skipped conds (Java `QuestState.setCond(cond,
    /// old)`), which every cond write after a skip repeats.
    const SKIP_FLAGS: &str = "__compltdStateFlags";

    fn pages() -> Pages {
        Pages::of(&Q00215TrialOfThePilgrim)
            .page("30648-03.htm", &["ACCEPT"])
            .page("30648-04a.htm", &["30648-05.html"])
            .page("30648-04.htm", &["30648-05.html"])
            .page("30648-05.html", &["30648-06.html"])
            .page("30648-06.html", &["30648-07.html"])
            .page("30648-07.html", &["30648-08.html"])
            .page("30648-08.html", &["30648-05.html"])
            .plain(&["30648-01.html", "30648-02.html", "30648-09.html", "30648-10.html"])
            .plain(&["30649-01.html", "30649-02.html", "30649-04.html"])
            .page("30649-03.html", &["30649-04.html"])
            .page("30650-01.html", &["30650-02.html", "30650-03.html"])
            .plain(&["30650-02.html", "30650-03.html", "30650-04.html"])
            .plain(&["30651-01.html", "30651-02.html", "30651-03.html"])
            .page("30652-01.html", &["30652-02.html"])
            .plain(&["30652-02.html", "30652-03.html"])
            .plain(&["30550-01.html", "30550-02.html"])
            .plain(&["30571-01.html", "30571-02.html", "30571-03.html"])
            .plain(&["30036-01.html", "30036-02.html", "30036-03.html", "30036-04.html"])
            .plain(&["30117-01.html", "30117-02.html"])
            .plain(&[
                "30362-01.html",
                "30362-01a.html",
                "30362-02.html",
                "30362-02a.html",
            ])
            .page("30362-03.html", &["30362-04.html", "30362-05.html"])
            .plain(&["30362-04.html", "30362-05.html", "30362-06.html", "30362-07.html"])
            .plain(&["30612-01.html", "30612-02.html"])
            .npcs(&[
                HERMIT_SANTIAGO,
                PRIEST_PETRON,
                PRIEST_PRIMOS,
                ANDELLIA,
                GAURI_TWINKLEROCK,
                SEER_TANAPI,
                ELDER_CASIAN,
                ANCESTOR_MARTANKUS,
                PRIEST_OF_THE_EARTH_GERALD,
                WANDERER_DORF,
                URUHA,
            ])
    }

    struct Npcs {
        santiago: Oid,
        petron: Oid,
        primos: Oid,
        andellia: Oid,
        gauri: Oid,
        tanapi: Oid,
        casian: Oid,
        martankus: Oid,
        gerald: Oid,
        dorf: Oid,
        uruha: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00215TrialOfThePilgrim);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            santiago: t.spawn(HERMIT_SANTIAGO, 20),
            petron: t.spawn(PRIEST_PETRON, 25),
            primos: t.spawn(PRIEST_PRIMOS, 30),
            andellia: t.spawn(ANDELLIA, 35),
            gauri: t.spawn(GAURI_TWINKLEROCK, 40),
            tanapi: t.spawn(SEER_TANAPI, 45),
            casian: t.spawn(ELDER_CASIAN, 50),
            martankus: t.spawn(ANCESTOR_MARTANKUS, 55),
            gerald: t.spawn(PRIEST_OF_THE_EARTH_GERALD, 60),
            dorf: t.spawn(WANDERER_DORF, 65),
            uruha: t.spawn(URUHA, 70),
        };
        (t, npcs)
    }

    /// A human cleric (a healer) of level 40, with `adena` adena.
    fn enter_healer(t: &mut ScriptTest, adena: i64) {
        let items = if adena > 0 {
            vec![item(100, ADENA, adena)]
        } else {
            vec![]
        };
        t.enter_as(1, 40, 0, 15, items);
    }

    /// Kills a quest monster next to the player; the mob is deleted after the kill.
    fn kill_mob(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 10);
        t.kill(1, mob);
        t.world.delete_npc(mob);
    }

    /// The `character_quests` writes since the last call, without the skip bookkeeping.
    fn rows_of(t: &ScriptTest) -> Vec<QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| match w {
                QuestWrite::InsertVar { var, .. }
                | QuestWrite::UpdateVar { var, .. }
                | QuestWrite::DeleteVar { var, .. } => var != SKIP_FLAGS,
                QuestWrite::DeleteQuest { .. } => true,
            })
            .collect()
    }

    /// Accepts the trial from Hermit Santiago (`ACCEPT`) and drops the writes of the accept.
    fn quest_accept(t: &mut ScriptTest, npcs: &Npcs) {
        assert!(t.quest_page(1, npcs.santiago).contains("30648-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30648-04a.htm"));
        t.take_writes();
    }

    #[test]
    fn a_character_outside_the_healer_group_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, 0, 0, vec![]);
        assert!(t.quest_page(1, npcs.santiago).contains("30648-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_healer_below_level_35_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 34, 0, 15, vec![]);
        assert!(t.quest_page(1, npcs.santiago).contains("30648-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_voucher_and_the_diamonds_once() {
        let (mut t, npcs) = setup();
        enter_healer(&mut t, 0);
        assert!(t.quest_page(1, npcs.santiago).contains("30648-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30648-04a.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(t.count(1, VOUCHER_OF_TRIAL), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 49);
        assert_eq!(
            t.world
                .player(Oid(1))
                .unwrap()
                .data
                .profile
                .variables
                .get(DIAMOND_REWARD_VAR),
            Some(&"1".to_owned())
        );
    }

    #[test]
    fn a_character_who_had_the_diamonds_gets_no_more() {
        let (mut t, npcs) = setup();
        let mut data = crate::testing::player_data(1, 40, vec![]);
        data.class_id = 15;
        data.base_class = 15;
        data.profile
            .variables
            .insert(DIAMOND_REWARD_VAR.to_owned(), "1".to_owned());
        t.enter_with(data);
        t.quest_page(1, npcs.santiago);
        assert!(t.event(1, NAME, "ACCEPT").contains("30648-04.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 0);
    }

    #[test]
    fn the_whole_trial_from_the_voucher_to_the_mark() {
        let (mut t, npcs) = setup();
        enter_healer(&mut t, 100_000);
        quest_accept(&mut t, &npcs);

        // Seer Tanapi takes the voucher (memo 2).
        assert!(t.quest_page(1, npcs.tanapi).contains("30571-01.html"));
        assert_eq!(t.count(1, VOUCHER_OF_TRIAL), 0);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.tanapi).contains("30571-02.html"));

        // Ancestor Martankus (memo 3).
        assert!(t.quest_page(1, npcs.martankus).contains("30649-01.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // A Lava Salamander drops the essence (memo 4).
        kill_mob(&mut t, LAVA_SALAMANDER);
        assert_eq!(t.count(1, ESSENSE_OF_FLAME), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );
        assert!(t.quest_page(1, npcs.martankus).contains("30649-03.html"));
        assert!(t.event(1, NAME, "30649-04.html").contains("30649-04.html"));
        assert_eq!(
            (t.count(1, ESSENSE_OF_FLAME), t.count(1, SPIRIT_OF_FLAME)),
            (0, 1)
        );
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // Gauri takes the spirit and gives the tag (memo 6, cond 7). The cond skips 6, so the
        // skip bookkeeping is written too (5 -> 7: bits 0..=4 and bit 6 set, MSB marks a skip).
        assert!(t.quest_page(1, npcs.gauri).contains("30550-01.html"));
        assert_eq!((t.count(1, SPIRIT_OF_FLAME), t.count(1, TAG_OF_RUMOR)), (0, 1));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "7"),
                rows::insert(NAME, SKIP_FLAGS, "-2147483553"),
            ]
        );

        // Gerald sells the book for 100 000 adena (memo 7).
        assert!(t.quest_page(1, npcs.gerald).contains("30650-01.html"));
        assert!(t.event(1, NAME, "30650-02.html").contains("30650-02.html"));
        assert_eq!((t.count(1, ADENA), t.count(1, BOOK_OF_GERALD)), (0, 1));
        assert_eq!(rows_of(&t), [rows::update(NAME, "memoState", "7")]);

        // Dorf takes the tag for the badge (memo 8).
        assert!(t.quest_page(1, npcs.dorf).contains("30651-02.html"));
        assert_eq!((t.count(1, TAG_OF_RUMOR), t.count(1, GREY_BADGE)), (0, 1));
        assert_eq!(rows_of(&t), [rows::update(NAME, "memoState", "8")]);

        // Gerald pays 100 000 adena for the badge and the book (memo 8 is not 6: the second branch).
        assert!(t.quest_page(1, npcs.gerald).contains("30650-04.html"));
        assert_eq!((t.count(1, ADENA), t.count(1, BOOK_OF_GERALD)), (100_000, 0));
        assert!(rows_of(&t).is_empty());

        // Primos (memo 9), Petron's picture (memo 10), a Nahir drops the hair (memo 11).
        assert!(t.quest_page(1, npcs.primos).contains("30117-01.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "9"),
                rows::update(NAME, "cond", "9")
            ]
        );
        assert!(t.quest_page(1, npcs.petron).contains("30036-01.html"));
        assert_eq!(t.count(1, PICTURE_OF_NAHIR), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "10"),
                rows::update(NAME, "cond", "10")
            ]
        );
        kill_mob(&mut t, NAHIR);
        assert_eq!(t.count(1, HAIR_OF_NAHIR), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "11"),
                rows::update(NAME, "cond", "11")
            ]
        );

        // Petron takes the picture and the hair, gives the statue (memo 12).
        assert!(t.quest_page(1, npcs.petron).contains("30036-03.html"));
        assert_eq!(
            (
                t.count(1, PICTURE_OF_NAHIR),
                t.count(1, HAIR_OF_NAHIR),
                t.count(1, STATUE_OF_EINHASAD)
            ),
            (0, 0, 1)
        );
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "12"),
                rows::update(NAME, "cond", "12")
            ]
        );

        // Andellia (memo 13), a Black Willow drops the debris (memo 14), Uruha takes it for the
        // book of darkness (memo 15).
        assert!(t.quest_page(1, npcs.andellia).contains("30362-01.html"));
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "13"),
                rows::update(NAME, "cond", "13")
            ]
        );
        kill_mob(&mut t, BLACK_WILLOW);
        assert_eq!(t.count(1, DEBRIS_OF_WILLOW), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "14"),
                rows::update(NAME, "cond", "14")
            ]
        );
        assert!(t.quest_page(1, npcs.uruha).contains("30652-01.html"));
        assert!(t.event(1, NAME, "30652-02.html").contains("30652-02.html"));
        assert_eq!(
            (t.count(1, DEBRIS_OF_WILLOW), t.count(1, BOOK_OF_DARKNESS)),
            (0, 1)
        );
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "15"),
                rows::update(NAME, "cond", "15")
            ]
        );

        // Andellia (memo 15) and her choice (memo 16): the book stays with the character.
        assert!(t.quest_page(1, npcs.andellia).contains("30362-03.html"));
        assert!(t.event(1, NAME, "30362-04.html").contains("30362-04.html"));
        assert_eq!(t.count(1, BOOK_OF_DARKNESS), 1);
        assert_eq!(
            rows_of(&t),
            [
                rows::update(NAME, "memoState", "16"),
                rows::update(NAME, "cond", "16")
            ]
        );

        // Elder Casian (memo 17): the book of sage, the badge, the spirit and the statue go,
        // the book of darkness gives 5000 exp and goes.
        assert!(t.quest_page(1, npcs.casian).contains("30612-01.html"));
        assert_eq!(t.count(1, BOOK_OF_SAGE), 1);
        assert_eq!(
            (
                t.count(1, GREY_BADGE),
                t.count(1, SPIRIT_OF_FLAME),
                t.count(1, STATUE_OF_EINHASAD)
            ),
            (0, 0, 0)
        );
        assert_eq!(t.count(1, BOOK_OF_DARKNESS), 0);
        assert_eq!(rows_of(&t), [rows::update(NAME, "memoState", "17")]);
        assert!(t.quest_page(1, npcs.casian).contains("30612-02.html"));
        assert_eq!(rows_of(&t), [rows::update(NAME, "cond", "17")]);

        // Santiago: the adena, the mark, the exp, `exitQuest(false, true)` and the social action.
        t.talk(1, npcs.santiago);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30648-10.html"));
        assert_eq!(t.count(1, MARK_OF_PILGRIM), 1);
        assert_eq!(t.count(1, ADENA), 100_000 + 229_298);
        let names = names_to(&out, 1);
        assert!(names.contains(&"SOCIAL_ACTION".to_owned()));
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.santiago)
                .contains("This quest has already been completed.")
        );
    }
}
