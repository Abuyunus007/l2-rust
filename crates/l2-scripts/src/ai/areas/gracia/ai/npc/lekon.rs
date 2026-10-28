//! Engineer Lekon: the Airship Summon License for ten Energy Star Stones.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/Lekon/Lekon.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LEKON: i32 = 32557;

// Items
const LICENCE: i32 = 13559; // Airship Summon License
const STONE: i32 = 13277; // Energy Star Stone

// Misc
const MIN_CLAN_LV: i32 = 5;
const STONE_COUNT: i64 = 10;

/// Java `ai.areas.Gracia.AI.NPC.Lekon.Lekon`.
pub struct Lekon;

impl Script for Lekon {
    fn name(&self) -> &'static str {
        "Lekon"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/Lekon"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[LEKON]);
        s.add_talk_id(&[LEKON]);
        s.add_start_npc(&[LEKON]);
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
            "32557-01.html" => {
                htmltext = Some(event.to_owned());
            }
            "licence" => {
                let clan = ctx.world().clan_of(player).map(|c| (c.id, c.level));
                if clan.is_none_or(|(_, level)| !ctx.world().is_clan_leader(player) || level < MIN_CLAN_LV) {
                    htmltext = Some("32557-02.html".to_owned());
                } else if ctx.has_at_least_one_quest_item(player, &[LICENCE]) {
                    htmltext = Some("32557-04.html".to_owned());
                } else if clan.is_some_and(|(id, _)| ctx.world().has_air_ship_license(id)) {
                    ctx.world().send(
                        player,
                        &SystemMessage::new(SystemMessageId::TheAirshipSummonLicenseHasAlreadyBeenAcquired),
                    );
                } else if ctx.get_quest_items_count(player, STONE) >= STONE_COUNT {
                    ctx.take_items(player, STONE, STONE_COUNT);
                    ctx.give_items(player, LICENCE, 1);
                } else {
                    htmltext = Some("32557-03.html".to_owned());
                }
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
    use l2_world::{ClanRecord, PlayerClanState, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to, player_data};

    const NPCS: &str = r#"<npc id="32557" type="Folk" level="70" name="Lekon"/>"#;
    const ITEMS: &str = r#"<item id="13559" type="EtcItem" name="Airship Summon License"/>
        <item id="13277" type="EtcItem" name="Energy Star Stone"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/Lekon";

    fn setup(stones: i64) -> (ScriptTest, l2_util::ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Lekon);
        let pages = [
            (
                format!("{DIR}/32557.html"),
                r#"<html><body>32557 <a action="bypass Script Lekon 32557-01.html">x</a></body></html>"#.to_owned(),
            ),
            (
                format!("{DIR}/32557-01.html"),
                r#"<html><body>32557-01 <a action="bypass -h Script Lekon licence">y</a></body></html>"#.to_owned(),
            ),
            (format!("{DIR}/32557-02.html"), "<html><body>32557-02</body></html>".to_owned()),
            (format!("{DIR}/32557-03.html"), "<html><body>32557-03</body></html>".to_owned()),
            (format!("{DIR}/32557-04.html"), "<html><body>32557-04</body></html>".to_owned()),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(LEKON, 20);
        t.enter(1, 80, vec![item(1000, STONE, stones)]);
        t.talk(1, npc);
        t.bypass(1, "Script Lekon 32557-01.html");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32557-01"));
        (t, npc)
    }

    /// A clan leader of a clan of `level` (clan 100) enters in place of the clanless player.
    fn leader(t: &mut ScriptTest, npc: l2_util::ObjectId, level: i32, stones: i64) {
        use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
        let mut data = player_data(2, 80, vec![item(2000, STONE, stones)]);
        data.profile.clan = Some(PlayerClanState {
            clan_id: 100,
            ..Default::default()
        });
        data.profile.clan_record = Some(Box::new(ClanRecord {
            clan_id: 100,
            data: Some(ClanDataRecord {
                name: Some("Clan".to_owned()),
                level,
                leader_id: 2,
                ..Default::default()
            }),
            members: vec![ClanMemberRecord {
                object_id: 2,
                name: "P2".to_owned(),
                level: 80,
                ..Default::default()
            }],
            ..Default::default()
        }));
        t.enter_with(data);
        t.talk(2, npc);
        t.bypass(2, "Script Lekon 32557-01.html");
        t.take();
    }

    #[test]
    fn a_character_without_a_clan_is_sent_away() {
        let (mut t, _) = setup(10);
        t.bypass(1, "Script Lekon licence");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32557-02"));
        assert_eq!(t.count(1, STONE), 10);
        assert_eq!(t.count(1, LICENCE), 0);
    }

    #[test]
    fn a_leader_of_a_small_clan_is_sent_away() {
        let (mut t, npc) = setup(10);
        leader(&mut t, npc, 4, 10);
        t.bypass(2, "Script Lekon licence");
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("32557-02"));
    }

    #[test]
    fn nine_stones_are_not_enough() {
        let (mut t, npc) = setup(10);
        leader(&mut t, npc, 5, 9);
        t.bypass(2, "Script Lekon licence");
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("32557-03"));
        assert_eq!(t.count(2, STONE), 9);
        assert_eq!(t.count(2, LICENCE), 0);
    }

    #[test]
    fn ten_stones_buy_the_licence_once() {
        let (mut t, npc) = setup(10);
        leader(&mut t, npc, 5, 10);
        t.bypass(2, "Script Lekon licence");
        // No page: the licence is just handed over.
        let out = t.take();
        assert!(pages_to(&out, 2).is_empty(), "{:?}", pages_to(&out, 2));
        assert_eq!(t.count(2, STONE), 0);
        assert_eq!(t.count(2, LICENCE), 1);
        // A second try: the character holds the licence.
        t.bypass(2, "Script Lekon licence");
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("32557-04"));
    }

    #[test]
    fn a_clan_that_has_the_licence_gets_a_message_and_keeps_its_stones() {
        let (mut t, npc) = setup(10);
        leader(&mut t, npc, 5, 10);
        t.world.register_air_ship_license(100);
        t.bypass(2, "Script Lekon licence");
        let out = t.take();
        assert!(pages_to(&out, 2).is_empty());
        assert!(names_to(&out, 2).contains(&"SYSTEM_MESSAGE".to_owned()));
        assert_eq!(t.count(2, STONE), 10);
        assert_eq!(t.count(2, LICENCE), 0);
    }
}
