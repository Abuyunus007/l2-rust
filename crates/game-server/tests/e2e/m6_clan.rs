//! Scenario: a clan. The leader (level 10, with the SP and adena the first levels cost) creates a
//! clan at the village master, invites the member, raises the clan to level 3 and registers a
//! crest, which the leader then asks for.

use l2_protocol_game::server::{AskJoinPledge, NpcHtmlMessage, NpcInfo, PledgeCrest};

use super::*;

const CLAN_NAME: &str = "Raiders";
/// Java `Clan.levelUpClan`: levels 1 and 2 cost SP and adena, level 3 SP and a Blood Mark.
const LEVEL_COSTS: [(i64, i64); 2] = [(20_000, 650_000), (100_000, 2_500_000)];
const LEVEL_3_SP: i64 = 350_000;
/// A crest is 256 bytes (a 16x12 image).
const CREST: [u8; 256] = [7; 256];

pub struct Played {
    left_a: Left,
    left_b: Left,
    clans: Vec<ClanDbRow>,
    crests: Vec<CrestDbRow>,
    clans_before_crest: Vec<ClanDbRow>,
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));
    let entered_a = h.enter(Who::A).await;
    let master = all::<NpcInfo>(&entered_a, "NPC_INFO")
        .iter()
        .find(|n| n.display_id == m6_data::MASTER_ID)
        .expect("the village master is in view")
        .object_id;
    h.enter(Who::B).await;
    h.collect_quiet(Who::A).await;

    // ---- the master's page: the link carries the name the player typed ($clan_name) -----------------------
    h.act(Who::A, act::click(master)).await;
    h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED")).await;
    h.act(Who::A, act::click(master)).await;
    let page = h.collect_until(Who::A, &|body| is_named(body, "NPC_HTML_MESSAGE")).await;
    let page: NpcHtmlMessage = all(&page, "NPC_HTML_MESSAGE").remove(0);
    assert!(page.html.contains(&format!("npc_{master}_create_clan $clan_name")), "{}", page.html);

    // ---- creating the clan ---------------------------------------------------------------------------------
    h.act(Who::A, act::bypass(&format!("npc_{master}_create_clan {CLAN_NAME}"))).await;
    let created = h.collect_quiet(Who::A).await;
    // Java `ClanTable.createClan`, at its end.
    assert_in_order(
        &names(&created),
        &[
            "PLEDGE_SHOW_INFO_UPDATE",
            "PLEDGE_SHOW_MEMBER_LIST_ALL",
            "USER_INFO",
            "PLEDGE_SHOW_MEMBER_LIST_UPDATE",
            "SYSTEM_MESSAGE",
        ],
    );
    assert_eq!(message_ids(&created), [SystemMessageId::YourClanHasBeenCreated]);
    let (clans, _) = h.clan_rows().await;
    assert_eq!(clans.len(), 1, "{clans:?}");
    let clan_id = clans[0].clan_id;
    // The row of `Clan.store()`: level 0, the creator as leader, no crest.
    assert_eq!(
        clans[0],
        ClanDbRow {
            clan_id,
            name: CLAN_NAME.to_owned(),
            level: 0,
            leader_id: a,
            crest_id: 0
        }
    );

    // ---- the invitation: B is asked, answers yes and is in -----------------------------------------------------
    h.act(Who::A, act::join_pledge(b, 0)).await;
    let asked = h.collect_quiet(Who::B).await;
    let ask: AskJoinPledge = all(&asked, "ASK_JOIN_PLEDGE").remove(0);
    assert_eq!(
        (ask.requestor_object_id, ask.pledge_type, ask.pledge_name.as_str()),
        (a, 0, CLAN_NAME)
    );
    h.collect_quiet(Who::A).await;
    h.act(Who::B, act::answer_join_pledge(1)).await;
    let joined_a = h.collect_quiet(Who::A).await;
    let joined_b = h.collect_quiet(Who::B).await;
    assert_in_order(&names(&joined_a), &["SYSTEM_MESSAGE", "PLEDGE_SHOW_MEMBER_LIST_ADD"]);
    assert_eq!(
        message_of(&joined_a, SystemMessageId::S1HasJoinedTheClan).params,
        [text(NAME_B)]
    );
    assert_in_order(
        &names(&joined_b),
        &["JOIN_PLEDGE", "PLEDGE_SHOW_MEMBER_LIST_ALL", "USER_INFO"],
    );
    assert!(message_ids(&joined_b).contains(&SystemMessageId::EnteredTheClan));

    // ---- the levels: SP and adena for 1 and 2, SP and a Blood Mark for 3 ---------------------------------------------
    for (level, (sp, adena)) in LEVEL_COSTS.iter().enumerate() {
        h.act(Who::A, act::bypass(&format!("npc_{master}_increase_clan_level"))).await;
        let up = h.collect_quiet(Who::A).await;
        let messages = system_messages(&up);
        assert_eq!(
            messages.iter().map(|m| (m.id, m.params.clone())).collect::<Vec<_>>(),
            [
                (
                    SystemMessageId::S1AdenaDisappeared,
                    vec![SystemMessageParam::LongNumber(*adena)]
                ),
                (
                    SystemMessageId::YourSpHasDecreasedByS1,
                    vec![SystemMessageParam::IntNumber(i32::try_from(*sp).unwrap())]
                ),
                (SystemMessageId::YourClanSLevelHasIncreased, vec![]),
            ],
            "level {}",
            level + 1
        );
        assert!(names(&up).contains(&"PLEDGE_SHOW_INFO_UPDATE".to_owned()));
        // The level-up effect (skill 5103) is played for everybody who sees the leader.
        assert!(names(&up).contains(&"MAGIC_SKILL_USE".to_owned()));
    }
    h.act(Who::A, act::bypass(&format!("npc_{master}_increase_clan_level"))).await;
    let up = h.collect_quiet(Who::A).await;
    assert_eq!(
        system_messages(&up)
            .iter()
            .map(|m| (m.id, m.params.clone()))
            .collect::<Vec<_>>(),
        [
            (
                SystemMessageId::YourSpHasDecreasedByS1,
                vec![SystemMessageParam::IntNumber(i32::try_from(LEVEL_3_SP).unwrap())]
            ),
            (
                SystemMessageId::S1HasDisappeared,
                vec![SystemMessageParam::ItemName(m6_data::BLOOD_MARK)]
            ),
            (SystemMessageId::YourClanSLevelHasIncreased, vec![]),
        ]
    );
    let (clans_before_crest, _) = h.clan_rows().await;

    // ---- the crest: allowed from level 3; the leader asks for the image ----------------------------------------------
    h.act(Who::A, act::set_pledge_crest(&CREST)).await;
    let registered = h.collect_quiet(Who::A).await;
    assert!(message_ids(&registered).contains(&SystemMessageId::TheCrestWasSuccessfullyRegistered));
    let (clans_now, crests_now) = h.clan_rows().await;
    let crest_id = crests_now.first().expect("the crest was stored").crest_id;
    assert_eq!(clans_now[0].crest_id, crest_id);
    h.act(Who::A, act::pledge_crest(crest_id)).await;
    let asked_crest = h.collect_quiet(Who::A).await;
    let image: PledgeCrest = all(&asked_crest, "PLEDGE_CREST").remove(0);
    assert_eq!((image.crest_id, image.data.as_slice()), (crest_id, &CREST[..]));

    let (clans, crests) = h.clan_rows().await;
    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    assert_eq!(clans[0].clan_id, clan_id);
    Played {
        left_a,
        left_b,
        clans,
        crests,
        clans_before_crest,
    }
}

pub fn assert_played(played: &Played, a: i32) {
    let clan = &played.clans[0];
    assert_eq!((clan.name.as_str(), clan.level, clan.leader_id), (CLAN_NAME, 3, a));
    assert_eq!(played.clans_before_crest[0].level, 3);
    assert_eq!(played.clans_before_crest[0].crest_id, 0);
    // D-39: the clan has an id of the Rust range.
    assert!(clan.clan_id >= l2_world::RUST_FIRST_OBJECT_ID, "{}", clan.clan_id);
    // `CrestTable.createCrest`: the first crest is 1, type 1 is the clan crest.
    assert_eq!(
        played.crests,
        [CrestDbRow {
            crest_id: 1,
            data: CREST.to_vec(),
            crest_type: 1
        }]
    );
    assert_eq!(clan.crest_id, 1);

    // The characters: both belong to the clan; the leader paid the levels.
    let (leader, member) = (&played.left_a, &played.left_b);
    assert_eq!((leader.clan_id, member.clan_id), (clan.clan_id, clan.clan_id));
    let sp_paid: i64 = LEVEL_COSTS.iter().map(|(sp, _)| sp).sum::<i64>() + LEVEL_3_SP;
    let adena_paid: i64 = LEVEL_COSTS.iter().map(|(_, adena)| adena).sum();
    assert_eq!(leader.sp, m6_data::LEADER_SP - sp_paid);
    assert_eq!(leader.count_of(ADENA_ID), m6_data::START_ADENA - adena_paid);
    assert_eq!(leader.count_of(m6_data::BLOOD_MARK), m6_data::BLOOD_MARKS_AT_START - 1);
    assert_eq!((leader.level, member.level), (m6_data::LEADER_LEVEL, 1));
    assert_eq!(member.sp, 0);
    assert_eq!(member.count_of(ADENA_ID), m6_data::START_ADENA);
}

#[tokio::test]
async fn a_clan_is_created_joined_levelled_and_given_a_crest_in_a_bare_world() {
    let mut world = BareWorld::start(true);
    let a = world.object_id(Who::A);
    let played = play(&mut world).await;
    assert_played(&played, a);
}

#[tokio::test]
async fn a_clan_is_created_joined_levelled_and_given_a_crest_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6c", true).await else {
        return;
    };
    let a = net.object_id(Who::A);
    let played = play(&mut net).await;
    assert_played(&played, a);
    net.finish().await;
}
