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
pub mod q00042_help_the_uncle;
pub mod q00043_help_the_sister;
pub mod q00044_help_the_son;
pub mod q00045_to_talking_island;
pub mod q00046_once_more_in_the_arms_of_the_mother_tree;
pub mod q00047_into_the_dark_elven_forest;
pub mod q00048_to_the_immortal_plateau;
pub mod q00049_the_road_home;
pub mod q00050_lanoscos_special_bait;
pub mod q00051_ofulles_special_bait;
pub mod q00052_willies_special_bait;
pub mod q00053_linnaeus_special_bait;
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
    r.add(q00042_help_the_uncle::Q00042HelpTheUncle);
    r.add(q00043_help_the_sister::Q00043HelpTheSister);
    r.add(q00044_help_the_son::Q00044HelpTheSon);
    r.add(q00045_to_talking_island::Q00045ToTalkingIsland);
    r.add(q00046_once_more_in_the_arms_of_the_mother_tree::Q00046OnceMoreInTheArmsOfTheMotherTree);
    r.add(q00047_into_the_dark_elven_forest::Q00047IntoTheDarkElvenForest);
    r.add(q00048_to_the_immortal_plateau::Q00048ToTheImmortalPlateau);
    r.add(q00049_the_road_home::Q00049TheRoadHome);
    r.add(q00050_lanoscos_special_bait::Q00050LanoscosSpecialBait);
    r.add(q00051_ofulles_special_bait::Q00051OFullesSpecialBait);
    r.add(q00052_willies_special_bait::Q00052WilliesSpecialBait);
    r.add(q00053_linnaeus_special_bait::Q00053LinnaeusSpecialBait);
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
