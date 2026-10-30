//! Quest 10275: Containing the Attribute Power (Holly/Weber send the adventurer to Yin and Yang with an attribute sword).
// Java: dist/game/data/scripts/quests/Q10275_ContainingTheAttributePower/Q10275_ContainingTheAttributePower.java
//
// Pattern: a kill quest that needs an item equipped (`getItemEquipped`, see Q00104), with a
// reward chosen by a numeric event (the attribute-crystal page links `1`..`6`).

use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const HOLLY: i32 = 30839;
const WEBER: i32 = 31307;
const YIN: i32 = 32325;
const YANG: i32 = 32326;
const WATER: i32 = 27380;
const AIR: i32 = 27381;

// Items
const YINSWORD: i32 = 13845;
const YANGSWORD: i32 = 13881;
const SOULPIECEWATER: i32 = 13861;
const SOULPIECEAIR: i32 = 13862;

// Skills (Java `SkillHolder(id, level)`)
const BLESSING_OF_FIRE: (i32, i32) = (2635, 1);
const BLESSING_OF_EARTH: (i32, i32) = (2636, 1);

// Java `Elementals.FIRE`, `Elementals.EARTH`
const FIRE: i32 = 0;
const EARTH: i32 = 3;

/// Java `getItemEquipped(player, slot)`: the item id in a paperdoll slot, 0 when empty.
fn item_equipped(ctx: &ScriptCtx<'_>, player: ObjectId, slot: usize) -> i32 {
    let Some(object_id) = ctx.world().paperdoll_object(player, slot) else {
        return 0;
    };
    ctx.world()
        .player(player)
        .and_then(|p| p.data.items.iter().find(|i| i.object_id == object_id))
        .map_or(0, |i| i.item_id)
}

/// Java `Q10275_ContainingTheAttributePower`.
pub struct Q10275ContainingTheAttributePower;

impl Script for Q10275ContainingTheAttributePower {
    fn name(&self) -> &'static str {
        "Q10275_ContainingTheAttributePower"
    }

    fn path(&self) -> &'static str {
        "quests/Q10275_ContainingTheAttributePower"
    }

    fn id(&self) -> i32 {
        10275
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HOLLY, WEBER]);
        s.add_talk_id(&[HOLLY, WEBER, YIN, YANG]);
        s.add_kill_id(&[AIR, WATER]);
        s.register_quest_items(&[YINSWORD, YANGSWORD, SOULPIECEWATER, SOULPIECEAIR]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = Some(event.to_owned());
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "30839-02.html" | "31307-02.html" => qs.start_quest(ctx),
            "30839-05.html" => qs.set_cond(ctx, 2, true),
            "31307-05.html" => qs.set_cond(ctx, 7, true),
            "32325-03.html" => {
                qs.set_cond(ctx, 3, true);
                ctx.give_items_elemental(player, YINSWORD, 1, FIRE, 10);
            }
            "32326-03.html" => {
                qs.set_cond(ctx, 8, true);
                ctx.give_items_elemental(player, YANGSWORD, 1, EARTH, 10);
            }
            "32325-06.html" => {
                if ctx.has_quest_items(player, YINSWORD) {
                    ctx.take_items(player, YINSWORD, 1);
                    htmltext = Some("32325-07.html".to_owned());
                }
                ctx.give_items_elemental(player, YINSWORD, 1, FIRE, 10);
            }
            "32326-06.html" => {
                if ctx.has_quest_items(player, YANGSWORD) {
                    ctx.take_items(player, YANGSWORD, 1);
                    htmltext = Some("32326-07.html".to_owned());
                }
                ctx.give_items_elemental(player, YANGSWORD, 1, EARTH, 10);
            }
            "32325-09.html" => {
                qs.set_cond(ctx, 5, true);
                ctx.world_mut().apply_effects(
                    player,
                    player,
                    SkillKey::new(BLESSING_OF_FIRE.0, BLESSING_OF_FIRE.1),
                );
                ctx.give_items_elemental(player, YINSWORD, 1, FIRE, 10);
            }
            "32326-09.html" => {
                qs.set_cond(ctx, 10, true);
                ctx.world_mut().apply_effects(
                    player,
                    player,
                    SkillKey::new(BLESSING_OF_EARTH.0, BLESSING_OF_EARTH.1),
                );
                ctx.give_items_elemental(player, YANGSWORD, 1, EARTH, 10);
            }
            _ => {}
        }

        // Java `StringUtil.isNumeric(event)`: the crystal choice page.
        if !event.is_empty() && event.bytes().all(|b| b.is_ascii_digit()) {
            // NOTE(java-quirk): `npc` is not null for a bypass; a timer-less event cannot get here.
            let npc_id = npc.map_or(0, |n| n.id);
            let choice: i32 = event.parse().unwrap_or(0);
            htmltext = Some(format!("{npc_id}-1{event}.html"));
            ctx.give_items(player, 10520 + choice, 2);
            ctx.add_exp_and_sp(player, 202_160, 20_375);
            qs.exit_quest(ctx, false, true);
        }

        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        match npc.id {
            AIR => {
                if (qs.is_cond(ctx, 8) || qs.is_cond(ctx, 10))
                    && item_equipped(ctx, killer, paperdoll::RHAND) == YANGSWORD
                    && ctx.get_quest_items_count(killer, SOULPIECEAIR) < 6
                    && ctx.get_random(100) < 30
                {
                    ctx.give_items(killer, SOULPIECEAIR, 1);
                    if ctx.get_quest_items_count(killer, SOULPIECEAIR) >= 6 {
                        let cond = qs.get_cond(ctx);
                        qs.set_cond(ctx, cond + 1, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            WATER => {
                // NOTE(java-quirk): `(cond >= 3) || (cond <= 5)` is always true in Java.
                if (qs.get_cond(ctx) >= 3 || qs.get_cond(ctx) <= 5)
                    && item_equipped(ctx, killer, paperdoll::RHAND) == YINSWORD
                    && ctx.get_quest_items_count(killer, SOULPIECEWATER) < 6
                    && ctx.get_random(100) < 30
                {
                    ctx.give_items(killer, SOULPIECEWATER, 1);
                    if ctx.get_quest_items_count(killer, SOULPIECEWATER) >= 6 {
                        let cond = qs.get_cond(ctx);
                        qs.set_cond(ctx, cond + 1, true);
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
        match npc.id {
            HOLLY => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 75 {
                        "30839-01.htm"
                    } else {
                        "30839-00.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30839-03.html".to_owned(),
                    2 => htmltext = "30839-05.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = "30839-0a.html".to_owned(),
                _ => {}
            },
            WEBER => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 75 {
                        "31307-01.htm"
                    } else {
                        "31307-00.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "31307-03.html".to_owned(),
                    7 => htmltext = "31307-05.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = "31307-0a.html".to_owned(),
                _ => {}
            },
            YIN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        2 => htmltext = "32325-01.html".to_owned(),
                        3 | 5 => htmltext = "32325-04.html".to_owned(),
                        4 => {
                            htmltext = "32325-08.html".to_owned();
                            ctx.take_items(player, YINSWORD, 1);
                            ctx.take_items(player, SOULPIECEWATER, -1);
                        }
                        6 => htmltext = "32325-10.html".to_owned(),
                        _ => {}
                    }
                }
            }
            YANG => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        7 => htmltext = "32326-01.html".to_owned(),
                        8 | 10 => htmltext = "32326-04.html".to_owned(),
                        9 => {
                            htmltext = "32326-08.html".to_owned();
                            ctx.take_items(player, YANGSWORD, 1);
                            ctx.take_items(player, SOULPIECEAIR, -1);
                        }
                        11 => htmltext = "32326-10.html".to_owned(),
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
    use l2_world::{ItemData, QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q10275_ContainingTheAttributePower";

    const NPCS: &str = r#"
        <npc id="30839" type="Folk" level="80" name="Holly"/>
        <npc id="31307" type="Folk" level="80" name="Weber"/>
        <npc id="32325" type="Folk" level="80" name="Yin"/>
        <npc id="32326" type="Folk" level="80" name="Yang"/>
        <npc id="27380" type="Monster" level="80" name="Water"/>
        <npc id="27381" type="Monster" level="80" name="Air"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13845" type="Weapon" name="Yin Sword"><set name="bodypart" val="rhand"/></item>
        <item id="13881" type="Weapon" name="Yang Sword"><set name="bodypart" val="rhand"/></item>
        <item id="13861" type="EtcItem" name="Soul Piece of Water"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13862" type="EtcItem" name="Soul Piece of Air"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10521" type="EtcItem" name="Crystal"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10275ContainingTheAttributePower);
        let pages = Pages::of(&Q10275ContainingTheAttributePower)
            .page("30839-01.htm", &["30839-02.html"])
            .plain(&["30839-00.html", "30839-02.html", "30839-03.html", "30839-0a.html"])
            .page("32325-01.html", &["32325-02.html"])
            .page("32325-02.html", &["32325-03.html"])
            .plain(&["32325-03.html", "32325-04.html", "32325-08.html"])
            .page("32325-10.html", &["1", "2", "3", "4", "5", "6"])
            .plain(&["32325-11.html"])
            .npcs(&[HOLLY, WEBER, YIN, YANG]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    fn at_cond(t: &mut ScriptTest, level: i32, cond: &str, items: Vec<ItemData>) {
        let mut data = player_data(1, level, items);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some(cond.to_owned()))],
        };
        t.enter_with(data);
    }

    fn equipped(object_id: i32, item_id: i32) -> ItemData {
        ItemData {
            loc: "PAPERDOLL".to_owned(),
            loc_data: paperdoll::RHAND as i32,
            ..item(object_id, item_id, 1)
        }
    }

    #[test]
    fn the_start_needs_level_76() {
        let mut t = setup();
        let holly = t.spawn(HOLLY, 20);
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, holly).contains("30839-00.html"));
        t.enter(2, 76, vec![]);
        assert!(t.quest_page(2, holly).contains("30839-01.htm"));
        assert!(t.event(2, NAME, "30839-02.html").contains("30839-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(2, holly).contains("30839-03.html"));
    }

    #[test]
    fn yin_gives_a_fire_sword() {
        let mut t = setup();
        let yin = t.spawn(YIN, 20);
        at_cond(&mut t, 80, "2", vec![]);
        assert!(t.quest_page(1, yin).contains("32325-01.html"));
        t.event(1, NAME, "32325-02.html");
        assert!(t.event(1, NAME, "32325-03.html").contains("32325-03.html"));
        assert_eq!(t.count(1, YINSWORD), 1);
        let sword = t
            .world
            .player(ObjectId(1))
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.item_id == YINSWORD)
            .cloned()
            .unwrap();
        assert_eq!(sword.elementals.len(), 1);
        assert!(t.quest_page(1, yin).contains("32325-04.html"));
    }

    #[test]
    fn the_sword_in_hand_collects_soul_pieces() {
        let mut t = setup();
        at_cond(&mut t, 80, "3", vec![equipped(0x6000_0100, YINSWORD)]);
        for _ in 0..200 {
            let mob = t.spawn(WATER, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, SOULPIECEWATER), 6);
    }

    #[test]
    fn the_crystal_choice_pays_and_ends_the_quest() {
        let mut t = setup();
        let yin = t.spawn(YIN, 20);
        at_cond(&mut t, 80, "6", vec![]);
        assert!(t.quest_page(1, yin).contains("32325-10.html"));
        assert!(t.event(1, NAME, "1").contains("32325-11.html"));
        assert_eq!(t.count(1, 10521), 2);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
