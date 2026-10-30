//! Master of Enchanting: Master Yogi sells the staff and the scrolls and pays for the enchant level.
// Java: dist/game/data/scripts/events/MasterOfEnchanting/MasterOfEnchanting.java
// Java: dist/game/data/scripts/events/MasterOfEnchanting/config.xml

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::long_time_event::{self, DropSpec, EventConfig, SpawnSpec};

// NPC
const MASTER_YOGI: i32 = 32599;

// Items
const MASTER_YOGI_STAFF: i32 = 13539;
const MASTER_YOGI_SCROLL: i32 = 13540;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Misc
const STAFF_PRICE: i64 = 1_000_000;
const SCROLL_24_PRICE: i64 = 5_000_000;
const SCROLL_24_TIME: i64 = 6;
const SCROLL_1_PRICE: i64 = 500_000;
const SCROLL_10_PRICE: i64 = 5_000_000;

const HAT_SHADOW_REWARD: [i32; 3] = [13074, 13075, 13076];
const HAT_EVENT_REWARD: [i32; 3] = [13518, 13519, 13522];
const CRYSTAL_REWARD: [i32; 3] = [9570, 9571, 9572];

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Master of Enchanting",
    active: "27 02 2010-28 02 2010",
    drop_period: None,
    drops: &[DropSpec {
        item: 13540,
        min: 1,
        max: 1,
        chance: 1.0,
        min_level: 1,
        max_level: i32::MAX,
        monster_ids: &[],
    }],
    spawns: &[
        SpawnSpec::at(32599, 16111, 142850, -2707, 16000, 0),
        SpawnSpec::at(32599, 17275, 145000, -3037, 25000, 0),
        SpawnSpec::at(32599, 83037, 149324, -3470, 44000, 0),
        SpawnSpec::at(32599, 82145, 148609, -3468, 0, 0),
        SpawnSpec::at(32599, 81755, 146487, -3534, 32768, 0),
        SpawnSpec::at(32599, -81031, 150038, -3045, 0, 0),
        SpawnSpec::at(32599, -83156, 150994, -3130, 0, 0),
        SpawnSpec::at(32599, -13727, 122117, -2990, 16384, 0),
        SpawnSpec::at(32599, -14129, 123869, -3118, 40959, 0),
        SpawnSpec::at(32599, -84411, 244813, -3730, 57343, 0),
        SpawnSpec::at(32599, -84023, 243051, -3730, 4096, 0),
        SpawnSpec::at(32599, 46908, 50856, -2997, 8192, 0),
        SpawnSpec::at(32599, 45538, 48357, -3061, 18000, 0),
        SpawnSpec::at(32599, 9929, 16324, -4576, 62999, 0),
        SpawnSpec::at(32599, 11546, 17599, -4586, 46900, 0),
        SpawnSpec::at(32599, 81987, 53723, -1497, 0, 0),
        SpawnSpec::at(32599, 81083, 56118, -1562, 32768, 0),
        SpawnSpec::at(32599, 147200, 25614, -2014, 16384, 0),
        SpawnSpec::at(32599, 148557, 26806, -2206, 32768, 0),
        SpawnSpec::at(32599, 117356, 76708, -2695, 49151, 0),
        SpawnSpec::at(32599, 115887, 76382, -2714, 0, 0),
        SpawnSpec::at(32599, -117239, 46842, 367, 49151, 0),
        SpawnSpec::at(32599, -119494, 44882, 367, 24576, 0),
        SpawnSpec::at(32599, 111004, 218928, -3544, 16384, 0),
        SpawnSpec::at(32599, 108426, 221876, -3600, 49151, 0),
        SpawnSpec::at(32599, -45278, -112766, -241, 0, 0),
        SpawnSpec::at(32599, -45372, -114104, -241, 16384, 0),
        SpawnSpec::at(32599, 115096, -178370, -891, 0, 0),
        SpawnSpec::at(32599, 116199, -182694, -1506, 0, 0),
        SpawnSpec::at(32599, 86865, -142915, -1341, 26000, 0),
        SpawnSpec::at(32599, 85584, -142490, -1343, 0, 0),
        SpawnSpec::at(32599, 147421, -55435, -2736, 49151, 0),
        SpawnSpec::at(32599, 148206, -55786, -2782, 61439, 0),
        SpawnSpec::at(32599, 43165, -48461, -797, 17000, 0),
        SpawnSpec::at(32599, 43966, -47709, -798, 49999, 0),
    ],
    on_enter_msg: "Master of enchanting: Event ongoing! Visit Master Yogi!",
    on_end_msg: "Master of Enchantig: event end!",
    destroy_items_on_end: &[],
};

/// Java `getRandom(3)` as an index of a three item array.
fn pick(ctx: &mut ScriptCtx<'_>, items: &[i32; 3]) -> i32 {
    items[usize::try_from(ctx.get_random(3)).unwrap_or(0).min(2)]
}

/// Java `events.MasterOfEnchanting.MasterOfEnchanting`.
pub struct MasterOfEnchanting;

impl Script for MasterOfEnchanting {
    fn name(&self) -> &'static str {
        "MasterOfEnchanting"
    }

    fn path(&self) -> &'static str {
        "events/MasterOfEnchanting"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_YOGI]);
        s.add_first_talk_id(&[MASTER_YOGI]);
        s.add_talk_id(&[MASTER_YOGI]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        long_time_event::load(ctx, &CONFIG);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if long_time_event::on_event(ctx, &CONFIG, event, npc) {
            return None;
        }
        let player = player?;
        let mut htmltext = Some(event.to_owned());
        if event.eq_ignore_ascii_case("buy_staff") {
            if !ctx.owns_at_least_one_item(player, &[MASTER_YOGI_STAFF])
                && ctx.get_quest_items_count(player, ADENA_ID) > STAFF_PRICE
            {
                ctx.take_items(player, ADENA_ID, STAFF_PRICE);
                ctx.give_items(player, MASTER_YOGI_STAFF, 1);
                htmltext = Some("32599-staffbuyed.htm".to_owned());
            } else {
                htmltext = Some("32599-staffcant.htm".to_owned());
            }
        } else if event.eq_ignore_ascii_case("buy_scroll_24") {
            // NOTE(java-quirk): `player.getCreateDate().after(getStartDate())` is a `Calendar`
            // compared with a `Date`: `Calendar.after` answers false for anything that is not a
            // `Calendar`, so "32599-birth.htm" is never shown.
            let value = ctx
                .player_variable(player, "MasterOfEnchanting")
                .unwrap_or_default();
            // NOTE(java-quirk): `Long.parseLong` throws on a value that is not a number; it is
            // read as no reuse here.
            let reuse: i64 = if value.is_empty() {
                0
            } else {
                value.parse().unwrap_or(0)
            };
            let cur_time = ctx.now_ms();
            if cur_time > reuse {
                if ctx.get_quest_items_count(player, ADENA_ID) > SCROLL_24_PRICE {
                    buy_24(ctx, player);
                    htmltext = Some("32599-scroll24.htm".to_owned());
                } else {
                    htmltext = Some("32599-s24-no.htm".to_owned());
                }
            } else {
                let remaining_time = (reuse - cur_time) / 1000;
                // NOTE(java-quirk): `(int) remainingTime / 3600` casts first, then divides.
                let hours = remaining_time as i32 / 3600;
                let minutes = (remaining_time as i32 % 3600) / 60;
                if hours > 0 {
                    let sm = SystemMessage::new(
                        SystemMessageId::ThereAreS1HoursSAndS2MinuteSRemainingUntilTheItemCanBePurchasedAgain,
                    )
                    .with(SystemMessageParam::IntNumber(hours))
                    .with(SystemMessageParam::IntNumber(minutes));
                    ctx.world().send(player, &sm);
                    htmltext = Some("32599-scroll24.htm".to_owned());
                } else if minutes > 0 {
                    let sm = SystemMessage::new(
                        SystemMessageId::ThereAreS1MinuteSRemainingUntilTheItemCanBePurchasedAgain,
                    )
                    .with(SystemMessageParam::IntNumber(minutes));
                    ctx.world().send(player, &sm);
                    htmltext = Some("32599-scroll24.htm".to_owned());
                }
                // Little glitch. There is no SystemMessage with seconds only.
                // If time is less than 1 minute player can buy scrolls
                else if ctx.get_quest_items_count(player, ADENA_ID) > SCROLL_24_PRICE {
                    buy_24(ctx, player);
                    htmltext = Some("32599-scroll24.htm".to_owned());
                } else {
                    htmltext = Some("32599-s24-no.htm".to_owned());
                }
            }
        } else if event.eq_ignore_ascii_case("buy_scroll_1") {
            if ctx.get_quest_items_count(player, ADENA_ID) > SCROLL_1_PRICE {
                ctx.take_items(player, ADENA_ID, SCROLL_1_PRICE);
                ctx.give_items(player, MASTER_YOGI_SCROLL, 1);
                htmltext = Some("32599-scroll-ok.htm".to_owned());
            } else {
                htmltext = Some("32599-s1-no.htm".to_owned());
            }
        } else if event.eq_ignore_ascii_case("buy_scroll_10") {
            if ctx.get_quest_items_count(player, ADENA_ID) > SCROLL_10_PRICE {
                ctx.take_items(player, ADENA_ID, SCROLL_10_PRICE);
                ctx.give_items(player, MASTER_YOGI_SCROLL, 10);
                htmltext = Some("32599-scroll-ok.htm".to_owned());
            } else {
                htmltext = Some("32599-s10-no.htm".to_owned());
            }
        } else if event.eq_ignore_ascii_case("receive_reward") {
            // `getItemEquipped(player, Inventory.PAPERDOLL_RHAND) == MASTER_YOGI_STAFF`
            let in_hand = ctx.active_weapon_item_id(player);
            if in_hand == Some(MASTER_YOGI_STAFF) && ctx.get_enchant_level(player, MASTER_YOGI_STAFF) > 3 {
                match ctx.get_enchant_level(player, MASTER_YOGI_STAFF) {
                    4 => {
                        ctx.give_items(player, 6406, 1); // Firework
                    }
                    5 => {
                        ctx.give_items(player, 6406, 2); // Firework
                        ctx.give_items(player, 6407, 1); // Large Firework
                    }
                    6 => {
                        ctx.give_items(player, 6406, 3); // Firework
                        ctx.give_items(player, 6407, 2); // Large Firework
                    }
                    7 => {
                        let hat = pick(ctx, &HAT_SHADOW_REWARD);
                        ctx.give_items(player, hat, 1);
                    }
                    8 => {
                        ctx.give_items(player, 955, 1); // Scroll: Enchant Weapon (D)
                    }
                    9 => {
                        ctx.give_items(player, 955, 1); // Scroll: Enchant Weapon (D)
                        ctx.give_items(player, 956, 1); // Scroll: Enchant Armor (D)
                    }
                    10 => {
                        ctx.give_items(player, 951, 1); // Scroll: Enchant Weapon (C)
                    }
                    11 => {
                        ctx.give_items(player, 951, 1); // Scroll: Enchant Weapon (C)
                        ctx.give_items(player, 952, 1); // Scroll: Enchant Armor (C)
                    }
                    12 => {
                        ctx.give_items(player, 948, 1); // Scroll: Enchant Armor (B)
                    }
                    13 => {
                        ctx.give_items(player, 729, 1); // Scroll: Enchant Weapon (A)
                    }
                    14 => {
                        let hat = pick(ctx, &HAT_EVENT_REWARD);
                        ctx.give_items(player, hat, 1);
                    }
                    15 => {
                        ctx.give_items(player, 13992, 1); // Grade S Accessory Chest (Event)
                    }
                    16 => {
                        ctx.give_items(player, 8762, 1); // Top-Grade Life Stone: level 76
                    }
                    17 => {
                        ctx.give_items(player, 959, 1); // Scroll: Enchant Weapon (S)
                    }
                    18 => {
                        ctx.give_items(player, 13991, 1); // Grade S Armor Chest (Event)
                    }
                    19 => {
                        ctx.give_items(player, 13990, 1); // Grade S Weapon Chest (Event)
                    }
                    20 => {
                        // Red/Blue/Green Soul Crystal - Stage 14
                        let crystal = pick(ctx, &CRYSTAL_REWARD);
                        ctx.give_items(player, crystal, 1);
                    }
                    21 => {
                        ctx.give_items(player, 8762, 1); // Top-Grade Life Stone: level 76
                        ctx.give_items(player, 8752, 1); // High-Grade Life Stone: level 76
                        // Red/Blue/Green Soul Crystal - Stage 14
                        let crystal = pick(ctx, &CRYSTAL_REWARD);
                        ctx.give_items(player, crystal, 1);
                    }
                    22 => {
                        ctx.give_items(player, 13989, 1); // S80 Grade Armor Chest (Event)
                    }
                    23 => {
                        ctx.give_items(player, 13988, 1); // S80 Grade Weapon Chest (Event)
                    }
                    _ => {
                        if ctx.get_enchant_level(player, MASTER_YOGI_STAFF) > 23 {
                            ctx.give_items(player, 13988, 1); // S80 Grade Weapon Chest (Event)
                        }
                    }
                }

                ctx.take_items(player, MASTER_YOGI_STAFF, 1);
                htmltext = Some("32599-rewardok.htm".to_owned());
            } else {
                htmltext = Some("32599-rewardnostaff.htm".to_owned());
            }
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.htm", npc.id))
    }
}

/// The two identical purchases of `buy_scroll_24`.
fn buy_24(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.take_items(player, ADENA_ID, SCROLL_24_PRICE);
    ctx.give_items(player, MASTER_YOGI_SCROLL, 24);
    let until = ctx.now_ms() + SCROLL_24_TIME * 3_600_000;
    ctx.set_player_variable(player, "MasterOfEnchanting", &until.to_string());
}

#[cfg(test)]
mod tests {
    use l2_protocol_game::server::charscreen::paperdoll;
    use l2_world::{ItemData, ScriptRegistry};

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32599" type="Folk" level="70" name="Master Yogi"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13539" type="Weapon" name="Master Yogi's Staff"><set name="bodypart" val="rhand"/></item>
        <item id="13540" type="EtcItem" name="Master Yogi's Scroll Enchant Weapon"><set name="is_stackable" val="true"/></item>
        <item id="6406" type="EtcItem" name="Firework"><set name="is_stackable" val="true"/></item>
        <item id="6407" type="EtcItem" name="Large Firework"><set name="is_stackable" val="true"/></item>
        <item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D)"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"><set name="is_stackable" val="true"/></item>
        <item id="13988" type="EtcItem" name="S80 Grade Weapon Chest"><set name="is_stackable" val="true"/></item>
        <item id="13074" type="EtcItem" name="Hat 1"/>
        <item id="13075" type="EtcItem" name="Hat 2"/>
        <item id="13076" type="EtcItem" name="Hat 3"/>"#;
    /// 2010-02-27 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_267_272_000_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(MasterOfEnchanting);
        let pages = Pages::of(&MasterOfEnchanting)
            .page(
                "32599.htm",
                &[
                    "buy_staff",
                    "buy_scroll_24",
                    "buy_scroll_1",
                    "buy_scroll_10",
                    "receive_reward",
                ],
            )
            .npcs(&[MASTER_YOGI]);
        // Every answer page has the same links as the menu, so a test can click on.
        let links = [
            "buy_staff",
            "buy_scroll_24",
            "buy_scroll_1",
            "buy_scroll_10",
            "receive_reward",
        ];
        let pages = [
            "32599-staffbuyed.htm",
            "32599-staffcant.htm",
            "32599-scroll24.htm",
            "32599-s24-no.htm",
            "32599-scroll-ok.htm",
            "32599-s1-no.htm",
            "32599-s10-no.htm",
            "32599-rewardok.htm",
            "32599-rewardnostaff.htm",
        ]
        .iter()
        .fold(pages, |pages, file| pages.page(file, &links));
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    fn staff(enchant: i32) -> ItemData {
        ItemData {
            loc: "PAPERDOLL".to_owned(),
            loc_data: paperdoll::RHAND as i32,
            enchant_level: enchant,
            ..item(2000, MASTER_YOGI_STAFF, 1)
        }
    }

    fn at_yogi(t: &mut ScriptTest, player: i32) {
        let yogi = t.spawn(MASTER_YOGI, 20);
        t.talk(player, yogi);
    }

    #[test]
    fn inside_the_dates_the_yogis_stand_and_outside_they_do_not() {
        assert_eq!(setup(Some(INSIDE)).npcs_of(MASTER_YOGI).len(), 35);
        assert!(setup(None).npcs_of(MASTER_YOGI).is_empty());
    }

    #[test]
    fn the_staff_is_sold_once() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 2_000_001)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_staff")
                .contains("32599-staffbuyed.htm")
        );
        assert_eq!(t.count(1, MASTER_YOGI_STAFF), 1);
        assert_eq!(t.count(1, ADENA_ID), 1_000_001);
        // Owning one already: no second staff.
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_staff")
                .contains("32599-staffcant.htm")
        );
        assert_eq!(t.count(1, MASTER_YOGI_STAFF), 1);
    }

    #[test]
    fn the_small_scrolls_cost_more_than_the_price_in_adena() {
        let mut t = setup(Some(INSIDE));
        // Java asks for more than the price (`>`), so exactly 500,000 is not enough.
        t.enter(1, 40, vec![item(1000, ADENA_ID, SCROLL_1_PRICE)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_scroll_1")
                .contains("32599-s1-no.htm")
        );
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_scroll_10")
                .contains("32599-s10-no.htm")
        );

        t.enter(2, 40, vec![item(1001, ADENA_ID, 6_000_000)]);
        at_yogi(&mut t, 2);
        assert!(
            t.event(2, "MasterOfEnchanting", "buy_scroll_1")
                .contains("32599-scroll-ok.htm")
        );
        assert_eq!(t.count(2, MASTER_YOGI_SCROLL), 1);
        assert!(
            t.event(2, "MasterOfEnchanting", "buy_scroll_10")
                .contains("32599-scroll-ok.htm")
        );
        assert_eq!(t.count(2, MASTER_YOGI_SCROLL), 11);
    }

    #[test]
    fn the_24_scrolls_wait_six_hours() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 11_000_000)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_scroll_24")
                .contains("32599-scroll24.htm")
        );
        assert_eq!(t.count(1, MASTER_YOGI_SCROLL), 24);
        assert!(
            t.world
                .player(ObjectId(1))
                .and_then(|p| p.data.profile.variables.get("MasterOfEnchanting").cloned())
                .is_some()
        );
        // Again: the page is the same, the message tells the hours, nothing is sold.
        let (page, names) = crate::events::click(&mut t, 1, "MasterOfEnchanting", "buy_scroll_24");
        assert!(page.contains("32599-scroll24.htm"));
        assert_eq!(t.count(1, MASTER_YOGI_SCROLL), 24);
        assert!(names.contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn without_the_adena_there_are_no_24_scrolls() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 5_000_000)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "buy_scroll_24")
                .contains("32599-s24-no.htm")
        );
    }

    #[test]
    fn the_staff_pays_by_its_enchant_level() {
        let mut t = setup(Some(INSIDE));
        // +6: three fireworks and two large ones, and the staff is gone.
        t.enter(1, 40, vec![staff(6)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "receive_reward")
                .contains("32599-rewardok.htm")
        );
        assert_eq!(t.count(1, 6406), 3);
        assert_eq!(t.count(1, 6407), 2);
        assert_eq!(t.count(1, MASTER_YOGI_STAFF), 0);

        // +9: two scrolls.
        t.enter(2, 40, vec![staff(9)]);
        at_yogi(&mut t, 2);
        t.event(2, "MasterOfEnchanting", "receive_reward");
        assert_eq!((t.count(2, 955), t.count(2, 956)), (1, 1));

        // +7: one of the three shadow hats.
        t.enter(3, 40, vec![staff(7)]);
        at_yogi(&mut t, 3);
        t.event(3, "MasterOfEnchanting", "receive_reward");
        assert_eq!(HAT_SHADOW_REWARD.iter().map(|h| t.count(3, *h)).sum::<i64>(), 1);

        // Above +23 the last chest.
        t.enter(4, 40, vec![staff(30)]);
        at_yogi(&mut t, 4);
        t.event(4, "MasterOfEnchanting", "receive_reward");
        assert_eq!(t.count(4, 13988), 1);
    }

    #[test]
    fn a_low_or_unworn_staff_gets_nothing() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![staff(3)]);
        at_yogi(&mut t, 1);
        assert!(
            t.event(1, "MasterOfEnchanting", "receive_reward")
                .contains("32599-rewardnostaff.htm")
        );
        assert_eq!(t.count(1, MASTER_YOGI_STAFF), 1);

        // In the bag, not in the hand.
        let mut bag = item(2001, MASTER_YOGI_STAFF, 1);
        bag.enchant_level = 10;
        t.enter(2, 40, vec![bag]);
        at_yogi(&mut t, 2);
        assert!(
            t.event(2, "MasterOfEnchanting", "receive_reward")
                .contains("32599-rewardnostaff.htm")
        );
    }
}
