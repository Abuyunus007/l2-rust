//! `data/scripts/quests`: the quests, one module per quest, named like the Java folder
//! (`Q00001_LettersOfLove` -> `q00001_letters_of_love`).
//!
//! Adding a quest: its file, one `pub mod` line and one `add` line below, both sorted by id
//! (`docs/design/scripts.md`).

pub mod abstract_saga_quest;
pub mod q00001_letters_of_love;
pub mod q00034_in_search_of_cloth;
pub mod q00035_find_glittering_jewelry;
pub mod q00036_make_a_sewing_kit;
pub mod q00037_make_formal_wear;
pub mod q00038_dragon_fangs;
pub mod q00039_red_eyed_invaders;
pub mod q00040_a_special_order;
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
    r.add(q00034_in_search_of_cloth::Q00034InSearchOfCloth);
    r.add(q00035_find_glittering_jewelry::Q00035FindGlitteringJewelry);
    r.add(q00036_make_a_sewing_kit::Q00036MakeASewingKit);
    r.add(q00037_make_formal_wear::Q00037MakeFormalWear);
    r.add(q00038_dragon_fangs::Q00038DragonFangs);
    r.add(q00039_red_eyed_invaders::Q00039RedEyedInvaders);
    r.add(q00040_a_special_order::Q00040ASpecialOrder);
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
