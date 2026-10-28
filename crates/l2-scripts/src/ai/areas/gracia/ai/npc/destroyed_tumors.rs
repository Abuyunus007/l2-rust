//! The Destroyed Tumors of the Seed of Infinity: the pages of the Hall of Suffering / Erosion /
//! Heart of Infinity instances.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/DestroyedTumors/DestroyedTumors.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

/// Java's `warpTimer`, the time the script was made.
#[derive(Default)]
struct TumorsState {
    warp_timer: i64,
}

/// Java `ai.areas.Gracia.AI.NPC.DestroyedTumors.DestroyedTumors`.
pub struct DestroyedTumors;

impl DestroyedTumors {
    /// The `showcheckpage` branch the three instances share, with their last page.
    fn show_check_page(ctx: &mut ScriptCtx<'_>, player: ObjectId, tag: i32, warp_timer: i64, last: &str) -> String {
        let has_item = ctx.has_quest_items(player, 13797);
        if !has_item {
            "32535-6.htm".to_owned()
        } else if (warp_timer + 60000) > ctx.now_ms() {
            "32535-4.htm".to_owned()
        } else if tag <= 0 {
            "32535-3.htm".to_owned()
        } else {
            last.to_owned()
        }
    }
}

impl Script for DestroyedTumors {
    fn name(&self) -> &'static str {
        "DestroyedTumors"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/DestroyedTumors"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[32535]);
        s.add_first_talk_id(&[32535]);
        s.add_talk_id(&[32535]);

        s.add_start_npc(&[32536]);
        s.add_first_talk_id(&[32536]);
        s.add_talk_id(&[32536]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let now = ctx.now_ms();
        ctx.state::<TumorsState>().warp_timer = now;
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

        ctx.get_quest_state(player, true);

        let warp_timer = ctx.state::<TumorsState>().warp_timer;
        // `InstanceManager.getInstance().getPlayerWorld(player)`: the template and the "tag".
        let world = ctx
            .world()
            .player_instance_world(player)
            .map(|w| (w.template_id, w.parameters.get("tag").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0)));
        let is_leader = {
            let party = ctx.world().party_of(player);
            party.is_some() && ctx.world().is_party_leader(player)
        };
        let npc_id = npc.map(|n| n.id);

        if let Some((template_id, tag)) = world.filter(|(template, _)| (119..=121).contains(template)) {
            if event.eq_ignore_ascii_case("examine_tumor") {
                if template_id == 121 {
                    if npc_id == Some(32536) {
                        htmltext = Some(if !is_leader { "32536-2.htm" } else { "32536-1.htm" }.to_owned());
                    }

                    if npc_id == Some(32535) {
                        htmltext = Some(if !is_leader { "32535-2.htm" } else { "32535-7.htm" }.to_owned());
                    }
                } else {
                    htmltext = Some(if !is_leader { "32535-2.htm" } else { "32535-1.htm" }.to_owned());
                }
            } else if event.eq_ignore_ascii_case("showcheckpage") {
                let last = match template_id {
                    119 => "32535-5a.htm",
                    120 => "32535-5b.htm",
                    _ => "32535-5.htm",
                };
                htmltext = Some(Self::show_check_page(ctx, player, tag, warp_timer, last));
            } else if template_id == 121 && event.eq_ignore_ascii_case("reenter") {
                let count = ctx.get_quest_items_count(player, 13797);
                htmltext = Some(if !ctx.has_quest_items(player, 13797) || count < 3 {
                    "32535-6.htm"
                } else {
                    "32535-8.htm"
                }
                .to_owned());
            }
        } else if let Some((122, _)) = world {
            if event.eq_ignore_ascii_case("examine_tumor") && npc_id == Some(32535) {
                htmltext = Some("32535-4.htm".to_owned());
            }
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);

        if npc.id == 32535 {
            return Some("32535.htm".to_owned());
        }

        if npc.id == 32536 {
            return Some("32536.htm".to_owned());
        }

        Some(String::new())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::instance::InstanceWorld;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="32535" type="Folk" level="70" name="Tumor"/>
        <npc id="32536" type="Folk" level="70" name="Tumor2"/>"#;
    const ITEMS: &str = r#"<item id="13797" type="EtcItem" name="Tumor item"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/DestroyedTumors";

    fn setup(template: i32, tag: i32, items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(DestroyedTumors);
        let links: String = ["examine_tumor", "showcheckpage", "reenter"]
            .iter()
            .map(|e| format!(r#"<a action="bypass -h Script DestroyedTumors {e}">x</a>"#))
            .collect();
        let mut pages = vec![
            (format!("{DIR}/32535.htm"), format!("<html><body>32535 {links}</body></html>")),
            (format!("{DIR}/32536.htm"), format!("<html><body>32536 {links}</body></html>")),
        ];
        for page in [
            "32535-1", "32535-2", "32535-3", "32535-4", "32535-5", "32535-5a", "32535-5b", "32535-6",
            "32535-7", "32535-8", "32536-1", "32536-2",
        ] {
            pages.push((format!("{DIR}/{page}.htm"), format!("<html><body>{page} {links}</body></html>")));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let tumor = t.spawn(32535, 20);
        let tumor2 = t.spawn(32536, 25);
        t.enter(1, 80, items);
        // A script world of that template that allows the player.
        let mut world = InstanceWorld::new(500, template);
        world.parameters.insert("tag".to_owned(), tag.to_string());
        world.add_allowed(ObjectId(1));
        t.world.add_instance_world(world);
        t.talk(1, tumor);
        t.take();
        (t, tumor, tumor2)
    }

    fn ask(t: &mut ScriptTest, event: &str) -> String {
        t.bypass(1, &format!("Script DestroyedTumors {event}"));
        pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    #[test]
    fn a_non_leader_is_refused_in_the_hall_of_suffering() {
        let (mut t, _, _) = setup(119, 1, vec![]);
        assert!(ask(&mut t, "examine_tumor").contains("32535-2"));
    }

    #[test]
    fn the_check_page_depends_on_the_item_the_timer_and_the_tag() {
        // No item: 32535-6.
        let (mut t, _, _) = setup(119, 1, vec![]);
        assert!(ask(&mut t, "showcheckpage").contains("32535-6"));
        // With the item, one minute after the script was made: 32535-4.
        let (mut t, _, _) = setup(120, 1, vec![item(1000, 13797, 3)]);
        assert!(ask(&mut t, "showcheckpage").contains("32535-4"));
    }

    #[test]
    fn after_a_minute_the_check_page_goes_by_the_tag_and_the_instance() {
        for (template, tag, page) in [(119, 1, "32535-5a"), (120, 1, "32535-5b"), (121, 1, "32535-5"), (119, 0, "32535-3")] {
            let (mut t, _, _) = setup(template, tag, vec![item(1000, 13797, 3)]);
            // The script was made more than a minute ago.
            t.world.run_as_script("DestroyedTumors", |ctx| ctx.state::<TumorsState>().warp_timer -= 120_000);
            let shown = ask(&mut t, "showcheckpage");
            assert!(shown.contains(&format!("{page} ")), "{template} {tag}: {shown}");
        }
    }

    #[test]
    fn the_destruction_tumor_takes_three_items_to_come_back() {
        let (mut t, _, _) = setup(121, 1, vec![item(1000, 13797, 2)]);
        assert!(ask(&mut t, "reenter").contains("32535-6"));
        let (mut t, _, _) = setup(121, 1, vec![item(1000, 13797, 3)]);
        assert!(ask(&mut t, "reenter").contains("32535-8"));
    }

    #[test]
    fn the_closed_instance_only_knows_the_examination() {
        let (mut t, _, _) = setup(122, 1, vec![]);
        assert!(ask(&mut t, "examine_tumor").contains("32535-4"));
    }
}
