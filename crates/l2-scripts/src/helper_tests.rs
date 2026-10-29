//! Tests of the `ScriptCtx` helpers of S-DATA-01 (`l2_world::script::player_help`), run through a
//! probe script that calls them from an event the way a quest would.

use l2_util::ObjectId as Oid;
use l2_world::{PlayerClanState, Script, ScriptCtx, ScriptNpc, ScriptRegistry, ScriptSetup};

use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, player_data};

const NPC_ID: i32 = 31_000;
const NPCS: &str = r#"<npc id="31000" type="Folk" level="70" name="Probe"/>"#;
const ITEMS: &str =
    r#"<item id="10280" type="EtcItem" name="Certificate"><set name="is_stackable" val="true"/></item>"#;

struct Probe;

impl Script for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }

    fn path(&self) -> &'static str {
        "test/Probe"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NPC_ID]);
        s.add_talk_id(&[NPC_ID]);
        s.add_first_talk_id(&[NPC_ID]);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: Oid) -> Option<String> {
        Some("p.html".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<Oid>,
    ) -> Option<String> {
        let player = player?;
        match event {
            "clan" => Some(format!(
                "<html><body>sponsor={} apprentice={} pledge={}</body></html>",
                ctx.player_sponsor(player),
                ctx.player_apprentice(player),
                ctx.player_pledge_type(player)
            )),
            "social" => {
                ctx.send_social_action(player, 3);
                None
            }
            "warehouse" => {
                let before = ctx.private_warehouse_item(player, Oid(900));
                let destroyed = ctx.destroy_private_warehouse_item(player, Oid(900), 1);
                let after = ctx.private_warehouse_item(player, Oid(900));
                let too_many = ctx.destroy_private_warehouse_item(player, Oid(900), 99);
                let missing = ctx.destroy_private_warehouse_item(player, Oid(12345), 1);
                Some(format!(
                    "<html><body>{before:?} {destroyed} {after:?} {too_many} {missing}</body></html>"
                ))
            }
            _ => None,
        }
    }
}

fn setup() -> (ScriptTest, Oid) {
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    let pages = Pages::of(&Probe).page("p.html", &["clan", "social", "warehouse"]);
    let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
    let npc = t.spawn(NPC_ID, 20);
    (t, npc)
}

fn enter(t: &mut ScriptTest, npc: Oid, id: i32, mut data: l2_world::PlayerData) {
    data.object_id = Oid(id);
    t.enter_with(data);
    t.talk(id, npc);
    t.take();
}

#[test]
fn sponsor_apprentice_and_pledge_type_come_from_the_clan_state() {
    let (mut t, npc) = setup();
    // A player without a clan: 0, 0, 0 (`Player._pledgeType` starts at 0).
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    assert!(
        t.event(1, "Probe", "clan")
            .contains("sponsor=0 apprentice=0 pledge=0")
    );
    // An academy member with a sponsor, and its sponsor.
    let mut academy = player_data(2, 20, vec![]);
    academy.profile.clan = Some(PlayerClanState {
        clan_id: 7,
        pledge_type: -1,
        sponsor: 3,
        ..Default::default()
    });
    enter(&mut t, npc, 2, academy);
    assert!(
        t.event(2, "Probe", "clan")
            .contains("sponsor=3 apprentice=0 pledge=-1")
    );
    let mut sponsor = player_data(3, 80, vec![]);
    sponsor.profile.clan = Some(PlayerClanState {
        clan_id: 7,
        apprentice: 2,
        ..Default::default()
    });
    enter(&mut t, npc, 3, sponsor);
    assert!(
        t.event(3, "Probe", "clan")
            .contains("sponsor=0 apprentice=2 pledge=0")
    );
}

#[test]
fn a_social_action_goes_to_the_player_only() {
    let (mut t, npc) = setup();
    enter(&mut t, npc, 1, player_data(1, 80, vec![]));
    enter(&mut t, npc, 2, player_data(2, 80, vec![]));
    t.bypass(1, "Script Probe social");
    let out = t.take();
    assert_eq!(
        names_to(&out, 1).iter().filter(|n| *n == "SOCIAL_ACTION").count(),
        1
    );
    assert!(!names_to(&out, 2).contains(&"SOCIAL_ACTION".to_owned()));
    let packet = out
        .iter()
        .find(|(to, body)| *to == 1 && crate::testing::name_of(body) == "SOCIAL_ACTION")
        .map(|(_, body)| crate::testing::decode::<l2_protocol_game::server::SocialAction>(body))
        .unwrap();
    assert_eq!((packet.object_id, packet.action_id), (1, 3));
}

#[test]
fn a_warehouse_item_is_destroyed_by_count() {
    let (mut t, npc) = setup();
    let mut data = player_data(1, 80, vec![item(901, 10280, 5)]);
    let mut book = item(900, 10280, 2);
    book.loc = "WAREHOUSE".to_owned();
    data.profile.warehouse.private.push(book);
    enter(&mut t, npc, 1, data);
    // 2 -> 1 (`destroyItem` with a smaller count only lowers the stack), then 99 is too many and an
    // unknown object id finds nothing.
    let page = t.event(1, "Probe", "warehouse");
    assert!(
        page.contains("Some((10280, 2)) true Some((10280, 1)) false false"),
        "{page}"
    );
    // The bag is not touched.
    assert_eq!(t.count(1, 10280), 5);
    // The last unit deletes the item.
    assert!(t.world.destroy_private_warehouse_item(Oid(1), Oid(900), 1));
    assert_eq!(t.world.private_warehouse_item(Oid(1), Oid(900)), None);
    let _ = pages_to;
}
