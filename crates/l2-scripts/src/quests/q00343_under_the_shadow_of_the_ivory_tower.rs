//! Quest 343: Under The Shadow Of The Ivory Tower (Magic Trader Cema, Lich King Icarus and the
//! collectors Marsha and Trumpin; the Nebulite Orbs of the Ivory Tower monsters).
// Java: dist/game/data/scripts/quests/Q00343_UnderTheShadowOfTheIvoryTower/Q00343_UnderTheShadowOfTheIvoryTower.java
//
// Pattern: a repeatable quest of orbs from kills (a drop chance, a memo state ex that counts down
// with the kills); the collectors trade orbs for rewards by their own script variables
// (`param1`, `param2`, read and set with `qs.set`). The branches are kept as Java has them.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAGIC_TRADER_CEMA: i32 = 30834;
const LICH_KING_ICARUS: i32 = 30835;
const COLLECTOR_MARSHA: i32 = 30934;
const COLLECTOR_TRUMPIN: i32 = 30935;

// Item
const NEBULITE_ORB: i32 = 4364;

// Rewards
const TOWER_SHIELD: i32 = 103;
const NICKLACE_OF_MAGIC: i32 = 118;
const SAGES_BLOOD: i32 = 316;
const SQUARE_SHIELD: i32 = 630;
const SCROLL_OF_ESCAPE: i32 = 736;
const RING_OF_AGES: i32 = 885;
const NICKLACE_OF_MERMAID: i32 = 917;
const SCROLL_ENCHANT_WEAPON_C_GRADE: i32 = 951;
const SCROLL_ENCHANT_WEAPON_D_GRADE: i32 = 955;
const SPIRITSHOT_D_GRADE: i32 = 2510;
const SPIRITSHOT_C_GRADE: i32 = 2511;
const ECTOPLASM_LIQUEUR: i32 = 4365;

// Monsters
const MANASHEN_GARGOYLE: i32 = 20563;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const ENCHANTED_STONE_GOLEM: i32 = 20565;
const ENCHANTED_IRON_GOLEM: i32 = 20566;

// Misc
const MIN_LEVEL: i32 = 40;

/// Java `Q00343_UnderTheShadowOfTheIvoryTower`.
pub struct Q00343UnderTheShadowOfTheIvoryTower;

impl Script for Q00343UnderTheShadowOfTheIvoryTower {
    fn name(&self) -> &'static str {
        "Q00343_UnderTheShadowOfTheIvoryTower"
    }

    fn path(&self) -> &'static str {
        "quests/Q00343_UnderTheShadowOfTheIvoryTower"
    }

    fn id(&self) -> i32 {
        343
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAGIC_TRADER_CEMA]);
        s.add_talk_id(&[
            MAGIC_TRADER_CEMA,
            LICH_KING_ICARUS,
            COLLECTOR_MARSHA,
            COLLECTOR_TRUMPIN,
        ]);
        s.add_kill_id(&[
            MANASHEN_GARGOYLE,
            ENCHANTED_MONSTEREYE,
            ENCHANTED_STONE_GOLEM,
            ENCHANTED_IRON_GOLEM,
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
            "30834-05.htm" => {
                if qs.is_created(ctx) {
                    qs.set_memo_state(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30834-04.htm" => {
                if ctx.world().is_in_category(player, CategoryType::WizardGroup)
                    && ctx.player_level(player) >= MIN_LEVEL
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "30834-08.html" => {
                if ctx.has_quest_items(player, NEBULITE_ORB) {
                    let count = ctx.get_quest_items_count(player, NEBULITE_ORB);
                    ctx.give_adena(player, count * 120, true);
                    ctx.take_items(player, NEBULITE_ORB, -1);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30834-08a.html".to_owned());
                }
            }
            "30834-11.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30835-02.html" => {
                if !ctx.has_quest_items(player, ECTOPLASM_LIQUEUR) {
                    htmltext = Some(event.to_owned());
                } else {
                    let chance = ctx.get_random(1000);
                    if chance <= 119 {
                        ctx.give_items(player, SCROLL_ENCHANT_WEAPON_D_GRADE, 1);
                    } else if chance <= 169 {
                        ctx.give_items(player, SCROLL_ENCHANT_WEAPON_C_GRADE, 1);
                    } else if chance <= 329 {
                        let count = i64::from(ctx.get_random(200) + 401);
                        ctx.give_items(player, SPIRITSHOT_C_GRADE, count);
                    } else if chance <= 559 {
                        let count = i64::from(ctx.get_random(200) + 401);
                        ctx.give_items(player, SPIRITSHOT_D_GRADE, count);
                    } else if chance <= 561 {
                        ctx.give_items(player, SAGES_BLOOD, 1);
                    } else if chance <= 578 {
                        ctx.give_items(player, SQUARE_SHIELD, 1);
                    } else if chance <= 579 {
                        ctx.give_items(player, NICKLACE_OF_MAGIC, 1);
                    } else if chance <= 581 {
                        ctx.give_items(player, RING_OF_AGES, 1);
                    } else if chance <= 582 {
                        ctx.give_items(player, TOWER_SHIELD, 1);
                    } else if chance <= 584 {
                        ctx.give_items(player, NICKLACE_OF_MERMAID, 1);
                    } else {
                        ctx.give_items(player, SCROLL_OF_ESCAPE, 1);
                    }

                    ctx.take_items(player, ECTOPLASM_LIQUEUR, 1);
                    htmltext = Some("30835-03.html".to_owned());
                }
            }
            "30934-05.html" => {
                if qs.is_memo_state(ctx, 1) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                    let orbs = ctx.get_quest_items_count(player, NEBULITE_ORB);
                    if memo_state_ex >= 25 {
                        htmltext = Some(event.to_owned());
                    } else if (1..25).contains(&memo_state_ex) && orbs < 10 {
                        htmltext = Some("30934-06.html".to_owned());
                    } else if (1..25).contains(&memo_state_ex) && orbs > 10 {
                        // NOTE(java-quirk): exactly 10 orbs match neither branch (`> 10`, not `>= 10`),
                        // so Marsha shows no page at all.
                        qs.set_memo_state(ctx, 2);
                        ctx.take_items(player, NEBULITE_ORB, 10);
                        htmltext = Some("30934-07.html".to_owned());
                    }
                }
            }
            "30934-08a.html" => {
                if qs.is_memo_state(ctx, 2) {
                    let i0 = ctx.get_random(100);
                    let i1 = ctx.get_random(3);
                    if i0 < 20 && i1 == 0 {
                        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 4);
                        qs.set_int(ctx, "param1", 0);
                        htmltext = Some(event.to_owned());
                    } else if i0 < 20 && i1 == 1 {
                        qs.set_int(ctx, "param1", 1);
                        htmltext = Some("30934-08b.html".to_owned());
                    } else if i0 < 20 && i1 == 2 {
                        qs.set_int(ctx, "param1", 2);
                        htmltext = Some("30934-08c.html".to_owned());
                    } else if (20..50).contains(&i0) && i1 == 0 {
                        let param1 = if ctx.get_random(2) == 0 { 0 } else { 1 };
                        qs.set_int(ctx, "param1", param1);
                        htmltext = Some("30934-09a.html".to_owned());
                    } else if (20..50).contains(&i0) && i1 == 1 {
                        let param1 = if ctx.get_random(2) == 0 { 1 } else { 2 };
                        qs.set_int(ctx, "param1", param1);
                        htmltext = Some("30934-09b.html".to_owned());
                    } else if (20..50).contains(&i0) && i1 == 2 {
                        let param1 = if ctx.get_random(2) == 0 { 2 } else { 0 };
                        qs.set_int(ctx, "param1", param1);
                        htmltext = Some("30934-09c.html".to_owned());
                    } else {
                        let param1 = ctx.get_random(3);
                        qs.set_int(ctx, "param1", param1);
                        htmltext = Some("30934-10.html".to_owned());
                    }
                }
            }
            "30934-11a.html" => {
                if qs.is_memo_state(ctx, 2) {
                    if qs.get_int(ctx, "param1") == 0 {
                        ctx.give_items(player, NEBULITE_ORB, 10);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some(event.to_owned());
                    } else if qs.get_int(ctx, "param1") == 1 {
                        htmltext = Some("30934-11b.html".to_owned());
                    } else if qs.get_int(ctx, "param1") == 2 {
                        ctx.give_items(player, NEBULITE_ORB, 20);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some("30934-11c.html".to_owned());
                    }

                    qs.set_memo_state(ctx, 1);
                }
            }
            "30934-12a.html" => {
                if qs.is_memo_state(ctx, 2) {
                    if qs.get_int(ctx, "param1") == 0 {
                        ctx.give_items(player, NEBULITE_ORB, 20);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some(event.to_owned());
                    } else if qs.get_int(ctx, "param1") == 1 {
                        ctx.give_items(player, NEBULITE_ORB, 10);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some("30934-12b.html".to_owned());
                    } else if qs.get_int(ctx, "param1") == 2 {
                        htmltext = Some("30934-12c.html".to_owned());
                    }

                    qs.set_memo_state(ctx, 1);
                }
            }
            "30934-13a.html" => {
                if qs.is_memo_state(ctx, 2) {
                    if qs.get_int(ctx, "param1") == 0 {
                        htmltext = Some(event.to_owned());
                    } else if qs.get_int(ctx, "param1") == 1 {
                        ctx.give_items(player, NEBULITE_ORB, 20);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some("30934-13b.html".to_owned());
                    } else if qs.get_int(ctx, "param1") == 2 {
                        ctx.give_items(player, NEBULITE_ORB, 10);
                        qs.set_int(ctx, "param1", 4);
                        htmltext = Some("30934-13c.html".to_owned());
                    }

                    qs.set_memo_state(ctx, 1);
                }
            }
            "30935-03.html" => {
                if ctx.get_quest_items_count(player, NEBULITE_ORB) < 10 {
                    htmltext = Some(event.to_owned());
                } else {
                    let param2 = ctx.get_random(2);
                    qs.set_int(ctx, "param2", param2);
                    htmltext = Some("30935-04.html".to_owned());
                }
            }
            "30935-05.html" => {
                let orbs = ctx.get_quest_items_count(player, NEBULITE_ORB);
                let param1 = qs.get_int(ctx, "param1");
                let param2 = qs.get_int(ctx, "param2");
                if param1 == 0 && param2 == 0 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 1);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some(event.to_owned());
                } else if param1 == 1 && param2 == 0 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 2);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-05a.html".to_owned());
                } else if param1 == 2 && param2 == 0 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 3);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-05b.html".to_owned());
                } else if param1 == 3 && param2 == 0 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 4);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-05c.html".to_owned());
                } else if param1 == 4 && param2 == 0 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 310);
                    htmltext = Some("30935-05d.html".to_owned());
                } else if param2 == 1 && orbs >= 10 {
                    ctx.take_items(player, NEBULITE_ORB, 10);
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-06.html".to_owned());
                } else if orbs < 10 {
                    htmltext = Some("30935-03.html".to_owned());
                }
            }
            "30935-07.html" => {
                let orbs = ctx.get_quest_items_count(player, NEBULITE_ORB);
                let param1 = qs.get_int(ctx, "param1");
                let param2 = qs.get_int(ctx, "param2");
                if param2 == 0 && orbs >= 10 {
                    ctx.take_items(player, NEBULITE_ORB, 10);
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some(event.to_owned());
                } else if param1 == 0 && param2 == 1 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 1);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-08.html".to_owned());
                } else if param1 == 1 && param2 == 1 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 2);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-08a.html".to_owned());
                } else if param1 == 2 && param2 == 1 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 3);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-08b.html".to_owned());
                } else if param1 == 3 && param2 == 1 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 4);
                    qs.set_int(ctx, "param2", 2);
                    htmltext = Some("30935-08c.html".to_owned());
                } else if param1 == 4 && param2 == 1 && orbs >= 10 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 310);
                    htmltext = Some("30935-08d.html".to_owned());
                } else if orbs < 10 {
                    htmltext = Some("30935-03.html".to_owned());
                }
            }
            "30935-09.html" => {
                let param1 = qs.get_int(ctx, "param1");
                if param1 == 1 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 10);
                    htmltext = Some(event.to_owned());
                } else if param1 == 2 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 30);
                    htmltext = Some("30935-09a.html".to_owned());
                } else if param1 == 3 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 70);
                    htmltext = Some("30935-09b.html".to_owned());
                } else if param1 == 4 {
                    qs.set_int(ctx, "param1", 0);
                    qs.set_int(ctx, "param2", 2);
                    ctx.give_items(player, NEBULITE_ORB, 150);
                    htmltext = Some("30935-09c.html".to_owned());
                }
            }
            "30935-10.html" => {
                qs.set_int(ctx, "param1", 0);
                qs.set_int(ctx, "param2", 2);
                htmltext = Some(event.to_owned());
            }
            "30834-04a.html" | "30834-06a.html" | "30834-09a.html" | "30834-09b.html" | "30834-11a.html"
            | "30834-09.html" | "30834-10.html" | "30835-04.html" | "30835-05.html" | "30934-03a.html"
            | "30935-01.html" | "30935-01a.html" | "30935-01b.html" => {
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
            // The orb chance of each monster, and the chance that its kill counts the memo down.
            let (orb_chance, countdown_chance) = match npc.id {
                MANASHEN_GARGOYLE | ENCHANTED_MONSTEREYE => (63, 12),
                ENCHANTED_STONE_GOLEM => (65, 12),
                ENCHANTED_IRON_GOLEM => (68, 13),
                _ => return,
            };
            if ctx.get_random(100) < orb_chance {
                ctx.give_items(killer, NEBULITE_ORB, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }

            if qs.get_memo_state_ex(ctx, 1) > 1 && ctx.get_random(100) <= countdown_chance {
                qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) - 1);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MAGIC_TRADER_CEMA {
                htmltext = if ctx.world().is_in_category(player, CategoryType::WizardGroup) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30834-03.htm"
                    } else {
                        "30834-02.htm"
                    }
                } else {
                    "30834-01.htm"
                }
                .to_owned();
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                MAGIC_TRADER_CEMA => {
                    htmltext = if !ctx.has_quest_items(player, NEBULITE_ORB) {
                        "30834-06.html"
                    } else {
                        "30834-07.html"
                    }
                    .to_owned();
                }
                LICH_KING_ICARUS => htmltext = "30835-01.html".to_owned(),
                COLLECTOR_MARSHA => {
                    if qs.get_memo_state_ex(ctx, 1) == 0 {
                        qs.set_memo_state_ex(ctx, 1, 1);
                        htmltext = "30934-03.html".to_owned();
                    } else {
                        qs.set_memo_state(ctx, 1);
                        htmltext = "30934-04.html".to_owned();
                    }
                }
                COLLECTOR_TRUMPIN => {
                    qs.set_int(ctx, "param1", 0);
                    htmltext = "30935-01.html".to_owned();
                }
                _ => {}
            }
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

    const NAME: &str = "Q00343_UnderTheShadowOfTheIvoryTower";

    const NPCS: &str = r#"
        <npc id="30834" type="Folk" level="40" name="Magic Trader Cema"/>
        <npc id="30835" type="Folk" level="40" name="Lich King Icarus"/>
        <npc id="30934" type="Folk" level="40" name="Collector Marsha"/>
        <npc id="30935" type="Folk" level="40" name="Collector Trumpin"/>
        <npc id="20563" type="Monster" level="40" name="Manashen Gargoyle"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4364" type="EtcItem" name="Nebulite Orb"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4365" type="EtcItem" name="Ectoplasm Liqueur"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="951" type="EtcItem" name="Scroll C"><set name="is_stackable" val="true"/></item>
        <item id="955" type="EtcItem" name="Scroll D"><set name="is_stackable" val="true"/></item>
        <item id="2510" type="EtcItem" name="Spiritshot D"><set name="is_stackable" val="true"/></item>
        <item id="2511" type="EtcItem" name="Spiritshot C"><set name="is_stackable" val="true"/></item>
        <item id="316" type="EtcItem" name="Sage's Blood"/>
        <item id="630" type="EtcItem" name="Square Shield"/>
        <item id="736" type="EtcItem" name="Scroll of Escape"/>
        <item id="885" type="EtcItem" name="Ring of Ages"/>
        <item id="917" type="EtcItem" name="Nicklace of Mermaid"/>
        <item id="118" type="EtcItem" name="Nicklace of Magic"/>
        <item id="103" type="EtcItem" name="Tower Shield"/>"#;

    /// A human mystic (class 10, `WIZARD_GROUP`).
    const MYSTIC: i32 = 10;

    /// The reward steps of Marsha's sale: every sale page links to them.
    const REWARD_LINKS: [&str; 3] = ["30934-11a.html", "30934-12a.html", "30934-13a.html"];

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00343UnderTheShadowOfTheIvoryTower);
        let pages = Pages::of(&Q00343UnderTheShadowOfTheIvoryTower)
            .page("30834-03.htm", &["30834-04.htm"])
            .plain(&["30834-01.htm", "30834-02.htm"])
            .page("30834-04.htm", &["30834-05.htm", "30834-04a.html"])
            .plain(&["30834-04a.html", "30834-05.htm"])
            .page("30834-06.html", &["30834-06a.html"])
            .plain(&["30834-06a.html"])
            .page(
                "30834-07.html",
                &[
                    "30834-09a.html",
                    "30834-09b.html",
                    "30834-08.html",
                    "30834-09.html",
                    "30834-11a.html",
                    "30834-10.html",
                ],
            )
            .page("30834-08.html", &["30834-11a.html", "30834-10.html"])
            .plain(&["30834-08a.html"])
            .page("30834-09.html", &["30834-09a.html", "30834-09b.html"])
            .plain(&["30834-09a.html", "30834-09b.html"])
            .page("30834-10.html", &["30834-11a.html", "30834-11.html"])
            .plain(&["30834-11.html", "30834-11a.html"])
            .page("30835-01.html", &["30835-02.html"])
            .plain(&["30835-02.html", "30835-03.html"])
            .page("30934-03.html", &["30934-05.html", "30934-03a.html"])
            .page("30934-04.html", &["30934-05.html", "30934-03a.html"])
            .plain(&["30934-03a.html", "30934-06.html"])
            .page("30934-07.html", &["30934-08a.html"])
            .page("30934-08a.html", &REWARD_LINKS)
            .page("30934-11a.html", &["30934-05.html", "30934-03a.html"])
            .page("30934-05.html", &[])
            .page("30934-08b.html", &REWARD_LINKS)
            .page("30934-08c.html", &REWARD_LINKS)
            .page("30934-09a.html", &REWARD_LINKS)
            .page("30934-09b.html", &REWARD_LINKS)
            .page("30934-09c.html", &REWARD_LINKS)
            .page("30934-10.html", &REWARD_LINKS)
            .plain(&[
                "30934-11b.html",
                "30934-11c.html",
                "30934-12b.html",
                "30934-12c.html",
                "30934-13b.html",
                "30934-13c.html",
            ])
            .page(
                "30935-01.html",
                &["30935-03.html", "30935-01a.html", "30935-01b.html"],
            )
            .plain(&["30935-01a.html", "30935-01b.html", "30935-03.html"])
            .page("30935-04.html", &["30935-05.html", "30935-07.html"])
            .page("30935-05.html", &["30935-09.html", "30935-03.html"])
            .plain(&["30935-06.html", "30935-07.html", "30935-09.html", "30935-10.html"])
            .npcs(&[
                MAGIC_TRADER_CEMA,
                LICH_KING_ICARUS,
                COLLECTOR_MARSHA,
                COLLECTOR_TRUMPIN,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let cema = t.spawn(MAGIC_TRADER_CEMA, 20);
        let icarus = t.spawn(LICH_KING_ICARUS, 40);
        let marsha = t.spawn(COLLECTOR_MARSHA, 60);
        let trumpin = t.spawn(COLLECTOR_TRUMPIN, 80);
        (t, cema, icarus, marsha, trumpin)
    }

    /// Cema's accept for a level 40 wizard: the memo states and the quest started.
    fn start(t: &mut ScriptTest, cema: Oid) {
        assert!(t.quest_page(1, cema).contains("30834-03.htm"));
        assert!(t.event(1, NAME, "30834-04.htm").contains("30834-04.htm"));
        assert!(t.event(1, NAME, "30834-05.htm").contains("30834-05.htm"));
    }

    fn orbs(t: &ScriptTest) -> i64 {
        t.count(1, NEBULITE_ORB)
    }

    fn param1_of(t: &mut ScriptTest) -> i32 {
        let mut value = -1;
        t.world.run_as_script(NAME, |ctx| {
            value = ctx.get_quest_state(Oid(1), false).unwrap().get_int(ctx, "param1");
        });
        value
    }

    #[test]
    fn a_fighter_is_not_a_wizard_and_a_low_level_wizard_is_sent_away() {
        let (mut t, cema, ..) = setup();
        t.enter_as(1, 40, 0, 0, vec![]);
        assert!(t.quest_page(1, cema).contains("30834-01.htm"));
        t.enter_as(2, 39, 0, MYSTIC, vec![]);
        assert!(t.quest_page(2, cema).contains("30834-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn cema_starts_the_quest_for_a_wizard_of_level_forty() {
        let (mut t, cema, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![]);
        assert!(t.quest_page(1, cema).contains("30834-03.htm"));
        assert!(t.event(1, NAME, "30834-04.htm").contains("30834-04.htm"));
        t.take_writes();
        assert!(t.event(1, NAME, "30834-05.htm").contains("30834-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "memoStateEx1", "0"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, cema).contains("30834-06.html"));
    }

    #[test]
    fn the_orbs_are_sold_to_cema_for_120_adena_each_and_the_quest_ends() {
        let (mut t, cema, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 3)]);
        start(&mut t, cema);
        t.take_writes();
        assert!(t.quest_page(1, cema).contains("30834-07.html"));
        assert!(t.event(1, NAME, "30834-08.html").contains("30834-08.html"));
        assert_eq!(orbs(&t), 0);
        assert_eq!(t.count(1, 57), 360);
        assert!(t.event(1, NAME, "30834-10.html").contains("30834-10.html"));
        assert!(t.event(1, NAME, "30834-11.html").contains("30834-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn cema_refuses_the_sale_without_orbs() {
        let (mut t, cema, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 3)]);
        start(&mut t, cema);
        assert!(t.quest_page(1, cema).contains("30834-07.html"));
        // The orbs go elsewhere while the page is open: the sale finds none.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), NEBULITE_ORB, -1);
        });
        assert!(t.event(1, NAME, "30834-08.html").contains("30834-08a.html"));
        assert_eq!(t.count(1, 57), 0);
    }

    #[test]
    fn icarus_gives_one_reward_for_an_ectoplasm_liqueur() {
        let (mut t, cema, icarus, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, ECTOPLASM_LIQUEUR, 1)]);
        start(&mut t, cema);
        t.take_writes();
        assert!(t.quest_page(1, icarus).contains("30835-01.html"));
        assert!(t.event(1, NAME, "30835-02.html").contains("30835-03.html"));
        assert_eq!(t.count(1, ECTOPLASM_LIQUEUR), 0);
        let rewards: i64 = [
            SCROLL_ENCHANT_WEAPON_D_GRADE,
            SCROLL_ENCHANT_WEAPON_C_GRADE,
            SPIRITSHOT_C_GRADE,
            SPIRITSHOT_D_GRADE,
            SAGES_BLOOD,
            SQUARE_SHIELD,
            NICKLACE_OF_MAGIC,
            RING_OF_AGES,
            TOWER_SHIELD,
            NICKLACE_OF_MERMAID,
            SCROLL_OF_ESCAPE,
        ]
        .iter()
        .map(|id| t.count(1, *id))
        .sum();
        assert!(rewards >= 1);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn icarus_without_liqueur_only_talks() {
        let (mut t, cema, icarus, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![]);
        start(&mut t, cema);
        assert!(t.quest_page(1, icarus).contains("30835-01.html"));
        assert!(t.event(1, NAME, "30835-02.html").contains("30835-02.html"));
    }

    #[test]
    fn marsha_counts_the_memo_and_then_takes_the_orbs_above_ten() {
        let (mut t, cema, _icarus, marsha, _trumpin) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 11)]);
        start(&mut t, cema);
        t.take_writes();
        // The first visit sets memoStateEx 1 to 1; the second one sets memo 1.
        assert!(t.quest_page(1, marsha).contains("30934-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoStateEx1", "1")]);
        assert!(t.quest_page(1, marsha).contains("30934-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "1")]);
        // More than ten orbs: ten go, memo 2, the sell page.
        assert!(t.event(1, NAME, "30934-05.html").contains("30934-07.html"));
        assert_eq!(orbs(&t), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "2")]);
    }

    #[test]
    fn exactly_ten_orbs_get_no_page_from_marsha() {
        // NOTE(java-quirk): `orbs > 10` in Java, so ten orbs match no branch of `30934-05.html`.
        let (mut t, cema, _icarus, marsha, _trumpin) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 10)]);
        start(&mut t, cema);
        assert!(t.quest_page(1, marsha).contains("30934-03.html"));
        assert!(t.quest_page(1, marsha).contains("30934-04.html"));
        assert_eq!(t.event(1, NAME, "30934-05.html"), "");
        assert_eq!(orbs(&t), 10);
    }

    #[test]
    fn marsha_pays_the_reward_the_roll_of_the_sale_chose() {
        let (mut t, cema, _icarus, marsha, _trumpin) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 11)]);
        start(&mut t, cema);
        t.quest_page(1, marsha);
        t.quest_page(1, marsha);
        t.event(1, NAME, "30934-05.html");
        // The sell step rolls `param1` (0, 1 or 2 for the reward steps below); its page links to
        // the three reward steps.
        t.event(1, NAME, "30934-08a.html");
        let param1 = param1_of(&mut t);
        assert!((0..=2).contains(&param1));
        let orbs_before = orbs(&t);
        t.take_writes();
        let (page, gained) = match param1 {
            0 => ("30934-11a.html", 10),
            1 => ("30934-11b.html", 0),
            _ => ("30934-11c.html", 20),
        };
        assert!(
            t.event(1, NAME, "30934-11a.html").contains(page),
            "param1 {param1}"
        );
        assert_eq!(orbs(&t), orbs_before + gained);
        assert_eq!(
            t.take_writes().last(),
            Some(&rows::update(NAME, "memoState", "1"))
        );
    }

    #[test]
    fn trumpin_hands_out_the_orbs_of_the_two_step_exchange() {
        let (mut t, cema, _icarus, _marsha, trumpin) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 10)]);
        start(&mut t, cema);
        t.take_writes();
        assert!(t.quest_page(1, trumpin).contains("30935-01.html"));
        assert!(t.event(1, NAME, "30935-03.html").contains("30935-04.html"));
        // Fix the random second roll: param2 0 leads to the first chain.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(ctx, "param2", 0);
        });
        assert!(t.event(1, NAME, "30935-05.html").contains("30935-05.html"));
        // 09 pays the ten orbs of the first step.
        assert!(t.event(1, NAME, "30935-09.html").contains("30935-09.html"));
        assert_eq!(orbs(&t), 20);
    }

    #[test]
    fn trumpin_asks_for_ten_orbs_below_ten() {
        let (mut t, cema, _icarus, _marsha, trumpin) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![item(100, NEBULITE_ORB, 4)]);
        start(&mut t, cema);
        assert!(t.quest_page(1, trumpin).contains("30935-01.html"));
        assert!(t.event(1, NAME, "30935-03.html").contains("30935-03.html"));
        assert_eq!(orbs(&t), 4);
    }

    #[test]
    fn the_monsters_drop_orbs_while_the_quest_runs() {
        let (mut t, cema, ..) = setup();
        t.enter_as(1, 40, 0, MYSTIC, vec![]);
        start(&mut t, cema);
        for _ in 0..200 {
            if orbs(&t) >= 1 {
                break;
            }
            let gargoyle = t.spawn(MANASHEN_GARGOYLE, 60);
            t.kill(1, gargoyle);
        }
        assert!(orbs(&t) >= 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
    }
}
