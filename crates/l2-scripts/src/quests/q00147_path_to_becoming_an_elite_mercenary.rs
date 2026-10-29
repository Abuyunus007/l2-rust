//! Quest 147: Path to Becoming an Elite Mercenary (the mercenary captains of the castles).
// Java: dist/game/data/scripts/quests/Q00147_PathtoBecominganEliteMercenary/Q00147_PathtoBecominganEliteMercenary.java
//
// Pattern: a one-time chain of mercenary captain pages. The first talk hands out the ordinary
// certificate; the quest is started on the last page; the reward swaps the certificate. A clan
// that owns a castle is sent to the castle page instead (`castle.htm`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MERC: [i32; 9] = [36481, 36482, 36483, 36484, 36485, 36486, 36487, 36488, 36489];

// Items
const ORDINARY_CERTIFICATE: i32 = 13766;
const ELITE_CERTIFICATE: i32 = 13767;

/// Java `Q00147_PathtoBecominganEliteMercenary`.
pub struct Q00147PathToBecomingAnEliteMercenary;

impl Script for Q00147PathToBecomingAnEliteMercenary {
    fn name(&self) -> &'static str {
        "Q00147_PathtoBecominganEliteMercenary"
    }

    fn path(&self) -> &'static str {
        "quests/Q00147_PathtoBecominganEliteMercenary"
    }

    fn id(&self) -> i32 {
        147
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
        if event.eq_ignore_ascii_case("elite-02.htm") {
            if ctx.has_quest_items(player, ORDINARY_CERTIFICATE) {
                return Some("elite-02a.htm".to_owned());
            }
            ctx.give_items(player, ORDINARY_CERTIFICATE, 1);
        } else if event.eq_ignore_ascii_case("elite-04.htm") {
            qs.start_quest(ctx);
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.world().clan_of(player).is_some_and(|c| c.castle_id > 0) {
                    "castle.htm".to_owned()
                } else {
                    "elite-01.htm".to_owned()
                };
            }
            STARTED => {
                if qs.get_cond(ctx) < 4 {
                    htmltext = "elite-05.htm".to_owned();
                } else if qs.is_cond(ctx, 4) {
                    ctx.take_items(player, ORDINARY_CERTIFICATE, -1);
                    ctx.give_items(player, ELITE_CERTIFICATE, 1);
                    qs.exit_quest(ctx, false, false);
                    htmltext = "elite-06.htm".to_owned();
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
    use l2_world::{ClanRecord, PlayerData, QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00147_PathtoBecominganEliteMercenary";
    const LIONS: i32 = 1;
    const LORD: i32 = 2;
    /// A second class (`player_class::GLADIATOR`): the captain talks about the mercenaries.
    const GLADIATOR: i32 = 2;
    const HUMAN: i32 = 0;

    const NPCS: &str = r#"
        <npc id="36481" type="Folk" level="1" name="Mercenary Captain"/>"#;

    const ITEMS: &str = r#"
        <item id="13766" type="EtcItem" name="Ordinary Certificate"><set name="is_questitem" val="true"/></item>
        <item id="13767" type="EtcItem" name="Elite Certificate"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00147PathToBecomingAnEliteMercenary);
        let pages = Pages::of(&Q00147PathToBecomingAnEliteMercenary)
            .page("elite-01.htm", &["elite-02.htm"])
            .page("elite-02.htm", &["elite-03.htm"])
            .page("elite-03.htm", &["elite-04.htm"])
            .page("elite-02a.htm", &["elite-03.htm"])
            .plain(&["castle.htm", "elite-04.htm", "elite-05.htm", "elite-06.htm"])
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

    /// The lord of the castle clan.
    fn lord_of_the_castle(id: i32) -> PlayerData {
        let mut data = player_data(id, 40, vec![]);
        data.race = HUMAN;
        data.class_id = GLADIATOR;
        data.base_class = GLADIATOR;
        data.profile.clan = Some(PlayerClanState {
            clan_id: LIONS,
            privileges: ClanPrivilegesRef::Own(0x00FF_FFFF),
            ..PlayerClanState::default()
        });
        data
    }

    #[test]
    fn a_clan_with_a_castle_is_sent_to_the_castle_page() {
        let (mut t, merc) = setup();
        t.enter_with(lord_of_the_castle(2));
        // The lord's captain page (36481-04) has no quest link in the datapack, so the hook is
        // called directly: the castle check of Java's `onTalk`.
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(merc) {
                page = Q00147PathToBecomingAnEliteMercenary.on_talk(ctx, npc, Oid(2));
            }
        });
        assert_eq!(page.as_deref(), Some("castle.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_certificate_and_the_start() {
        let (mut t, merc) = setup();
        t.enter_as(1, 40, HUMAN, GLADIATOR, vec![]);
        assert!(t.quest_page(1, merc).contains("elite-01.htm"));
        assert!(t.event(1, NAME, "elite-02.htm").contains("elite-02.htm"));
        assert_eq!(t.count(1, ORDINARY_CERTIFICATE), 1);
        assert!(t.take_writes().is_empty());

        // A second ask with the certificate in hand goes to page 02a and gives nothing more.
        assert!(t.quest_page(1, merc).contains("elite-01.htm"));
        assert!(t.event(1, NAME, "elite-02.htm").contains("elite-02a.htm"));
        assert_eq!(t.count(1, ORDINARY_CERTIFICATE), 1);

        // The page 02a links page 03, which links the start.
        assert!(t.event(1, NAME, "elite-03.htm").contains("elite-03.htm"));
        assert!(t.event(1, NAME, "elite-04.htm").contains("elite-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Cond below 4: the captain says to wait.
        assert!(t.quest_page(1, merc).contains("elite-05.htm"));
    }

    #[test]
    fn the_elite_certificate_at_cond_4() {
        let (mut t, merc) = setup();
        let mut data = player_data(1, 40, vec![item(5, ORDINARY_CERTIFICATE, 1)]);
        data.class_id = GLADIATOR;
        data.base_class = GLADIATOR;
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("4".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, merc).contains("elite-06.htm"));
        assert_eq!(t.count(1, ORDINARY_CERTIFICATE), 0);
        assert_eq!(t.count(1, ELITE_CERTIFICATE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
