//! The 2nd class transfer of the Kamael: the male masters teach troopers, the female masters
//! warders; the certification quests stand in for the proofs.
// Java: dist/game/data/scripts/village_master/KamaelChange2/KamaelChange2.java

use l2_util::ObjectId;
use l2_world::subclass::categories::class_in_category;
use l2_world::subclass::{CategoryType, Race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, class_of, in_category, is_listed_page};

// NPCs
const NPCS_MALE: [i32; 9] = [
    32146, // Valpor
    32205, // Aetonic
    32209, // Ferdinand
    32213, // Vitus
    32217, // Barta
    32221, // Brome
    32225, // Taine
    32229, // Hagel
    32233, // Zoldart
];
const NPCS_FEMALE: [i32; 9] = [
    32145, // Maynard
    32206, // Pieche
    32210, // Eddy
    32214, // Meldina
    32218, // Miya
    32222, // Liane
    32226, // Raula
    32230, // Ceci
    32234, // Nizer
];

// Items
const ORKURUS_RECOMMENDATION: i32 = 9760;
const KAMAEL_INQUISITOR_MARK: i32 = 9782;
const SOUL_BREAKER_CERTIFICATE: i32 = 9806;

/// `PlayerClass.MALE_SOLDIER`, `FEMALE_SOLDIER`, `TROOPER`, `WARDER`, `BERSERKER`,
/// `MALE_SOULBREAKER`, `FEMALE_SOULBREAKER`, `ARBALESTER`.
const MALE_SOLDIER: i32 = 123;
const FEMALE_SOLDIER: i32 = 124;
const TROOPER: i32 = 125;
const WARDER: i32 = 126;
const BERSERKER: i32 = 127;
const MALE_SOULBREAKER: i32 = 128;
const FEMALE_SOULBREAKER: i32 = 129;
const ARBALESTER: i32 = 130;

/// Java `Q00064_CertifiedBerserker.class.getSimpleName()`, `Q00065_CertifiedSoulBreaker`,
/// `Q00066_CertifiedArbalester`.
const CERTIFIED_BERSERKER: &str = "Q00064_CertifiedBerserker";
const CERTIFIED_SOUL_BREAKER: &str = "Q00065_CertifiedSoulBreaker";
const CERTIFIED_ARBALESTER: &str = "Q00066_CertifiedArbalester";

/// The pages `onEvent` shows as they are (`32145-05.htm` ... `32145-15.htm`, without `-10`).
const PAGES: [i32; 10] = [5, 6, 7, 8, 9, 11, 12, 13, 14, 15];

/// Java `village_master.KamaelChange2.KamaelChange2`.
pub struct KamaelChange2;

/// `(qs != null) && qs.isCompleted()` of `player.getQuestState(name)`.
fn completed(ctx: &ScriptCtx<'_>, player: ObjectId, name: &str) -> bool {
    ctx.player_quest_state(player, name)
        .is_some_and(|qs| qs.is_completed())
}

/// The three pages of a certified class and the change: `low` (< 40, quest done), `low + 1`
/// (< 40, not done), `low + 2` (not done), `low + 3` (the change).
fn certified(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    quest: &str,
    item: i32,
    class_id: i32,
    low: i32,
) -> String {
    let done = completed(ctx, player, quest);
    let number = if ctx.player_level(player) < 40 {
        if done { low } else { low + 1 }
    } else if !done {
        low + 2
    } else {
        ctx.take_items(player, item, -1);
        ctx.world_mut().set_player_class(player, class_id);
        ctx.world_mut().set_base_class(player, class_id);
        // SystemMessage and cast skill is done by setClassId
        ctx.world_mut().broadcast_user_info(player);
        ctx.give_items(player, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, 15);
        low + 3
    };
    format!("32145-{number:02}.htm")
}

impl KamaelChange2 {
    /// Java `ClassChangeRequested(player, npc, classId)`.
    fn class_change_requested(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        class_id: i32,
    ) -> Option<String> {
        let mut htmltext = None;
        if class_in_category(CategoryType::KamaelThirdClassGroup, class_id) {
            let male_npc = NPCS_MALE.contains(&npc.id);
            let player_class = class_of(ctx, player);
            if in_category(ctx, player, CategoryType::KamaelFirstClassGroup) {
                if male_npc {
                    htmltext = Some("32145-02.htm".to_owned()); // master_all_kamael012b
                } else {
                    htmltext = Some("32145-03.htm".to_owned()); // master_all_kamael012c
                }
            } else if in_category(ctx, player, CategoryType::KamaelThirdClassGroup) {
                if male_npc {
                    htmltext = Some("32145-16.htm".to_owned()); // master_all_kamael005b
                } else {
                    htmltext = Some("32145-17.htm".to_owned()); // master_all_kamael005c
                }
            } else if in_category(ctx, player, CategoryType::KamaelFourthClassGroup) {
                if male_npc {
                    htmltext = Some("32145-18.htm".to_owned()); // master_all_kamael100b
                } else {
                    htmltext = Some("32145-19.htm".to_owned()); // master_all_kamael100c
                }
            } else if player_class == TROOPER {
                if male_npc {
                    if class_id == BERSERKER {
                        // master_all_kamael008ta .. 011ta
                        htmltext = Some(certified(
                            ctx,
                            player,
                            CERTIFIED_BERSERKER,
                            ORKURUS_RECOMMENDATION,
                            BERSERKER,
                            20,
                        ));
                    } else if class_id == MALE_SOULBREAKER {
                        // master_all_kamael008msa .. 011msa
                        htmltext = Some(certified(
                            ctx,
                            player,
                            CERTIFIED_SOUL_BREAKER,
                            SOUL_BREAKER_CERTIFICATE,
                            MALE_SOULBREAKER,
                            24,
                        ));
                    }
                } else {
                    htmltext = Some("32145-10.htm".to_owned()); // master_all_kamael002c
                }
            } else if player_class == WARDER {
                if male_npc {
                    htmltext = Some("32145-04.htm".to_owned()); // master_all_kamael002b
                } else {
                    if class_id == FEMALE_SOULBREAKER {
                        // master_all_kamael008fsa .. 011fsa
                        htmltext = Some(certified(
                            ctx,
                            player,
                            CERTIFIED_SOUL_BREAKER,
                            SOUL_BREAKER_CERTIFICATE,
                            FEMALE_SOULBREAKER,
                            28,
                        ));
                    } else if class_id == ARBALESTER {
                        // master_all_kamael008wa .. 011wa
                        htmltext = Some(certified(
                            ctx,
                            player,
                            CERTIFIED_ARBALESTER,
                            KAMAEL_INQUISITOR_MARK,
                            ARBALESTER,
                            32,
                        ));
                    }
                }
            }
        }
        htmltext
    }
}

impl Script for KamaelChange2 {
    fn name(&self) -> &'static str {
        "KamaelChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/KamaelChange2"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS_MALE);
        s.add_start_npc(&NPCS_FEMALE);
        s.add_talk_id(&NPCS_MALE);
        s.add_talk_id(&NPCS_FEMALE);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if is_listed_page(event, &[32145], &PAGES) {
            return Some(event.to_owned());
        }
        match event {
            "127" | "128" | "129" | "130" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        let male_npc = NPCS_MALE.contains(&npc.id);
        let player_class = class_of(ctx, player);
        if ctx.world().player_race(player) != Some(Race::Kamael) {
            htmltext = Some("32145-01.htm"); // master_all_kamael002a
        } else if in_category(ctx, player, CategoryType::KamaelFirstClassGroup) {
            if player_class == MALE_SOLDIER {
                htmltext = Some("32145-02.htm"); // master_all_kamael012b
            } else if player_class == FEMALE_SOLDIER {
                htmltext = Some("32145-03.htm"); // master_all_kamael012c
            }
        } else if in_category(ctx, player, CategoryType::KamaelSecondClassGroup) {
            if male_npc {
                if in_category(ctx, player, CategoryType::KamaelFemaleMainOccupation) {
                    return Some("32145-04.htm".to_owned()); // master_all_kamael002b
                }
                if player_class == TROOPER {
                    htmltext = Some("32145-05.htm"); // master_all_kamael003t
                } else if player_class == WARDER {
                    htmltext = Some("32145-02.htm"); // master_all_kamael012b
                }
            } else {
                if in_category(ctx, player, CategoryType::KamaelMaleMainOccupation) {
                    return Some("32145-10.htm".to_owned()); // master_all_kamael002c
                }
                if player_class == TROOPER {
                    htmltext = Some("32145-03.htm"); // master_all_kamael012c
                } else if player_class == WARDER {
                    htmltext = Some("32145-11.htm"); // master_all_kamael003w
                }
            }
        } else if in_category(ctx, player, CategoryType::KamaelThirdClassGroup) {
            if male_npc {
                if in_category(ctx, player, CategoryType::KamaelMaleMainOccupation) {
                    htmltext = Some("32145-16.htm"); // master_all_kamael005b
                } else {
                    htmltext = Some("32145-04.htm"); // master_all_kamael002b
                }
            } else {
                if in_category(ctx, player, CategoryType::KamaelFemaleMainOccupation) {
                    htmltext = Some("32145-17.htm"); // master_all_kamael005c
                } else {
                    htmltext = Some("32145-10.htm"); // master_all_kamael002c
                }
            }
        } else if in_category(ctx, player, CategoryType::KamaelFourthClassGroup) {
            if male_npc {
                if in_category(ctx, player, CategoryType::KamaelMaleMainOccupation) {
                    htmltext = Some("32145-18.htm"); // master_all_kamael100b
                } else {
                    htmltext = Some("32145-04.htm"); // master_all_kamael002b
                }
            } else {
                if in_category(ctx, player, CategoryType::KamaelFemaleMainOccupation) {
                    htmltext = Some("32145-19.htm"); // master_all_kamael100c
                } else {
                    htmltext = Some("32145-10.htm"); // master_all_kamael002c
                }
            }
        }
        htmltext.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::QuestLoad;

    use super::*;
    use crate::village_master::testing::{Vm, character, item};

    const DIR: &str = "data/scripts/village_master/KamaelChange2";

    /// The certification quests as far as this script needs them: quests of those names.
    struct Certification(&'static str, i32);

    impl Script for Certification {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Certified"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            KamaelChange2,
            DIR,
            "KamaelChange2",
            &[(32146, "VillageMasterKamael"), (32145, "VillageMasterKamael")],
        );
        vm.also(Certification(CERTIFIED_BERSERKER, 64));
        vm.also(Certification(CERTIFIED_SOUL_BREAKER, 65));
        vm.also(Certification(CERTIFIED_ARBALESTER, 66));
        vm.page("32145-05.htm", &["127", "128"]);
        vm.page("32145-11.htm", &["129", "130"]);
        for n in [1, 3, 4, 10, 16, 21, 22, 23, 26, 33, 35] {
            vm.page(&format!("32145-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    fn done(
        id: i32,
        level: i32,
        class_id: i32,
        quest: &str,
        items: Vec<l2_world::ItemData>,
    ) -> l2_world::PlayerData {
        let mut data = character(id, level, class_id, items);
        data.profile.quests = QuestLoad {
            states: vec![(quest.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        data
    }

    #[test]
    fn a_trooper_becomes_a_berserker_at_a_male_master() {
        let mut vm = setup();
        vm.t.enter_with(done(
            1,
            40,
            TROOPER,
            CERTIFIED_BERSERKER,
            vec![item(100, ORKURUS_RECOMMENDATION, 1)],
        ));
        vm.t.take();
        assert!(vm.talk(1, 32146).contains("32145-05.htm"));
        assert!(vm.click(1, "128").contains("32145-26.htm"));
        assert!(vm.talk(1, 32146).contains("32145-05.htm"));
        assert!(vm.click(1, "127").contains("32145-23.htm"));
        vm.assert_class(1, BERSERKER);
        assert_eq!(vm.t.count(1, ORKURUS_RECOMMENDATION), 0);
        assert_eq!(vm.t.count(1, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);
        assert!(vm.talk(1, 32146).contains("32145-16.htm"));
        // A trooper at a female master.
        vm.enter(2, 40, TROOPER, vec![]);
        assert!(vm.talk(2, 32145).contains("32145-10.htm"));
    }

    #[test]
    fn a_warder_at_the_female_master_and_the_rest() {
        let mut vm = setup();
        vm.t.enter_with(done(
            1,
            40,
            WARDER,
            CERTIFIED_ARBALESTER,
            vec![item(100, KAMAEL_INQUISITOR_MARK, 1)],
        ));
        vm.t.take();
        assert!(vm.talk(1, 32145).contains("32145-11.htm"));
        assert!(vm.click(1, "130").contains("32145-35.htm"));
        vm.assert_class(1, ARBALESTER);
        vm.enter(2, 39, WARDER, vec![]);
        assert!(vm.talk(2, 32145).contains("32145-11.htm"));
        assert!(vm.click(2, "130").contains("32145-33.htm"));
        // A warder at a male master, a soldier, another race.
        assert!(vm.talk(2, 32146).contains("32145-04.htm"));
        vm.enter(3, 20, FEMALE_SOLDIER, vec![]);
        assert!(vm.talk(3, 32146).contains("32145-03.htm"));
        vm.enter(4, 40, 0, vec![]);
        assert!(vm.talk(4, 32146).contains("32145-01.htm"));
    }
}
