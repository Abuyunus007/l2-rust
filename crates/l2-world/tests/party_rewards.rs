#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! What a party gets from a kill (M6-PARTY-01): `Party.distributeXpAndSp` with the party bonus and
//! the level cut-offs, the loot rules (`distributeItem`, `distributeAdena`), the pickup of a
//! party member's drops and the party sweep.
//!
//! The expected numbers are worked out by hand from `Party.java` and
//! `Attackable.calculateRewards`, not from this code. The monster of the scenes gives 1000 exp and
//! 100 SP at level 10.

mod party_scene;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessageParam;
use l2_world::*;
use party_scene::*;

/// Sure Spoil (always lands) and the Sweeper effect, as `tests/spoil.rs` has them.
const SKILLS: &str = r##"<list>
<skill id="9254" levels="1" name="Sure Spoil"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-138</effectPoint><effectRange>900</effectRange>
<hitTime>500</hitTime><magicLevel>1</magicLevel><mpConsume>3</mpConsume><reuseDelay>0</reuseDelay>
<activateRate>100</activateRate>
<effects><effect name="Spoil" /></effects></skill>
<skill id="42" levels="1" name="Sweeper"><operateType>A1</operateType><targetType>CORPSE_MOB</targetType>
<castRange>600</castRange><effectRange>900</effectRange><hitTime>500</hitTime><isMagic>2</isMagic>
<magicLevel>10</magicLevel><mpConsume>3</mpConsume><reuseDelay>500</reuseDelay>
<effects><effect name="Sweeper" /></effects></skill>
</list>"##;

const SURE_SPOIL: i32 = 9254;
const SWEEPER: i32 = 42;

fn config() -> Config {
    Config {
        skills_xml: SKILLS.into(),
        ..Config::default()
    }
}

/// The exp and SP `who` gained since it entered at `level`.
fn gained(s: &Scene, who: i32, level: i32) -> (i64, i64) {
    let player = s.player(who);
    (
        player.data.exp - EXP[usize::try_from(level).unwrap() - 1],
        player.data.sp,
    )
}

/// Kills the monster, the last hit by `killer`.
fn kill(s: &mut Scene, monster: ObjectId, killer: i32) {
    assert!(s.world.do_die(monster, Some(ObjectId(killer))));
}

/// A party of `levels.len()` players, each hitting for `damage`, and the monster dying.
fn party_kill(config: Config, levels: &[i32], damage: i64) -> Scene {
    let mut s = Scene::with(config);
    let ids: Vec<i32> = (0..levels.len())
        .map(|i| P1 + i32::try_from(i).unwrap())
        .collect();
    let names = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"];
    for (i, level) in levels.iter().enumerate() {
        s.enter_level(ids[i], names[i], *level);
    }
    let members: Vec<(i32, &str)> = ids.iter().zip(names).skip(1).map(|(i, n)| (*i, n)).collect();
    s.form_party(P1, &members, 0);
    let monster = s.spawn_monster();
    for who in &ids {
        s.hit(monster, *who, damage);
    }
    kill(&mut s, monster, P1);
    s
}

// ------------------------------------------------------------------ exp and SP

/// Two level 10 members, each 5 of 10 damage: `getExpBonus(2)` is 1.10 * `RatePartyXp`, and each
/// gets `sqLevel / sqLevelSum` (0.5) of it: 1000 * 1.1 * 0.5 = 550 exp and 100 * 1.1 * 0.5 = 55 SP.
#[test]
fn a_party_of_two_gets_the_ten_percent_bonus() {
    let s = party_kill(config(), &[10, 10], 5);
    for who in [P1, P2] {
        assert_eq!(gained(&s, who, 10), (550, 55), "member {who:x}");
    }
}

/// Three members: the bonus is 1.20, and each takes a third (0.3333333333333333) of the 1200.
#[test]
fn three_members_get_the_twenty_percent_bonus() {
    let s = party_kill(config(), &[10, 10, 10], 4);
    for who in [P1, P2, P3] {
        assert_eq!(gained(&s, who, 10), (400, 40), "member {who:x}");
    }
}

/// `RatePartyXp` and `RatePartySp` multiply the bonus of two or more members, each its own.
#[test]
fn the_party_rates_multiply_the_bonus() {
    let mut c = config();
    c.settings.party.rate_party_xp = 2.0;
    c.settings.party.rate_party_sp = 3.0;
    let s = party_kill(c, &[10, 10], 5);
    // 1000 * (1.10 * 2) * 0.5 = 1100 and 100 * (1.10 * 3) * 0.5 = 165.
    for who in [P1, P2] {
        assert_eq!(gained(&s, who, 10), (1100, 165), "member {who:x}");
    }
}

/// `ExpAmountMultipliers` / `SpAmountMultipliers` of `ClassBalance.ini` scale a member's share.
#[test]
fn the_class_multipliers_scale_the_share() {
    let mut c = config();
    c.settings.party.exp_amount_multipliers[0] = 0.5;
    c.settings.party.sp_amount_multipliers[0] = 2.0;
    let s = party_kill(c, &[10, 10], 5);
    // 550 * 0.5 = 275 and 55.00000000000001 * 2 = 110.
    assert_eq!(gained(&s, P1, 10), (275, 110));
}

/// The party takes its share of the damage once in `calculateExpAndSp` and once more as
/// `partyMul`; a player outside the party gets the plain share.
// NOTE(java-quirk): that makes the party's share smaller than its damage share.
#[test]
fn a_shared_kill_counts_the_party_damage_twice() {
    let mut s = Scene::with(config());
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P4, "Delta");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    s.hit(monster, P2, 5);
    s.hit(monster, P4, 10);
    kill(&mut s, monster, P4);
    // Delta: 1000 * 10 / 20 = 500 exp and 50 SP.
    assert_eq!(gained(&s, P4, 10), (500, 50));
    // The party: 1000 * 10 / 20 = 500, times 10 / 20 = 250 exp and 25 SP; times 1.1 for two
    // members, half each: 137.50000000000003 rounds to 138, and 13.750000000000002 to 14.
    for who in [P1, P2] {
        assert_eq!(gained(&s, who, 10), (138, 14), "member {who:x}");
    }
}

/// A dead member is not rewarded, but its damage stays in the total: the survivors get
/// `partyMul` = their damage over the total, on top of the damage share.
#[test]
fn a_dead_member_gets_nothing_and_costs_the_others() {
    let mut s = Scene::with(config());
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    s.hit(monster, P2, 5);
    s.world.set_dead(id(P2), true);
    kill(&mut s, monster, P1);
    // 1000 * 5 / 10 = 500, times 5 / 10: 250 exp and 25 SP for the only valid member.
    assert_eq!(gained(&s, P1, 10), (250, 25));
    assert_eq!(gained(&s, P2, 10), (0, 0));
}

/// A member beyond `AltPartyRange` of the monster is not part of the kill at all.
#[test]
fn a_member_too_far_away_is_left_out() {
    let mut s = Scene::with(config());
    s.enter(P1, "Alpha");
    s.enter_at(P2, "Bravo", 10, (X + 1_501, Y, Z));
    s.form_party(P1, &[(P2, "Bravo")], 0);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    s.hit(monster, P2, 5);
    kill(&mut s, monster, P1);
    // Bravo's damage is ignored, so Alpha did it all, as the only valid member (no bonus).
    assert_eq!(gained(&s, P1, 10), (1000, 100));
    assert_eq!(gained(&s, P2, 10), (0, 0));
}

fn with_method(method: PartyExpType) -> Config {
    let mut c = config();
    c.settings.party.xp_cutoff_method = method;
    c
}

/// `LEVEL` (the shipped `Player.ini`): a member more than `PartyXpCutoffLevel` levels below the
/// highest one is cut and gets nothing, and does not count for the bonus.
#[test]
fn the_level_cutoff_drops_the_low_member() {
    let mut c = with_method(PartyExpType::Level);
    c.monster_level = 30;
    let s = party_kill(c, &[10, 31], 5);
    // The top level is 31, 21 above Alpha: only Bravo counts, with no bonus: all 1000 exp.
    assert_eq!(gained(&s, P1, 10), (0, 0));
    assert_eq!(gained(&s, P2, 31), (1000, 100));
}

/// `NONE` cuts nobody: the shares are the squares of the levels (100 and 961 of 1061).
#[test]
fn without_a_cutoff_everybody_shares_by_the_squares_of_the_levels() {
    let mut c = with_method(PartyExpType::None);
    c.monster_level = 30;
    let s = party_kill(c, &[10, 31], 5);
    // 1100.0000000000002 * 100 / 1061 = 103.7 and * 961 / 1061 = 996.3.
    assert_eq!(gained(&s, P1, 10), (104, 10));
    assert_eq!(gained(&s, P2, 31), (996, 100));
}

/// `HIGHFIVE`: the gap to the top level picks a percentage (100, 30 or 0 by default).
#[test]
fn the_highfive_cutoff_pays_by_level_gap() {
    // A gap of 10 pays 30 percent: 220 exp and 22.000000000000004 SP are cut to 66 and 7 (6.6).
    let mut c = with_method(PartyExpType::HighFive);
    c.monster_level = 20;
    let s = party_kill(c, &[10, 20], 5);
    assert_eq!(gained(&s, P1, 10), (66, 7));
    assert_eq!(gained(&s, P2, 20), (880, 88));

    // A gap of 15 or more pays nothing.
    let mut c = with_method(PartyExpType::HighFive);
    c.monster_level = 25;
    let s = party_kill(c, &[10, 25], 5);
    assert_eq!(gained(&s, P1, 10), (0, 0));
    assert!(gained(&s, P2, 25).0 > 0);
}

/// `PERCENTAGE` cuts a member whose square of the level is under the percent of the sum, and `AUTO`
/// one under the average square (`sum / n^2`).
#[test]
fn the_percentage_and_auto_cutoffs() {
    // Squares 100 and 1600 (sum 1700): 100 * 100 = 10000 < 1700 * 10.0 cuts Alpha.
    let mut c = with_method(PartyExpType::Percentage);
    c.monster_level = 40;
    c.settings.party.xp_cutoff_percent = 10.0;
    let s = party_kill(c, &[10, 40], 5);
    assert_eq!(gained(&s, P1, 10), (0, 0));
    assert_eq!(gained(&s, P2, 40), (1000, 100));
    // With 3.0 percent both stay.
    let mut c = with_method(PartyExpType::Percentage);
    c.monster_level = 40;
    let s = party_kill(c, &[10, 40], 5);
    assert!(gained(&s, P1, 10).0 > 0 && gained(&s, P2, 40).0 > 0);

    // AUTO: 100 < 1700 / 4 cuts Alpha.
    let mut c = with_method(PartyExpType::Auto);
    c.monster_level = 40;
    let s = party_kill(c, &[10, 40], 5);
    assert_eq!(gained(&s, P1, 10), (0, 0));
    assert_eq!(gained(&s, P2, 40), (1000, 100));
}

/// A level up changes the party level (`PlayerStat.addLevel` -> `recalculatePartyLevel`).
#[test]
fn a_level_up_raises_the_party_level() {
    let mut s = Scene::with(config());
    s.enter_level(P1, "Alpha", 9);
    s.enter_level(P2, "Bravo", 10);
    s.form_party(P1, &[(P2, "Bravo")], 0);
    let party = s.party_of(P1).unwrap();
    assert_eq!(s.world.party_level(party), Some(10));
    // Level 10 needs 48229 exp, level 11 71202: Bravo's 23000 more are a level.
    assert!(s.world.add_exp(id(P2), 71_202 - EXP[9]));
    assert_eq!(s.player(P2).core.level, 11);
    assert_eq!(s.world.party_level(party), Some(11));
}

// ------------------------------------------------------------------ loot: auto loot

fn auto_loot(type_id: i32) -> (Config, i32) {
    let mut c = config();
    c.settings.ground.auto_loot = true;
    (c, type_id)
}

/// A party with `members` players of level 10 (Alpha leads), loot rule `type_id`, the monster
/// dying to hits of 5 from each; the packets of the kill are in `s.log`.
fn loot_party(config: Config, type_id: i32, members: usize) -> Scene {
    let mut s = Scene::with(config);
    let names = ["Alpha", "Bravo", "Charlie", "Delta"];
    let ids: Vec<i32> = (0..members).map(|i| P1 + i32::try_from(i).unwrap()).collect();
    for (i, who) in ids.iter().enumerate() {
        s.enter(*who, names[i]);
    }
    let rest: Vec<(i32, &str)> = ids.iter().zip(names).skip(1).map(|(i, n)| (*i, n)).collect();
    s.form_party(P1, &rest, type_id);
    let monster = s.spawn_monster();
    for who in &ids {
        s.hit(monster, *who, 5);
    }
    s.log.take();
    kill(&mut s, monster, P1);
    s
}

/// Finders keepers: the leader (who owns the drop, as the party that did the most damage) gets
/// every item; the adena of 50 is split between the two members; the other member hears who got
/// what.
#[test]
fn finders_keepers_gives_the_items_to_the_looter_and_splits_the_adena() {
    let (c, t) = auto_loot(0);
    let mut s = loot_party(c, t, 2);
    assert_eq!(s.inventory_count(P1, 1), 1, "the sword");
    assert_eq!(s.inventory_count(P1, 1060), 3, "the potions");
    assert_eq!(s.inventory_count(P2, 1), 0);
    assert_eq!(s.inventory_count(P1, 57), 25);
    assert_eq!(s.inventory_count(P2, 57), 25);
    s.ticks(10);
    let sent = s.log.take();
    let heard: Vec<_> = messages(&sent, P2)
        .into_iter()
        .filter(|m| {
            matches!(
                m.id,
                SystemMessageId::C1HasObtainedS2 | SystemMessageId::C1HasObtainedS3S2
            )
        })
        .collect();
    assert_eq!(heard.len(), 2, "{heard:?}");
    let name = SystemMessageParam::Text("Alpha".into());
    assert!(heard.iter().any(|m| m.id == SystemMessageId::C1HasObtainedS2
        && m.params == vec![name.clone(), SystemMessageParam::ItemName(1)]));
    assert!(heard.iter().any(|m| m.id == SystemMessageId::C1HasObtainedS3S2
        && m.params
            == vec![
                name.clone(),
                SystemMessageParam::ItemName(1060),
                SystemMessageParam::LongNumber(3)
            ]));
    // Each member heard of their own adena.
    for who in [P1, P2] {
        let earned = messages(&sent, who)
            .into_iter()
            .find(|m| m.id == SystemMessageId::YouHaveEarnedS1Adena)
            .unwrap();
        assert_eq!(earned.params, vec![SystemMessageParam::LongNumber(25)]);
    }
}

/// The adena that does not divide goes one by one from a random member on: 50 over 3 is 16, 16, 18
/// as 17 + 17 + 16 in some order.
#[test]
fn leftover_adena_goes_to_some_members() {
    let (c, t) = auto_loot(0);
    let s = loot_party(c, t, 3);
    let shares: Vec<i64> = [P1, P2, P3].iter().map(|w| s.inventory_count(*w, 57)).collect();
    assert_eq!(shares.iter().sum::<i64>(), 50);
    assert!(shares.iter().all(|a| (16..=17).contains(a)), "{shares:?}");
    assert_eq!(shares.iter().filter(|a| **a == 17).count(), 2, "{shares:?}");
}

/// A member out of range of the monster takes no part in the adena.
#[test]
fn adena_goes_only_to_members_in_range() {
    let (c, t) = auto_loot(0);
    let mut s = Scene::with(c);
    s.enter(P1, "Alpha");
    s.enter_at(P2, "Bravo", 10, (X + 2_000, Y, Z));
    s.form_party(P1, &[(P2, "Bravo")], t);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    kill(&mut s, monster, P1);
    assert_eq!(s.inventory_count(P1, 57), 50);
    assert_eq!(s.inventory_count(P2, 57), 0);
}

/// By turn: the next member after the one who got the last item gets the next one, starting with
/// the second member. A member without room is skipped.
#[test]
fn by_turn_hands_the_items_around() {
    let (c, t) = auto_loot(3);
    let s = loot_party(c, t, 2);
    // The first item goes to Bravo, the second to Alpha.
    let alpha = s.inventory_count(P1, 1) + s.inventory_count(P1, 1060);
    let bravo = s.inventory_count(P2, 1) + s.inventory_count(P2, 1060);
    assert!(alpha > 0 && bravo > 0, "one item each: {alpha} {bravo}");
    assert_eq!(s.inventory_count(P1, 1) + s.inventory_count(P2, 1), 1);
    assert_eq!(s.inventory_count(P1, 1060) + s.inventory_count(P2, 1060), 3);
    // The adena is not turned: it is shared.
    assert_eq!(s.inventory_count(P1, 57), 25);
}

#[test]
fn by_turn_skips_a_member_whose_inventory_is_full() {
    let (c, t) = auto_loot(3);
    let mut s = Scene::with(c);
    s.enter(P1, "Alpha");
    let mut bravo = player_data(P2, "Bravo", 10, (X, Y, Z));
    // 80 non-stackable items fill the 80 slots (`MaximumSlotsForNoDwarf`).
    bravo.items = (0..80).map(|i| item(0x2000_0100 + i, 1000, 1)).collect();
    s.enter_data(bravo);
    s.form_party(P1, &[(P2, "Bravo")], t);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    s.hit(monster, P2, 5);
    kill(&mut s, monster, P1);
    assert_eq!(s.inventory_count(P1, 1), 1);
    assert_eq!(s.inventory_count(P1, 1060), 3);
    assert_eq!(s.inventory_count(P2, 1), 0);
}

/// Random: a random member with room gets each item (the world's RNG decides).
#[test]
fn random_gives_each_item_to_a_member() {
    let (c, t) = auto_loot(1);
    let s = loot_party(c, t, 3);
    let total = |item_id: i32| -> i64 { [P1, P2, P3].iter().map(|w| s.inventory_count(*w, item_id)).sum() };
    assert_eq!(total(1), 1);
    assert_eq!(total(1060), 3);
    assert_eq!(total(57), 50);
}

// ------------------------------------------------------------------ loot on the ground

/// `ground_of` for a scene whose monster dropped its loot on the ground.
fn ground_of(s: &Scene, item_id: i32) -> ObjectId {
    s.world
        .ground_item_ids()
        .into_iter()
        .find(|g| s.world.ground_item(*g).unwrap().data.item_id == item_id)
        .unwrap()
}

/// `who` clicks the item and walks to it until it is gone.
fn pick_up(s: &mut Scene, who: i32, item: ObjectId) -> Sent {
    s.input(
        who,
        ClientInput::Action {
            object_id: item.0,
            action_id: 0,
        },
    );
    let mut sent = Vec::new();
    for _ in 0..400 {
        s.world.tick();
        sent.extend(s.log.take());
        if s.world.ground_item(item).is_none() {
            break;
        }
    }
    s.ticks(6);
    sent.extend(s.log.take());
    sent
}

/// The drops of a party's kill belong to its leader, but any member may pick them up; the pickup
/// goes through the loot rule, and the others hear who took it. An outsider is refused.
#[test]
fn a_party_member_can_pick_up_the_leaders_drops() {
    let mut s = loot_party(config(), 0, 2);
    s.enter(P4, "Delta");
    let potions = ground_of(&s, 1060);
    let item = s.world.ground_item(potions).unwrap();
    assert_eq!(item.owner, Some(id(P1)), "the leader has the loot privilege");
    assert_eq!(item.drop_protection.map(|p| p.owner), Some(id(P1)));

    // Delta is not in the party.
    let sent = pick_up_attempt(&mut s, P4, potions);
    assert!(s.world.ground_item(potions).is_some());
    assert!(message_ids(&sent, P4).contains(&SystemMessageId::YouHaveFailedToPickUpS1));

    let sent = pick_up(&mut s, P2, potions);
    assert!(s.world.ground_item(potions).is_none());
    assert_eq!(s.inventory_count(P2, 1060), 3);
    let obtained = messages(&sent, P2)
        .into_iter()
        .find(|m| m.id == SystemMessageId::YouHaveObtainedS2S1)
        .unwrap();
    assert_eq!(
        obtained.params,
        vec![
            SystemMessageParam::ItemName(1060),
            SystemMessageParam::LongNumber(3)
        ]
    );
    let heard = messages(&sent, P1)
        .into_iter()
        .find(|m| m.id == SystemMessageId::C1HasObtainedS3S2)
        .unwrap();
    assert_eq!(
        heard.params,
        vec![
            SystemMessageParam::Text("Bravo".into()),
            SystemMessageParam::ItemName(1060),
            SystemMessageParam::LongNumber(3)
        ]
    );

    // Adena picked up in a party is shared by the members in range.
    let adena = ground_of(&s, 57);
    pick_up(&mut s, P1, adena);
    assert_eq!(s.inventory_count(P1, 57), 25);
    assert_eq!(s.inventory_count(P2, 57), 25);
}

/// One click and a few ticks: for a pickup that is refused, where the item stays.
fn pick_up_attempt(s: &mut Scene, who: i32, item: ObjectId) -> Sent {
    s.input(
        who,
        ClientInput::Action {
            object_id: item.0,
            action_id: 0,
        },
    );
    s.ticks(200);
    s.log.take()
}

// ------------------------------------------------------------------ sweep

/// A party member sweeps what the spoiler of the party spoiled (`isInLooterParty`); with a rule
/// that includes spoil the items go through `distributeItem`, and the others hear it from the
/// sweeper message.
#[test]
fn a_party_sweep_goes_through_the_loot_rule() {
    // BY_TURN_INCLUDING_SPOIL (4): the first item goes to the second member.
    let mut s = loot_party_spoiled(4);
    s.world
        .apply_effects(id(P2), s.monster(), SkillKey::new(SWEEPER, 1));
    let sent = s.log.take();
    assert_eq!(s.inventory_count(P2, 1060), 4, "Bravo is next in turn");
    assert_eq!(s.inventory_count(P1, 1060), 0);
    // The looter hears "You have earned", the leader who spoiled hears who got it by sweeping.
    let earned = messages(&sent, P2)
        .into_iter()
        .find(|m| m.id == SystemMessageId::YouHaveEarnedS2S1S)
        .unwrap();
    assert_eq!(
        earned.params,
        vec![
            SystemMessageParam::ItemName(1060),
            SystemMessageParam::LongNumber(4)
        ]
    );
    let heard = messages(&sent, P1)
        .into_iter()
        .find(|m| m.id == SystemMessageId::C1HasObtainedS3S2ByUsingSweeper)
        .unwrap();
    assert_eq!(
        heard.params,
        vec![
            SystemMessageParam::Text("Bravo".into()),
            SystemMessageParam::ItemName(1060),
            SystemMessageParam::LongNumber(4)
        ]
    );
}

/// Plain by-turn leaves the spoil to the sweeper itself, and a player outside the party has no
/// right to the corpse.
#[test]
fn by_turn_does_not_share_the_spoil_and_strangers_cannot_sweep() {
    let mut s = loot_party_spoiled(3);
    s.enter(P4, "Delta");
    s.world
        .apply_effects(id(P4), s.monster(), SkillKey::new(SWEEPER, 1));
    assert_eq!(s.inventory_count(P4, 1060), 0, "no priority rights");
    s.world
        .apply_effects(id(P2), s.monster(), SkillKey::new(SWEEPER, 1));
    assert_eq!(s.inventory_count(P2, 1060), 4);
    assert_eq!(s.inventory_count(P1, 1060), 0);
}

/// Alpha and Bravo in a party with the given loot rule; Alpha spoiled the monster, both hit it and
/// it died.
fn loot_party_spoiled(type_id: i32) -> Scene {
    let mut s = Scene::with(config());
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], type_id);
    let monster = s.spawn_monster();
    s.world
        .apply_effects(id(P1), monster, SkillKey::new(SURE_SPOIL, 1));
    assert_eq!(s.world.spoiler_of(monster), Some(id(P1)));
    s.hit(monster, P1, 5);
    s.hit(monster, P2, 5);
    kill(&mut s, monster, P1);
    assert!(s.world.is_sweep_active(monster));
    s.log.take();
    s
}

trait MonsterOf {
    fn monster(&self) -> ObjectId;
}

impl MonsterOf for Scene {
    fn monster(&self) -> ObjectId {
        self.world.npc_ids().into_iter().next().unwrap()
    }
}
