//! Quest 148: Path to Becoming an Exalted Mercenary (the mercenary captains, after the elite one).
// Java: dist/game/data/scripts/quests/Q00148_PathtoBecominganExaltedMercenary/Q00148_PathtoBecominganExaltedMercenary.java
//
// Pattern: the same chain of captain pages as Q00147. The elite certificate of Q00147 is the
// entry; the captain swaps it for the top elite certificate at cond 4. A castle clan is sent to
// the castle page, and a character with the Q00147 quest completed gets the 00a page.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MERC: [i32; 9] = [36481, 36482, 36483, 36484, 36485, 36486, 36487, 36488, 36489];

// Items
const ELITE_CERTIFICATE: i32 = 13767;
const TOP_ELITE_CERTIFICATE: i32 = 13768;

// Previous quest (Q00147), read only
const PREVIOUS_QUEST: &str = "Q00147_PathtoBecominganEliteMercenary";

/// Java `Q00148_PathtoBecominganExaltedMercenary`.
pub struct Q00148PathToBecomingAnExaltedMercenary;

impl Script for Q00148PathToBecomingAnExaltedMercenary {
    fn name(&self) -> &'static str {
        "Q00148_PathtoBecominganExaltedMercenary"
    }

    fn path(&self) -> &'static str {
        "quests/Q00148_PathtoBecominganExaltedMercenary"
    }

    fn id(&self) -> i32 {
        148
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&MERC);
        s.add_talk_id(&MERC);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let htmltext = event.to_owned();
        let Some(qs) = ctx.get_quest_state(player, false) else {
            // NOTE(java-quirk): without a quest state Java returns the event as the page too.
            return Some(htmltext);
        };
        if event.eq_ignore_ascii_case("exalted-00b.htm") {
            ctx.give_items(player, ELITE_CERTIFICATE, 1);
        } else if event.eq_ignore_ascii_case("exalted-03.htm") {
            qs.start_quest(ctx);
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let previous_done = ctx
                    .player_quest_state(player, PREVIOUS_QUEST)
                    .is_some_and(|prev| prev.is_completed());
                if ctx.world().clan_of(player).is_some_and(|c| c.castle_id > 0) {
                    htmltext = "castle.htm".to_owned();
                } else if ctx.has_quest_items(player, ELITE_CERTIFICATE) {
                    htmltext = "exalted-01.htm".to_owned();
                } else if previous_done {
                    htmltext = "exalted-00a.htm".to_owned();
                } else {
                    htmltext = "exalted-00.htm".to_owned();
                }
            }
            STARTED => {
                if qs.get_cond(ctx) < 4 {
                    htmltext = "exalted-04.htm".to_owned();
                } else if qs.is_cond(ctx, 4) {
                    ctx.take_items(player, ELITE_CERTIFICATE, -1);
                    ctx.give_items(player, TOP_ELITE_CERTIFICATE, 1);
                    qs.exit_quest(ctx, false, false);
                    htmltext = "exalted-05.htm".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, PlayerClanState};
    use l2_world::{ClanRecord, PlayerData, QuestLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    /// Stand-in for Q00147 (the previous quest, read only). It must not register the captains:
    /// two quests on one NPC would open the quest chooser instead of this quest's window.
    struct ElitePreviousStandIn;

    impl Script for ElitePreviousStandIn {
        fn name(&self) -> &'static str {
            PREVIOUS_QUEST
        }

        fn path(&self) -> &'static str {
            "quests/Q00147_PathtoBecominganEliteMercenary"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    const NAME: &str = "Q00148_PathtoBecominganExaltedMercenary";
    const LIONS: i32 = 1;
    const LORD: i32 = 2;
    /// A second class (`player_class::GLADIATOR`): the captain talks about the mercenaries.
    const GLADIATOR: i32 = 2;
    const HUMAN: i32 = 0;

    const NPCS: &str = r#"
        <npc id="36481" type="Folk" level="1" name="Mercenary Captain"/>"#;

    const ITEMS: &str = r#"
        <item id="13767" type="EtcItem" name="Elite Certificate"><set name="is_questitem" val="true"/></item>
        <item id="13768" type="EtcItem" name="Top Elite Certificate"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00148PathToBecomingAnExaltedMercenary);
        registry.add(ElitePreviousStandIn);
        let pages = Pages::of(&Q00148PathToBecomingAnExaltedMercenary)
            .page("exalted-00a.htm", &["exalted-00b.htm"])
            .page("exalted-00b.htm", &["exalted-02.htm"])
            .page("exalted-01.htm", &["exalted-02.htm"])
            .page("exalted-02.htm", &["exalted-03.htm"])
            .plain(&[
                "castle.htm",
                "exalted-00.htm",
                "exalted-03.htm",
                "exalted-04.htm",
                "exalted-05.htm",
            ])
            .npcs(&MERC)
            // The captain's first page for a second-class character of level 40 who is not a
            // castle lord (`36481-01.html`): its `bypass Script` opens the quest window.
            .file(
                "data/scripts/ai/others/MercenaryCaptain/36481-01.html",
                r#"<html><body>captain <a action="bypass Script">Quest</a></body></html>"#,
            );
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        // The Lions, a clan that owns Gludio (castle 1).
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: LIONS,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 1,
                    leader_id: LORD,
                    ..ClanDataRecord::default()
                }),
                members: vec![ClanMemberRecord {
                    object_id: LORD,
                    name: "P2".to_owned(),
                    level: 40,
                    power_grade: 1,
                    ..ClanMemberRecord::default()
                }],
                ..ClanRecord::default()
            }],
            Vec::new(),
        );
        let merc = t.spawn(MERC[0], 20);
        (t, merc)
    }

    /// A second-class character of level 40 with the given quest rows and items.
    fn second_class(id: i32, quests: QuestLoad, items: Vec<l2_world::ItemData>) -> PlayerData {
        let mut data = player_data(id, 40, items);
        data.race = HUMAN;
        data.class_id = GLADIATOR;
        data.base_class = GLADIATOR;
        data.profile.quests = quests;
        data
    }

    fn previous_completed() -> QuestLoad {
        QuestLoad {
            states: vec![(PREVIOUS_QUEST.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        }
    }

    #[test]
    fn a_clan_with_a_castle_is_sent_to_the_castle_page() {
        let (mut t, merc) = setup();
        let mut data = second_class(2, QuestLoad::default(), vec![]);
        data.profile.clan = Some(PlayerClanState {
            clan_id: LIONS,
            privileges: ClanPrivilegesRef::Own(0x00FF_FFFF),
            ..PlayerClanState::default()
        });
        t.enter_with(data);
        // The lord's captain page has no quest link in the datapack, so the hook is called
        // directly: the castle check of Java's `onTalk`.
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(merc) {
                page = Q00148PathToBecomingAnExaltedMercenary.on_talk(ctx, npc, Oid(2));
            }
        });
        assert_eq!(page.as_deref(), Some("castle.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_elite_quest_the_character_is_sent_away() {
        let (mut t, merc) = setup();
        t.enter_with(second_class(1, QuestLoad::default(), vec![]));
        assert!(t.quest_page(1, merc).contains("exalted-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_elite_certificate_and_the_start() {
        let (mut t, merc) = setup();
        t.enter_with(second_class(1, previous_completed(), vec![]));
        assert!(t.quest_page(1, merc).contains("exalted-00a.htm"));
        assert!(t.event(1, NAME, "exalted-00b.htm").contains("exalted-00b.htm"));
        assert_eq!(t.count(1, ELITE_CERTIFICATE), 1);
        assert!(t.take_writes().is_empty());

        // Page 02 links the start (page 03).
        assert!(t.event(1, NAME, "exalted-02.htm").contains("exalted-02.htm"));
        assert!(t.event(1, NAME, "exalted-03.htm").contains("exalted-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Cond below 4: the captain says to wait, and the certificate in hand gives page 01
        // only before the quest starts (here: started, so page 04).
        assert!(t.quest_page(1, merc).contains("exalted-04.htm"));
    }

    #[test]
    fn the_top_elite_certificate_at_cond_4() {
        let (mut t, merc) = setup();
        let quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("4".to_owned()))],
        };
        t.enter_with(second_class(1, quests, vec![item(5, ELITE_CERTIFICATE, 1)]));
        assert!(t.quest_page(1, merc).contains("exalted-05.htm"));
        assert_eq!(t.count(1, ELITE_CERTIFICATE), 0);
        assert_eq!(t.count(1, TOP_ELITE_CERTIFICATE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
