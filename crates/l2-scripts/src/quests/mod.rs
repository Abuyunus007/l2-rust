//! `data/scripts/quests`: the quests, one module per quest, named like the Java folder
//! (`Q00001_LettersOfLove` -> `q00001_letters_of_love`).
//!
//! Adding a quest: its file, one `pub mod` line and one `add` line below, both sorted by id
//! (`docs/design/scripts.md`).

pub mod abstract_saga_quest;
pub mod q00001_letters_of_love;
pub mod q00002_what_women_want;
pub mod q00003_will_the_seal_be_broken;
pub mod q00004_long_live_the_paagrio_lord;
pub mod q00005_miners_favor;
pub mod q00006_step_into_the_future;
pub mod q00007_a_trip_begins;
pub mod q00008_an_adventure_begins;
pub mod q00009_into_the_city_of_humans;
pub mod q00010_into_the_world;
pub mod q00011_secret_meeting_with_ketra_orcs;
pub mod q00012_secret_meeting_with_varka_silenos;
pub mod q00013_parcel_delivery;
pub mod q00014_whereabouts_of_the_archaeologist;
pub mod q00015_sweet_whispers;
pub mod q00016_the_coming_darkness;
pub mod q00017_light_and_darkness;
pub mod q00018_meeting_with_the_golden_ram;
pub mod q00019_go_to_the_pastureland;
pub mod q00020_bring_up_with_love;
pub mod q00026_tired_of_waiting;
pub mod q00027_chest_caught_with_a_bait_of_wind;
pub mod q00028_chest_caught_with_a_bait_of_icy_air;
pub mod q00029_chest_caught_with_a_bait_of_earth;
pub mod q00030_chest_caught_with_a_bait_of_fire;
pub mod q00070_saga_of_the_phoenix_knight;
pub mod q00153_deliver_goods;
pub mod q00158_seed_of_evil;
pub mod q00182_new_recruits;
pub mod q00255_tutorial;
pub mod q00275_dark_winged_spies;
pub mod q00295_dreaming_of_the_skies;
pub mod q00303_collect_arrowheads;
pub mod q00317_catch_the_wind;
pub mod q00450_grave_robber_rescue;

use l2_world::ScriptRegistry;

/// Registers every quest, by id.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(q00001_letters_of_love::Q00001LettersOfLove);
    r.add(q00002_what_women_want::Q00002WhatWomenWant);
    r.add(q00003_will_the_seal_be_broken::Q00003WillTheSealBeBroken);
    r.add(q00004_long_live_the_paagrio_lord::Q00004LongLiveThePaagrioLord);
    r.add(q00005_miners_favor::Q00005MinersFavor);
    r.add(q00006_step_into_the_future::Q00006StepIntoTheFuture);
    r.add(q00007_a_trip_begins::Q00007ATripBegins);
    r.add(q00008_an_adventure_begins::Q00008AnAdventureBegins);
    r.add(q00009_into_the_city_of_humans::Q00009IntoTheCityOfHumans);
    r.add(q00010_into_the_world::Q00010IntoTheWorld);
    r.add(q00011_secret_meeting_with_ketra_orcs::Q00011SecretMeetingWithKetraOrcs);
    r.add(q00012_secret_meeting_with_varka_silenos::Q00012SecretMeetingWithVarkaSilenos);
    r.add(q00013_parcel_delivery::Q00013ParcelDelivery);
    r.add(q00014_whereabouts_of_the_archaeologist::Q00014WhereaboutsOfTheArchaeologist);
    r.add(q00015_sweet_whispers::Q00015SweetWhispers);
    r.add(q00016_the_coming_darkness::Q00016TheComingDarkness);
    r.add(q00017_light_and_darkness::Q00017LightAndDarkness);
    r.add(q00018_meeting_with_the_golden_ram::Q00018MeetingWithTheGoldenRam);
    r.add(q00019_go_to_the_pastureland::Q00019GoToThePastureland);
    r.add(q00020_bring_up_with_love::Q00020BringUpWithLove);
    r.add(q00026_tired_of_waiting::Q00026TiredOfWaiting);
    r.add(q00027_chest_caught_with_a_bait_of_wind::Q00027ChestCaughtWithABaitOfWind);
    r.add(q00028_chest_caught_with_a_bait_of_icy_air::Q00028ChestCaughtWithABaitOfIcyAir);
    r.add(q00029_chest_caught_with_a_bait_of_earth::Q00029ChestCaughtWithABaitOfEarth);
    r.add(q00030_chest_caught_with_a_bait_of_fire::Q00030ChestCaughtWithABaitOfFire);
    r.add(q00070_saga_of_the_phoenix_knight::Q00070SagaOfThePhoenixKnight);
    r.add(q00153_deliver_goods::Q00153DeliverGoods);
    r.add(q00158_seed_of_evil::Q00158SeedOfEvil);
    r.add(q00182_new_recruits::Q00182NewRecruits);
    r.add(q00255_tutorial::Q00255Tutorial);
    r.add(q00275_dark_winged_spies::Q00275DarkWingedSpies);
    r.add(q00295_dreaming_of_the_skies::Q00295DreamingOfTheSkies);
    r.add(q00303_collect_arrowheads::Q00303CollectArrowheads);
    r.add(q00317_catch_the_wind::Q00317CatchTheWind);
    r.add(q00450_grave_robber_rescue::Q00450GraveRobberRescue);
}
