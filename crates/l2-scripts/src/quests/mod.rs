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
