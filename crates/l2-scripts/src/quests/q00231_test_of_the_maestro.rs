//! Quest 231: Test of the Maestro (the Artisan's second class change: Iron Gates Lockirin and the
//! Dwarven Village's Golden Wheels, Silver Scales, Bronze Keys, Gray Pillar, Black Anvils, Master
//! Toma, Chief Croto, Jailer Dubabah and Researcher Lorain).
// Java: dist/game/data/scripts/quests/Q00231_TestOfTheMaestro/Q00231_TestOfTheMaestro.java
//
// Pattern: a long chain on the memo state (`setMemoState`, `isMemoState`) with items collected
// from monsters (`onKill`, ten of each) and a teleport with a timed spawn (`startQuestTimer`).

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const IRON_GATES_LOCKIRIN: i32 = 30531;
const GOLDEN_WHEELS_SPIRON: i32 = 30532;
const SILVER_SCALES_BALANKI: i32 = 30533;
const BRONZE_KEYS_KEEF: i32 = 30534;
const GRAY_PILLAR_MEMBER_FILAUR: i32 = 30535;
const BLACK_ANVILS_ARIN: i32 = 30536;
const MASTER_TOMA: i32 = 30556;
const CHIEF_CROTO: i32 = 30671;
const JAILER_DUBABAH: i32 = 30672;
const RESEARCHER_LORAIN: i32 = 30673;

// Items
const RECOMMENDATION_OF_BALANKI: i32 = 2864;
const RECOMMENDATION_OF_FILAUR: i32 = 2865;
const RECOMMENDATION_OF_ARIN: i32 = 2866;
const LETTER_OF_SOLDER_DERACHMENT: i32 = 2868;
const PAINT_OF_KAMURU: i32 = 2869;
const NECKLACE_OF_KAMUTU: i32 = 2870;
const PAINT_OF_TELEPORT_DEVICE: i32 = 2871;
const TELEPORT_DEVICE: i32 = 2872;
const ARCHITECTURE_OF_CRUMA: i32 = 2873;
const REPORT_OF_CRUMA: i32 = 2874;
const INGREDIENTS_OF_ANTIDOTE: i32 = 2875;
const STINGER_WASP_NEEDLE: i32 = 2876;
const MARSH_SPIDERS_WEB: i32 = 2877;
const BLOOD_OF_LEECH: i32 = 2878;
const BROKEN_TELEPORT_DEVICE: i32 = 2916;

// Reward
const MARK_OF_MAESTRO: i32 = 2867;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const KING_BUGBEAR: i32 = 20150;
const GIANT_MIST_LEECH: i32 = 20225;
const STINGER_WASP: i32 = 20229;
const MARSH_SPIDER: i32 = 20233;

// Quest monster
const EVIL_EYE_LORD: i32 = 27133;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00231_TestOfTheMaestro`.
pub struct Q00231TestOfTheMaestro;

impl Script for Q00231TestOfTheMaestro {
    fn name(&self) -> &'static str {
        "Q00231_TestOfTheMaestro"
    }

    fn path(&self) -> &'static str {
        "quests/Q00231_TestOfTheMaestro"
    }

    fn id(&self) -> i32 {
        231
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[IRON_GATES_LOCKIRIN]);
        s.add_talk_id(&[
            IRON_GATES_LOCKIRIN,
            GOLDEN_WHEELS_SPIRON,
            SILVER_SCALES_BALANKI,
            BRONZE_KEYS_KEEF,
            GRAY_PILLAR_MEMBER_FILAUR,
            BLACK_ANVILS_ARIN,
            MASTER_TOMA,
            CHIEF_CROTO,
            JAILER_DUBABAH,
            RESEARCHER_LORAIN,
        ]);
        s.add_kill_id(&[
            KING_BUGBEAR,
            GIANT_MIST_LEECH,
            STINGER_WASP,
            MARSH_SPIDER,
            EVIL_EYE_LORD,
        ]);
        s.register_quest_items(&[
            RECOMMENDATION_OF_BALANKI,
            RECOMMENDATION_OF_FILAUR,
            RECOMMENDATION_OF_ARIN,
            LETTER_OF_SOLDER_DERACHMENT,
            PAINT_OF_KAMURU,
            NECKLACE_OF_KAMUTU,
            PAINT_OF_TELEPORT_DEVICE,
            TELEPORT_DEVICE,
            ARCHITECTURE_OF_CRUMA,
            REPORT_OF_CRUMA,
            INGREDIENTS_OF_ANTIDOTE,
            STINGER_WASP_NEEDLE,
            MARSH_SPIDERS_WEB,
            BLOOD_OF_LEECH,
            BROKEN_TELEPORT_DEVICE,
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 23);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30531-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30531-04.htm".to_owned());
                    }
                }
            }
            "30533-02.html" => {
                qs.set_memo_state(ctx, 2);
                htmltext = Some(event.to_owned());
            }
            "30556-02.html" | "30556-03.html" | "30556-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "TELEPORT" => {
                if ctx.has_quest_items(player, PAINT_OF_TELEPORT_DEVICE) {
                    ctx.give_items(player, BROKEN_TELEPORT_DEVICE, 1);
                    ctx.take_items(player, PAINT_OF_TELEPORT_DEVICE, 1);
                    // Java `player.teleToLocation(x, y, z)`: no instance change, no random offset.
                    ctx.teleport_player(player, 140352, -194133, -3146, -1, false);
                    ctx.start_quest_timer("SPAWN_KING_BUGBEAR", 5000, npc, Some(player), false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30671-02.html" => {
                ctx.give_items(player, PAINT_OF_KAMURU, 1);
                htmltext = Some(event.to_owned());
            }
            "30673-04.html" => {
                if ctx.has_quest_items(player, INGREDIENTS_OF_ANTIDOTE)
                    && ctx.get_quest_items_count(player, STINGER_WASP_NEEDLE) >= 10
                    && ctx.get_quest_items_count(player, MARSH_SPIDERS_WEB) >= 10
                    && ctx.get_quest_items_count(player, BLOOD_OF_LEECH) >= 10
                {
                    ctx.give_items(player, REPORT_OF_CRUMA, 1);
                    ctx.take_items(player, STINGER_WASP_NEEDLE, -1);
                    ctx.take_items(player, MARSH_SPIDERS_WEB, -1);
                    ctx.take_items(player, BLOOD_OF_LEECH, -1);
                    ctx.take_items(player, INGREDIENTS_OF_ANTIDOTE, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "SPAWN_KING_BUGBEAR" => {
                // Java adds three kings with the same arguments, each set to attack the player.
                for _ in 0..3 {
                    if let Some(mob) =
                        ctx.add_spawn(KING_BUGBEAR, 140395, -194147, -3146, 0, false, 200_000, 0)
                    {
                        ctx.add_attack_desire(mob, player, 999);
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                GIANT_MIST_LEECH => {
                    if qs.is_memo_state(ctx, 4)
                        && ctx.has_quest_items(killer, INGREDIENTS_OF_ANTIDOTE)
                        && ctx.get_quest_items_count(killer, BLOOD_OF_LEECH) < 10
                    {
                        collect_ingredient(ctx, killer, BLOOD_OF_LEECH);
                    }
                }
                STINGER_WASP => {
                    if qs.is_memo_state(ctx, 4)
                        && ctx.has_quest_items(killer, INGREDIENTS_OF_ANTIDOTE)
                        && ctx.get_quest_items_count(killer, STINGER_WASP_NEEDLE) < 10
                    {
                        collect_ingredient(ctx, killer, STINGER_WASP_NEEDLE);
                    }
                }
                MARSH_SPIDER => {
                    if qs.is_memo_state(ctx, 4)
                        && ctx.has_quest_items(killer, INGREDIENTS_OF_ANTIDOTE)
                        && ctx.get_quest_items_count(killer, MARSH_SPIDERS_WEB) < 10
                    {
                        collect_ingredient(ctx, killer, MARSH_SPIDERS_WEB);
                    }
                }
                EVIL_EYE_LORD => {
                    if qs.is_memo_state(ctx, 2)
                        && ctx.has_quest_items(killer, PAINT_OF_KAMURU)
                        && !ctx.has_quest_items(killer, NECKLACE_OF_KAMUTU)
                    {
                        ctx.give_items(killer, NECKLACE_OF_KAMUTU, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
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
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == IRON_GATES_LOCKIRIN {
                htmltext = if ctx.player_class_id(player) == player_class::ARTISAN {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30531-03.htm"
                    } else {
                        "30531-01.html"
                    }
                } else {
                    "30531-02.html"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                IRON_GATES_LOCKIRIN => {
                    let all_recommendations = ctx.has_all_quest_items(
                        player,
                        &[
                            RECOMMENDATION_OF_BALANKI,
                            RECOMMENDATION_OF_FILAUR,
                            RECOMMENDATION_OF_ARIN,
                        ],
                    );
                    if memo_state >= 1 && !all_recommendations {
                        htmltext = "30531-05.html".to_owned();
                    } else if all_recommendations {
                        ctx.give_adena(player, 372_154, true);
                        ctx.give_items(player, MARK_OF_MAESTRO, 1);
                        ctx.add_exp_and_sp(player, 2_085_244, 141_240);
                        qs.exit_quest(ctx, false, true);
                        ctx.world().send(
                            player,
                            &SocialAction {
                                object_id: player.0,
                                action_id: 3,
                            },
                        );
                        htmltext = "30531-06.html".to_owned();
                    }
                }
                GOLDEN_WHEELS_SPIRON => {
                    htmltext = "30532-01.html".to_owned();
                }
                SILVER_SCALES_BALANKI => {
                    if memo_state == 1 && !ctx.has_quest_items(player, RECOMMENDATION_OF_BALANKI) {
                        htmltext = "30533-01.html".to_owned();
                    } else if memo_state == 2 {
                        if !ctx.has_quest_items(player, LETTER_OF_SOLDER_DERACHMENT) {
                            htmltext = "30533-03.html".to_owned();
                        } else {
                            ctx.give_items(player, RECOMMENDATION_OF_BALANKI, 1);
                            ctx.take_items(player, LETTER_OF_SOLDER_DERACHMENT, 1);
                            qs.set_memo_state(ctx, 1);
                            if ctx.has_all_quest_items(
                                player,
                                &[RECOMMENDATION_OF_ARIN, RECOMMENDATION_OF_FILAUR],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30533-04.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, RECOMMENDATION_OF_BALANKI) {
                        htmltext = "30533-05.html".to_owned();
                    }
                }
                BRONZE_KEYS_KEEF => {
                    htmltext = "30534-01.html".to_owned();
                }
                GRAY_PILLAR_MEMBER_FILAUR => {
                    if memo_state == 1 && !ctx.has_quest_items(player, RECOMMENDATION_OF_FILAUR) {
                        ctx.give_items(player, ARCHITECTURE_OF_CRUMA, 1);
                        qs.set_memo_state(ctx, 4);
                        htmltext = "30535-01.html".to_owned();
                    } else if memo_state == 4 {
                        if ctx.has_quest_items(player, ARCHITECTURE_OF_CRUMA)
                            && !ctx.has_quest_items(player, REPORT_OF_CRUMA)
                        {
                            htmltext = "30535-02.html".to_owned();
                        } else if ctx.has_quest_items(player, REPORT_OF_CRUMA)
                            && !ctx.has_quest_items(player, ARCHITECTURE_OF_CRUMA)
                        {
                            ctx.give_items(player, RECOMMENDATION_OF_FILAUR, 1);
                            ctx.take_items(player, REPORT_OF_CRUMA, 1);
                            qs.set_memo_state(ctx, 1);
                            if ctx.has_all_quest_items(
                                player,
                                &[RECOMMENDATION_OF_BALANKI, RECOMMENDATION_OF_ARIN],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30535-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, RECOMMENDATION_OF_FILAUR) {
                        htmltext = "30535-04.html".to_owned();
                    }
                }
                BLACK_ANVILS_ARIN => {
                    if memo_state == 1 && !ctx.has_quest_items(player, RECOMMENDATION_OF_ARIN) {
                        ctx.give_items(player, PAINT_OF_TELEPORT_DEVICE, 1);
                        qs.set_memo_state(ctx, 3);
                        htmltext = "30536-01.html".to_owned();
                    } else if memo_state == 3 {
                        if ctx.has_quest_items(player, PAINT_OF_TELEPORT_DEVICE)
                            && !ctx.has_quest_items(player, TELEPORT_DEVICE)
                        {
                            htmltext = "30536-02.html".to_owned();
                        } else if ctx.get_quest_items_count(player, TELEPORT_DEVICE) >= 5 {
                            ctx.give_items(player, RECOMMENDATION_OF_ARIN, 1);
                            ctx.take_items(player, TELEPORT_DEVICE, -1);
                            qs.set_memo_state(ctx, 1);
                            if ctx.has_all_quest_items(
                                player,
                                &[RECOMMENDATION_OF_BALANKI, RECOMMENDATION_OF_FILAUR],
                            ) {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30536-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, RECOMMENDATION_OF_ARIN) {
                        htmltext = "30536-04.html".to_owned();
                    }
                }
                MASTER_TOMA => {
                    if memo_state == 3 {
                        if ctx.has_quest_items(player, PAINT_OF_TELEPORT_DEVICE) {
                            htmltext = "30556-01.html".to_owned();
                        } else if ctx.has_quest_items(player, BROKEN_TELEPORT_DEVICE) {
                            ctx.give_items(player, TELEPORT_DEVICE, 5);
                            ctx.take_items(player, BROKEN_TELEPORT_DEVICE, 1);
                            htmltext = "30556-06.html".to_owned();
                        } else if ctx.get_quest_items_count(player, TELEPORT_DEVICE) == 5 {
                            htmltext = "30556-07.html".to_owned();
                        }
                    }
                }
                CHIEF_CROTO => {
                    if memo_state == 2
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[PAINT_OF_KAMURU, NECKLACE_OF_KAMUTU, LETTER_OF_SOLDER_DERACHMENT],
                        )
                    {
                        htmltext = "30671-01.html".to_owned();
                    } else if ctx.has_quest_items(player, PAINT_OF_KAMURU)
                        && !ctx.has_quest_items(player, NECKLACE_OF_KAMUTU)
                    {
                        htmltext = "30671-03.html".to_owned();
                    } else if ctx.has_quest_items(player, NECKLACE_OF_KAMUTU) {
                        ctx.give_items(player, LETTER_OF_SOLDER_DERACHMENT, 1);
                        ctx.take_items(player, NECKLACE_OF_KAMUTU, 1);
                        ctx.take_items(player, PAINT_OF_KAMURU, 1);
                        htmltext = "30671-04.html".to_owned();
                    } else if ctx.has_quest_items(player, LETTER_OF_SOLDER_DERACHMENT) {
                        htmltext = "30671-05.html".to_owned();
                    }
                }
                JAILER_DUBABAH => {
                    if ctx.has_quest_items(player, PAINT_OF_KAMURU) {
                        htmltext = "30672-01.html".to_owned();
                    }
                }
                RESEARCHER_LORAIN => {
                    if memo_state == 4 {
                        if ctx.has_quest_items(player, ARCHITECTURE_OF_CRUMA)
                            && !ctx.has_at_least_one_quest_item(
                                player,
                                &[INGREDIENTS_OF_ANTIDOTE, REPORT_OF_CRUMA],
                            )
                        {
                            ctx.give_items(player, INGREDIENTS_OF_ANTIDOTE, 1);
                            ctx.take_items(player, ARCHITECTURE_OF_CRUMA, 1);
                            htmltext = "30673-01.html".to_owned();
                        } else if ctx.has_quest_items(player, INGREDIENTS_OF_ANTIDOTE)
                            && !ctx.has_quest_items(player, REPORT_OF_CRUMA)
                        {
                            if ctx.get_quest_items_count(player, STINGER_WASP_NEEDLE) >= 10
                                && ctx.get_quest_items_count(player, MARSH_SPIDERS_WEB) >= 10
                                && ctx.get_quest_items_count(player, BLOOD_OF_LEECH) >= 10
                            {
                                htmltext = "30673-03.html".to_owned();
                            } else {
                                htmltext = "30673-02.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, REPORT_OF_CRUMA) {
                            htmltext = "30673-05.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == IRON_GATES_LOCKIRIN {
            htmltext = ctx.get_already_completed_msg(player);
        }

        Some(htmltext)
    }
}

/// The kill drops of the Lockirin chain: one more ingredient, the middle sound at ten.
fn collect_ingredient(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32) {
    ctx.give_items(killer, item, 1);
    if ctx.get_quest_items_count(killer, item) >= 10 {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00231_TestOfTheMaestro";

    const NPCS: &str = r#"
        <npc id="30531" type="Folk" level="70" name="Iron Gates Lockirin"/>
        <npc id="30532" type="Folk" level="70" name="Golden Wheels Spiron"/>
        <npc id="30533" type="Folk" level="70" name="Silver Scales Balanki"/>
        <npc id="30534" type="Folk" level="70" name="Bronze Keys Keef"/>
        <npc id="30535" type="Folk" level="70" name="Gray Pillar Member Filaur"/>
        <npc id="30536" type="Folk" level="70" name="Black Anvils Arin"/>
        <npc id="30556" type="Folk" level="70" name="Master Toma"/>
        <npc id="30671" type="Folk" level="70" name="Chief Croto"/>
        <npc id="30672" type="Folk" level="70" name="Jailer Dubabah"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="20150" type="Monster" level="40" name="King Bugbear"/>
        <npc id="20225" type="Monster" level="40" name="Giant Mist Leech"/>
        <npc id="20229" type="Monster" level="40" name="Stinger Wasp"/>
        <npc id="20233" type="Monster" level="40" name="Marsh Spider"/>
        <npc id="27133" type="Monster" level="40" name="Evil Eye Lord"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2864" type="EtcItem" name="Recommendation of Balanki"><set name="is_questitem" val="true"/></item>
        <item id="2865" type="EtcItem" name="Recommendation of Filaur"><set name="is_questitem" val="true"/></item>
        <item id="2866" type="EtcItem" name="Recommendation of Arin"><set name="is_questitem" val="true"/></item>
        <item id="2867" type="EtcItem" name="Mark of Maestro"><set name="is_questitem" val="true"/></item>
        <item id="2868" type="EtcItem" name="Letter of Solder Derachment"><set name="is_questitem" val="true"/></item>
        <item id="2869" type="EtcItem" name="Paint of Kamuru"><set name="is_questitem" val="true"/></item>
        <item id="2870" type="EtcItem" name="Necklace of Kamutu"><set name="is_questitem" val="true"/></item>
        <item id="2871" type="EtcItem" name="Paint of Teleport Device"><set name="is_questitem" val="true"/></item>
        <item id="2872" type="EtcItem" name="Teleport Device"><set name="is_questitem" val="true"/></item>
        <item id="2873" type="EtcItem" name="Architecture of Cruma"><set name="is_questitem" val="true"/></item>
        <item id="2874" type="EtcItem" name="Report of Cruma"><set name="is_questitem" val="true"/></item>
        <item id="2875" type="EtcItem" name="Ingredients of Antidote"><set name="is_questitem" val="true"/></item>
        <item id="2876" type="EtcItem" name="Stinger Wasp Needle"><set name="is_questitem" val="true"/></item>
        <item id="2877" type="EtcItem" name="Marsh Spider's Web"><set name="is_questitem" val="true"/></item>
        <item id="2878" type="EtcItem" name="Blood of Leech"><set name="is_questitem" val="true"/></item>
        <item id="2916" type="EtcItem" name="Broken Teleport Device"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest, with the links the real pages have (`bypass Script <name> <event>`).
    fn pages() -> Pages {
        Pages::of(&Q00231TestOfTheMaestro)
            .page("30531-01.html", &[])
            .page("30531-02.html", &[])
            .page("30531-03.htm", &["ACCEPT"])
            .plain(&["30531-04.htm", "30531-04a.htm", "30531-05.html", "30531-06.html"])
            .plain(&["30532-01.html", "30534-01.html"])
            .page("30533-01.html", &["30533-02.html"])
            .plain(&["30533-02.html", "30533-03.html", "30533-04.html", "30533-05.html"])
            .plain(&["30535-01.html", "30535-02.html", "30535-03.html", "30535-04.html"])
            .plain(&["30536-01.html", "30536-02.html", "30536-03.html", "30536-04.html"])
            .page("30556-01.html", &["30556-02.html", "30556-03.html"])
            .page("30556-02.html", &["30556-04.html"])
            .page("30556-03.html", &[])
            .page("30556-04.html", &["TELEPORT"])
            .plain(&["30556-06.html", "30556-07.html"])
            .page("30671-01.html", &["30671-02.html"])
            .plain(&["30671-02.html", "30671-03.html", "30671-04.html", "30671-05.html"])
            .plain(&["30672-01.html"])
            .page("30673-03.html", &["30673-04.html"])
            .plain(&["30673-01.html", "30673-02.html", "30673-04.html", "30673-05.html"])
            .npcs(&[
                IRON_GATES_LOCKIRIN,
                GOLDEN_WHEELS_SPIRON,
                SILVER_SCALES_BALANKI,
                BRONZE_KEYS_KEEF,
                GRAY_PILLAR_MEMBER_FILAUR,
                BLACK_ANVILS_ARIN,
                MASTER_TOMA,
                CHIEF_CROTO,
                JAILER_DUBABAH,
                RESEARCHER_LORAIN,
            ])
    }

    struct Npcs {
        lockirin: Oid,
        balanki: Oid,
        arin: Oid,
        toma: Oid,
        croto: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00231TestOfTheMaestro);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            lockirin: t.spawn(IRON_GATES_LOCKIRIN, 20),
            balanki: t.spawn(SILVER_SCALES_BALANKI, 30),
            arin: t.spawn(BLACK_ANVILS_ARIN, 50),
            toma: t.spawn(MASTER_TOMA, 60),
            croto: t.spawn(CHIEF_CROTO, 70),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_artisan_of_level_39_is_offered_the_test() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, 0, player_class::ARTISAN, vec![]);
        assert!(t.quest_page(1, npcs.lockirin).contains("30531-01.html"));
        t.enter_as(2, 40, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.lockirin).contains("30531-02.html"));
        t.enter_as(3, 39, 0, player_class::ARTISAN, vec![]);
        assert!(t.quest_page(3, npcs.lockirin).contains("30531-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_diamonds_once() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ARTISAN, vec![]);
        assert!(t.quest_page(1, npcs.lockirin).contains("30531-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30531-04a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 23);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        // `getVariables().set("2ND_CLASS_DIAMOND_REWARD", 1)`: the reward is given once.
        let variables = &t.world.player(Oid(1)).unwrap().data.profile.variables;
        assert_eq!(
            variables.get("2ND_CLASS_DIAMOND_REWARD").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn the_recommendations_are_collected_from_the_chain() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ARTISAN, vec![]);
        t.quest_page(1, npcs.lockirin);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();

        // Balanki wants the letter from Croto first: memo 2 and the kamuru paint.
        assert!(t.quest_page(1, npcs.balanki).contains("30533-01.html"));
        assert!(t.event(1, NAME, "30533-02.html").contains("30533-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "2")]);
        assert!(t.quest_page(1, npcs.balanki).contains("30533-03.html"));

        // Croto gives the paint, the Evil Eye Lord drops the necklace, Croto swaps it.
        assert!(t.quest_page(1, npcs.croto).contains("30671-01.html"));
        assert!(t.event(1, NAME, "30671-02.html").contains("30671-02.html"));
        assert_eq!(t.count(1, PAINT_OF_KAMURU), 1);
        let eye = t.spawn(EVIL_EYE_LORD, 90);
        t.kill(1, eye);
        assert_eq!(t.count(1, NECKLACE_OF_KAMUTU), 1);
        assert!(t.quest_page(1, npcs.croto).contains("30671-04.html"));
        assert_eq!(
            (
                t.count(1, NECKLACE_OF_KAMUTU),
                t.count(1, LETTER_OF_SOLDER_DERACHMENT)
            ),
            (0, 1)
        );
        t.world.delete_npc(eye);

        // Balanki takes the letter: his recommendation, memo 1, no step 2 yet.
        assert!(t.quest_page(1, npcs.balanki).contains("30533-04.html"));
        assert_eq!(t.count(1, RECOMMENDATION_OF_BALANKI), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "memoState", "1")));
    }

    #[test]
    fn a_teleport_spawns_the_kings_and_the_chain_ends_with_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ARTISAN, vec![]);
        t.quest_page(1, npcs.lockirin);
        t.event(1, NAME, "ACCEPT");
        // Filaur and Arin hand out their steps (memo 4 and 3); the teleport device goes to Toma.
        assert!(t.quest_page(1, npcs.arin).contains("30536-01.html"));
        assert_eq!(t.count(1, PAINT_OF_TELEPORT_DEVICE), 1);
        t.take_writes();

        // Toma's page: the event TELEPORT takes the paint and starts the kings after 5 s.
        assert!(t.quest_page(1, npcs.toma).contains("30556-01.html"));
        t.event(1, NAME, "30556-02.html");
        t.event(1, NAME, "30556-04.html");
        // Java `htmltext = event` ("TELEPORT") is no page: `showResult` sends it as chat.
        assert!(t.event(1, NAME, "TELEPORT").is_empty());
        assert_eq!(
            (
                t.count(1, PAINT_OF_TELEPORT_DEVICE),
                t.count(1, BROKEN_TELEPORT_DEVICE)
            ),
            (0, 1)
        );
        assert!(t.npcs_of(KING_BUGBEAR).is_empty());
        t.wait_ms(5_000);
        assert_eq!(t.npcs_of(KING_BUGBEAR).len(), 3);
        for king in t.npcs_of(KING_BUGBEAR) {
            t.world.delete_npc(king);
        }
    }

    #[test]
    fn the_chain_is_finished_by_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 0, player_class::ARTISAN, vec![]);
        t.quest_page(1, npcs.lockirin);
        t.event(1, NAME, "ACCEPT");
        // The three recommendations are given to the player directly (the chain is tested above).
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RECOMMENDATION_OF_BALANKI, 1);
            ctx.give_items(Oid(1), RECOMMENDATION_OF_FILAUR, 1);
            ctx.give_items(Oid(1), RECOMMENDATION_OF_ARIN, 1);
        });
        t.take_writes();
        t.take();
        t.talk(1, npcs.lockirin);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30531-06.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_MAESTRO), 1);
        assert_eq!(t.count(1, 57), 372_154);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.lockirin)
                .contains("This quest has already been completed.")
        );
    }
}
