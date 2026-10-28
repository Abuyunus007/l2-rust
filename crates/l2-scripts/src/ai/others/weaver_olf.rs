//! Weaver Olf: unseals the Sealed Magic Pins, Pouches, Rune Clips and Ornaments for adena.
// Java: dist/game/data/scripts/ai/others/WeaverOlf/WeaverOlf.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NPCS: [i32; 2] = [
    32610, // Olf Kanore
    32612, // Olf Adams
];

/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

const UNSEAL_PRICE: [i64; 4] = [3200, 11800, 26500, 136600];

const CHANCES: [i32; 4] = [
    1,   // top
    10,  // high
    40,  // mid
    100, // low
];

/// Per grade: the sealed item, then the top, high, mid and low grade result.
const PINS: [[i32; 5]; 4] = [
    [13898, 13905, 13904, 13903, 13902], // C-Grade
    [13899, 13909, 13908, 13907, 13906], // B-Grade
    [13900, 13913, 13912, 13911, 13910], // A-Grade
    [13901, 13917, 13916, 13915, 13914], // S-Grade
];

const POUCHS: [[i32; 5]; 4] = [
    [13918, 13925, 13924, 13923, 13922], // C-Grade
    [13919, 13929, 13928, 13927, 13926], // B-Grade
    [13920, 13933, 13932, 13931, 13930], // A-Grade
    [13921, 13937, 13936, 13935, 13934], // S-Grade
];

const CLIPS_ORNAMENTS: [[i32; 5]; 4] = [
    [14902, 14909, 14908, 14907, 14906], // Magic Rune Clip (A-Grade)
    [14903, 14913, 14912, 14911, 14910], // Magic Rune Clip (S-Grade)
    [14904, 14917, 14916, 14915, 14914], // Magic Ornament (A-Grade)
    [14905, 14921, 14920, 14919, 14918], // Magic Ornament (S-Grade)
];

/// Java `ai.others.WeaverOlf.WeaverOlf`.
pub struct WeaverOlf;

impl Script for WeaverOlf {
    fn name(&self) -> &'static str {
        "WeaverOlf"
    }

    fn path(&self) -> &'static str {
        "ai/others/WeaverOlf"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.contains("_grade_") {
            // NOTE(java-quirk): Java parses the first character and indexes the tables with it; a
            // bad character or index throws, and nothing happens here.
            let grade = usize::try_from(event.chars().next()?.to_digit(10)?).ok()?;
            let (price, item_ids) = if event.ends_with("_pin") {
                (*UNSEAL_PRICE.get(grade)?, PINS.get(grade)?)
            } else if event.ends_with("_pouch") {
                (*UNSEAL_PRICE.get(grade)?, POUCHS.get(grade)?)
            } else if event.ends_with("_clip") {
                (
                    *UNSEAL_PRICE.get(grade)?,
                    CLIPS_ORNAMENTS.get(grade.checked_sub(2)?)?,
                )
            } else if event.ends_with("_ornament") {
                (*UNSEAL_PRICE.get(grade)?, CLIPS_ORNAMENTS.get(grade)?)
            } else {
                return None;
            };
            let npc = npc?;
            let player = player?;

            if ctx.has_quest_items(player, item_ids[0]) {
                // NOTE(java-quirk): `>` and not `>=`: a player with exactly the price is refused.
                if ctx.adena_of(player) > price {
                    ctx.take_items(player, ADENA_ID, price);
                    ctx.take_items(player, item_ids[0], 1);
                    let rand = ctx.get_random(200);
                    if rand <= CHANCES[0] {
                        ctx.give_items(player, item_ids[1], 1);
                    } else if rand <= CHANCES[1] {
                        ctx.give_items(player, item_ids[2], 1);
                    } else if rand <= CHANCES[2] {
                        ctx.give_items(player, item_ids[3], 1);
                    } else if rand <= CHANCES[3] {
                        ctx.give_items(player, item_ids[4], 1);
                    } else {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::WhatAPredicamentMyAttemptsWereUnsuccessful,
                            &[],
                        );
                    }
                } else {
                    return Some(format!("{}-low.htm", npc.id));
                }
            } else {
                return Some(format!("{}-no.htm", npc.id));
            }

            return None;
        }

        Some(event.to_owned())
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}-1.htm", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS_XML: &str = r#"<npc id="32610" type="Folk" level="70" name="Olf Kanore"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13898" type="EtcItem" name="Sealed Magic Pin"/>
        <item id="13905" type="EtcItem" name="Top Pin"/>
        <item id="13904" type="EtcItem" name="High Pin"/>
        <item id="13903" type="EtcItem" name="Mid Pin"/>
        <item id="13902" type="EtcItem" name="Low Pin"/>"#;

    fn event(t: &mut ScriptTest, name: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("WeaverOlf", |ctx| {
            let npc = ctx.script_npc(t_npc(ctx));
            out = WeaverOlf.on_event(ctx, name, npc, Some(ObjectId(1)));
        });
        out
    }

    fn t_npc(ctx: &ScriptCtx<'_>) -> ObjectId {
        *ctx.world().npc_ids().first().unwrap()
    }

    fn setup(items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(WeaverOlf);
        let mut t = ScriptTest::new(registry, NPCS_XML, ITEMS, &[]);
        t.spawn(32610, 20);
        t.enter(1, 60, items);
        t
    }

    #[test]
    fn other_events_are_echoed_and_the_talk_page_follows_the_npc() {
        let mut t = setup(vec![]);
        assert_eq!(event(&mut t, "32610-2.htm").as_deref(), Some("32610-2.htm"));
        let mut page = None;
        t.world.run_as_script("WeaverOlf", |ctx| {
            let npc = ctx.script_npc(t_npc(ctx)).unwrap();
            page = WeaverOlf.on_talk(ctx, npc, ObjectId(1));
        });
        assert_eq!(page.as_deref(), Some("32610-1.htm"));
    }

    #[test]
    fn without_the_sealed_pin_or_the_adena_a_page_answers() {
        let mut t = setup(vec![]);
        assert_eq!(event(&mut t, "0_grade_pin").as_deref(), Some("32610-no.htm"));
        let mut t = setup(vec![item(5000, 13898, 1), item(5001, 57, 3200)]);
        // Exactly the price is not enough (`>`).
        assert_eq!(event(&mut t, "0_grade_pin").as_deref(), Some("32610-low.htm"));
    }

    #[test]
    fn unsealing_takes_the_pin_and_the_price_and_gives_a_result_or_a_sigh() {
        let mut t = setup(vec![item(5000, 13898, 1), item(5001, 57, 10000)]);
        assert_eq!(event(&mut t, "0_grade_pin"), None);
        assert_eq!(t.count(1, 13898), 0);
        assert_eq!(t.count(1, 57), 10000 - 3200);
        let results: i64 = [13905, 13904, 13903, 13902].iter().map(|&i| t.count(1, i)).sum();
        // rand 0..199: 100 of 200 give an item, the rest the sigh; with the seeded RNG either way
        // exactly one of the two happened.
        assert!(results <= 1);
    }

    #[test]
    fn the_clip_table_starts_at_grade_two() {
        let mut t = setup(vec![]);
        // Grade 0 or 1 clips index below the table: Java throws, nothing happens.
        assert_eq!(event(&mut t, "1_grade_clip"), None);
        assert_eq!(event(&mut t, "2_grade_clip").as_deref(), Some("32610-no.htm"));
    }
}
