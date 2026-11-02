//! The shadow weapon exchange pages of the armor and weapon shops.
// Java: dist/game/data/scripts/custom/ShadowWeapons/ShadowWeapons.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// @formatter:off
const NPCS: &[i32] = &[
    30037, 30066, 30070, 30109, 30115, 30120, 30174, 30175, 30176, 30187, 30191, 30195, 30288, 30289, 30290,
    30297, 30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30511, 30512, 30513, 30595, 30676,
    30677, 30681, 30685, 30687, 30689, 30694, 30699, 30704, 30845, 30847, 30849, 30854, 30857, 30862, 30865,
    30894, 30897, 30900, 30905, 30910, 30913, 31269, 31272, 31276, 31285, 31288, 31314, 31317, 31321, 31324,
    31326, 31328, 31331, 31334, 31336, 31958, 31961, 31965, 31968, 31974, 31977, 31996, 32092, 32093, 32094,
    32095, 32096, 32097, 32098, 32193, 32196, 32199, 32202, 32205, 32206, 32213, 32214, 32221, 32222, 32229,
    32230, 32233, 32234,
];
// @formatter:on

/// Shadow Item Exchange Coupon (D-Grade).
const COUPON_D: i32 = 8869;
/// Shadow Item Exchange Coupon (C-Grade).
const COUPON_C: i32 = 8870;

/// Java `custom.ShadowWeapons.ShadowWeapons`.
pub struct ShadowWeapons;

impl Script for ShadowWeapons {
    fn name(&self) -> &'static str {
        "ShadowWeapons"
    }

    fn path(&self) -> &'static str {
        "custom/ShadowWeapons"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(NPCS);
        s.add_talk_id(NPCS);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let has_d = ctx.has_quest_items(player, COUPON_D);
        let has_c = ctx.has_quest_items(player, COUPON_C);
        let htmltext = if has_d || has_c {
            if !has_d {
                "exchange_c.html"
            } else if !has_c {
                "exchange_d.html"
            } else {
                "exchange_both.html"
            }
        } else {
            "exchange_no.html"
        };

        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPC: i32 = 30037;
    const NPCS_XML: &str = r#"<npc id="30037" type="Folk" level="70" name="Shop"/>"#;
    const ITEMS: &str =
        r#"<item id="8869" type="EtcItem" name="D coupon"/><item id="8870" type="EtcItem" name="C coupon"/>"#;

    fn page_for(items: Vec<l2_world::ItemData>) -> String {
        let mut registry = ScriptRegistry::new();
        registry.add(ShadowWeapons);
        let pages = Pages::of(&ShadowWeapons)
            .plain(&[
                "exchange_c.html",
                "exchange_d.html",
                "exchange_both.html",
                "exchange_no.html",
            ])
            .npcs_by_name(&[NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS_XML, ITEMS, pages);
        t.enter(1, 40, items);
        let npc = t.spawn(NPC, 20);
        t.script_page(1, npc, "ShadowWeapons")
    }

    #[test]
    fn the_page_follows_the_coupons() {
        assert!(page_for(vec![]).contains("exchange_no.html"));
        assert!(page_for(vec![item(100, 8869, 1)]).contains("exchange_d.html"));
        assert!(page_for(vec![item(100, 8870, 1)]).contains("exchange_c.html"));
        assert!(page_for(vec![item(100, 8869, 1), item(101, 8870, 1)]).contains("exchange_both.html"));
    }

    #[test]
    fn the_npc_list_is_the_java_one() {
        assert_eq!(NPCS.len(), 93);
    }
}
