//! Mage Nottingale, the guide of Gracia: radar markers to the air ship docks and hunting grounds.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/Nottingale/Nottingale.java

use l2_protocol_game::server::RadarControl;
use l2_util::ObjectId;
use l2_world::clan::ClanAccess;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NOTTINGALE: i32 = 32627;

/// Java `Q10273_GoodDayToFly.class.getSimpleName()`.
const GOOD_DAY_TO_FLY: &str = "Q10273_GoodDayToFly";

/// Java `RADARS.get(key)`: `new RadarControl(0, a, b, c, d)`.
// NOTE(java-quirk): the entries pass four numbers after the 0 to a five-argument constructor
// `(showRadar, type, x, y, z)`, so the type is the x coordinate and the others are shifted. Ported
// as they are.
fn radar(key: i32) -> Option<RadarControl> {
    let (radar_type, x, y, z) = match key {
        2 => (-184545, 243120, 1581, 2),
        5 => (-192361, 254528, 3598, 1),
        6 => (-174600, 219711, 4424, 1),
        7 => (-181989, 208968, 4424, 1),
        8 => (-252898, 235845, 5343, 1),
        9 => (-212819, 209813, 4288, 1),
        10 => (-246899, 251918, 4352, 1),
        _ => return None,
    };
    Some(RadarControl {
        show_radar: 0,
        radar_type,
        x,
        y,
        z,
    })
}

/// Java `ai.areas.Gracia.AI.NPC.Nottingale.Nottingale`.
pub struct Nottingale;

impl Nottingale {
    /// The "no airship, no quest" branch: the Aurabird page for those who finished
    /// `Q10273_GoodDayToFly`, else the radar to the first marker and the refusal page.
    fn quest_or_refuse(ctx: &mut ScriptCtx<'_>, player: ObjectId, event: &str) -> Option<String> {
        let completed = ctx
            .player_quest_state(player, GOOD_DAY_TO_FLY)
            .is_some_and(|qs| qs.is_completed());
        if completed {
            Some(event.to_owned())
        } else {
            if let Some(packet) = radar(2) {
                ctx.world().send(player, &packet);
            }
            Some("32627-01.html".to_owned())
        }
    }
}

impl Script for Nottingale {
    fn name(&self) -> &'static str {
        "Nottingale"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/Nottingale"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NOTTINGALE]);
        s.add_talk_id(&[NOTTINGALE]);
        s.add_first_talk_id(&[NOTTINGALE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        match event {
            "32627-02.html" | "32627-03.html" | "32627-04.html" => {
                let clan_id = ctx.world().clan_id_of(player);
                if clan_id != 0 {
                    if ctx.world().has_clan_access(player, ClanAccess::AccessAirship)
                        && ctx.world().has_air_ship_license(clan_id)
                        && !ctx.world().has_air_ship(clan_id)
                    {
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Self::quest_or_refuse(ctx, player, event);
                    }
                } else {
                    htmltext = Self::quest_or_refuse(ctx, player, event);
                }
            }
            "32627-05.html" | "32627-06.html" | "32627-07.html" | "32627-08.html" | "32627-09.html"
            | "32627-10.html" => {
                // `Integer.parseInt(event.substring(6, 8))`
                if let Some(packet) = event.get(6..8).and_then(|n| n.parse().ok()).and_then(radar) {
                    ctx.world().send(player, &packet);
                }
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to, player_data};

    const NPCS: &str = r#"<npc id="32627" type="Folk" level="70" name="Nottingale"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/Nottingale";

    /// Java `Q10273_GoodDayToFly` as far as Nottingale needs it: a quest of that name.
    struct GoodDayToFly;

    impl Script for GoodDayToFly {
        fn name(&self) -> &'static str {
            GOOD_DAY_TO_FLY
        }
        fn path(&self) -> &'static str {
            "quests/Q10273_GoodDayToFly"
        }
        fn id(&self) -> i32 {
            10273
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, l2_util::ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Nottingale);
        registry.add(GoodDayToFly);
        let mut pages = vec![(
            format!("{DIR}/32627.html"),
            [2, 3, 4]
                .iter()
                .map(|n| format!(r#"<a action="bypass Script Nottingale 32627-0{n}.html">x</a>"#))
                .collect::<String>(),
        )];
        for n in 1..=10 {
            let links: String = [5, 6, 7, 8, 9, 10]
                .iter()
                .map(|m| {
                    let page = if *m < 10 {
                        format!("32627-0{m}.html")
                    } else {
                        format!("32627-{m}.html")
                    };
                    format!(r#"<a action="bypass Script Nottingale {page}">x</a>"#)
                })
                .collect();
            let name = if n < 10 { format!("32627-0{n}.html") } else { format!("32627-{n}.html") };
            pages.push((format!("{DIR}/{name}"), format!("<html><body>{name}{links}</body></html>")));
        }
        pages[0].1 = format!("<html><body>32627{}</body></html>", pages[0].1);
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let npc = t.spawn(NOTTINGALE, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, npc);
        t.take();
        (t, npc)
    }

    #[test]
    fn a_character_without_the_quest_is_marked_the_way() {
        let (mut t, _) = setup();
        t.bypass(1, "Script Nottingale 32627-02.html");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32627-01"));
        assert!(names_to(&out, 1).contains(&"RADAR_CONTROL".to_owned()));
    }

    #[test]
    fn a_character_with_the_finished_quest_gets_the_page() {
        let (mut t, npc) = setup();
        let mut data = player_data(2, 80, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(GOOD_DAY_TO_FLY.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        t.talk(2, npc);
        t.take();
        t.bypass(2, "Script Nottingale 32627-03.html");
        let out = t.take();
        assert!(pages_to(&out, 2).pop().unwrap().contains("32627-03"));
        assert!(!names_to(&out, 2).contains(&"RADAR_CONTROL".to_owned()));
    }

    #[test]
    fn the_marker_pages_send_the_radar() {
        let (mut t, _) = setup();
        // The marker pages are linked from the pages the first answer opens; ask for the quest page
        // (refused) and then the markers directly: the page `32627-01.html` links them too.
        t.bypass(1, "Script Nottingale 32627-02.html");
        t.take();
        t.bypass(1, "Script Nottingale 32627-05.html");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("32627-05"));
        assert!(names_to(&out, 1).contains(&"RADAR_CONTROL".to_owned()));
    }
}
