//! Freya Celebration: Freya takes an Adena for a potion and thanks the casters of the gift skills.
// Java: dist/game/data/scripts/events/FreyaCelebration/FreyaCelebration.java
// Java: dist/game/data/scripts/events/FreyaCelebration/config.xml

use l2_protocol_game::server::{ChatType, SystemMessage, SystemMessageParam};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::long_time_event::{self, EventConfig, SpawnSpec};
use super::{broadcast_say_string, hours_and_minutes};

// NPC
const FREYA: i32 = 13296;

// Items
const FREYA_POTION: i32 = 15440;
const FREYA_GIFT: i32 = 17138;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Misc
const RESET_VAR: &str = "FreyaCelebration";
const HOURS: i64 = 20;

const SKILLS: [i32; 7] = [9150, 9151, 9152, 9153, 9154, 9155, 9156];

const FREYA_TEXT: [NpcStringId; 5] = [
    NpcStringId::EvenThoughYouBringSomethingCalledAGiftAmongYourHumansItWouldJustBeProblematicForMe,
    NpcStringId::IJustDonTKnowWhatExpressionIShouldHaveItAppearedOnMeAreHumanSEmotionsLikeThisFeeling,
    NpcStringId::TheFeelingOfThanksIsJustTooMuchDistantMemoryForMe,
    NpcStringId::ButIKindOfMissItLikeIHadFeltThisFeelingBefore,
    NpcStringId::IAmIceQueenFreyaThisFeelingAndEmotionAreNothingButAPartOfMelissaAMemories,
];

/// `config.xml` of the event.
const CONFIG: EventConfig = EventConfig {
    name: "Freya Celebration",
    active: "27 02 2009-28 02 2009",
    drop_period: None,
    drops: &[],
    spawns: &[
        SpawnSpec::at(13296, -119494, 44882, 360, 24576, 0),
        SpawnSpec::at(13296, -117239, 46842, 360, 49151, 0),
        SpawnSpec::at(13296, -84023, 243051, -3728, 4096, 0),
        SpawnSpec::at(13296, -84411, 244813, -3728, 57343, 0),
        SpawnSpec::at(13296, 46908, 50856, -2992, 8192, 0),
        SpawnSpec::at(13296, 45538, 48357, -3056, 18000, 0),
        SpawnSpec::at(13296, -45372, -114104, -240, 16384, 0),
        SpawnSpec::at(13296, -45278, -112766, -240, 0, 0),
        SpawnSpec::at(13296, 9929, 16324, -4568, 62999, 0),
        SpawnSpec::at(13296, 11546, 17599, -4584, 46900, 0),
        SpawnSpec::at(13296, 115096, -178370, -880, 0, 0),
        SpawnSpec::at(13296, -13727, 122117, -2984, 16384, 0),
        SpawnSpec::at(13296, -14129, 123869, -3112, 40959, 0),
        SpawnSpec::at(13296, -83156, 150994, -3120, 0, 0),
        SpawnSpec::at(13296, -81031, 150038, -3040, 0, 0),
        SpawnSpec::at(13296, 16111, 142850, -2696, 16000, 0),
        SpawnSpec::at(13296, 17275, 145000, -3032, 25000, 0),
        SpawnSpec::at(13296, 111004, 218928, -3536, 16384, 0),
        SpawnSpec::at(13296, 81755, 146487, -3528, 32768, 0),
        SpawnSpec::at(13296, 82145, 148609, -3464, 0, 0),
        SpawnSpec::at(13296, 83037, 149324, -3464, 44000, 0),
        SpawnSpec::at(13296, 81987, 53723, -1488, 0, 0),
        SpawnSpec::at(13296, 147200, 25614, -2008, 16384, 0),
        SpawnSpec::at(13296, 148557, 26806, -2200, 32768, 0),
        SpawnSpec::at(13296, 147421, -55435, -2728, 49151, 0),
        SpawnSpec::at(13296, 148206, -55786, -2776, 61439, 0),
        SpawnSpec::at(13296, 85584, -142490, -1336, 0, 0),
        SpawnSpec::at(13296, 86865, -142915, -1336, 26000, 0),
        SpawnSpec::at(13296, 43966, -47709, -792, 49999, 0),
        SpawnSpec::at(13296, 43165, -48461, -792, 17000, 0),
    ],
    on_enter_msg: "Freya Celebration: Event ongoing!",
    on_end_msg: "Freya Celebration: Event end!",
    destroy_items_on_end: &[],
};

/// Java `events.FreyaCelebration.FreyaCelebration`.
pub struct FreyaCelebration;

impl Script for FreyaCelebration {
    fn name(&self) -> &'static str {
        "FreyaCelebration"
    }

    fn path(&self) -> &'static str {
        "events/FreyaCelebration"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FREYA]);
        s.add_first_talk_id(&[FREYA]);
        s.add_talk_id(&[FREYA]);
        s.add_skill_see_id(&[FREYA]);
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
        if event.eq_ignore_ascii_case("give_potion") {
            if ctx.get_quest_items_count(player, ADENA_ID) > 1 {
                let current_time = ctx.now_ms();
                let reuse_time: i64 = ctx
                    .player_variable(player, RESET_VAR)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if current_time > reuse_time {
                    ctx.take_items(player, ADENA_ID, 1);
                    ctx.give_items(player, FREYA_POTION, 1);
                    let until = current_time + HOURS * 3_600_000;
                    ctx.set_player_variable(player, RESET_VAR, &until.to_string());
                } else {
                    let (hours, minutes) = hours_and_minutes(reuse_time - current_time);
                    let sm =
                        SystemMessage::new(SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS)
                            .with(ctx.world().item_message_param(FREYA_POTION))
                            .with(SystemMessageParam::IntNumber(hours))
                            .with(SystemMessageParam::IntNumber(minutes));
                    ctx.world().send(player, &sm);
                }
            } else {
                let sm = SystemMessage::new(SystemMessageId::S2UnitSOfTheItemS1IsAreRequired)
                    .with(ctx.world().item_message_param(ADENA_ID))
                    .with(SystemMessageParam::IntNumber(1));
                ctx.world().send(player, &sm);
            }
        }

        None
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if npc.id == FREYA && targets.contains(&npc.object_id) && SKILLS.contains(&skill.id) {
            if ctx.get_random(100) < 5 {
                let caster_name = ctx.player_name(caster);
                broadcast_say_string(
                    ctx,
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::DearS1ThinkOfThisAsMyAppreciationForTheGiftTakeThisWithYouThereSNothingStrangeAboutItItSJustABitOfMyCapriciousness.id(),
                    vec![caster_name],
                );

                // `caster.addItem(ItemProcessType.REWARD, FREYA_GIFT, 1, npc, true)`
                ctx.give_items(caster, FREYA_GIFT, 1);
            } else if ctx.get_random(10) < 2 {
                if let Some(text) = ctx.get_random_entry(&FREYA_TEXT) {
                    broadcast_say_string(ctx, npc, ChatType::NpcGeneral, text.id(), Vec::new());
                }
            }
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("13296.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::events::long_time_event::TEST_CLOCK;
    use crate::testing::{Pages, ScriptTest, item, names_to};

    const NPCS: &str = r#"<npc id="13296" type="Folk" level="70" name="Freya"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15440" type="EtcItem" name="Freya's Potion"/>
        <item id="17138" type="EtcItem" name="Freya's Gift"/>"#;
    /// 2009-02-27 12:00 UTC, inside the event.
    const INSIDE: i64 = 1_235_736_000_000;

    fn setup(now: Option<i64>) -> ScriptTest {
        TEST_CLOCK.set(now);
        let mut registry = ScriptRegistry::new();
        registry.add(FreyaCelebration);
        let pages = Pages::of(&FreyaCelebration)
            .page("13296.htm", &["give_potion"])
            .npcs(&[FREYA]);
        ScriptTest::with_pages(registry, NPCS, ITEMS, pages)
    }

    #[test]
    fn inside_the_dates_the_freyas_stand_and_outside_they_do_not() {
        assert_eq!(setup(Some(INSIDE)).npcs_of(FREYA).len(), 30);
        assert!(setup(None).npcs_of(FREYA).is_empty());
    }

    #[test]
    fn one_adena_buys_a_potion_every_20_hours() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 5)]);
        let freya = t.spawn(FREYA, 20);
        t.talk(1, freya);
        t.event(1, "FreyaCelebration", "give_potion");
        assert_eq!(t.count(1, FREYA_POTION), 1);
        assert_eq!(t.count(1, ADENA_ID), 4);

        // The same day: the system message tells the time, nothing else happens.
        t.talk(1, freya);
        t.take();
        t.bypass(1, "Script FreyaCelebration give_potion");
        assert_eq!(t.count(1, FREYA_POTION), 1);
        assert_eq!(t.count(1, ADENA_ID), 4);
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"SYSTEM_MESSAGE".to_owned()), "{names:?}");
    }

    #[test]
    fn one_adena_is_not_enough() {
        // Java quirk: it asks for more than one Adena (`> 1`) while the message says 1.
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![item(1000, ADENA_ID, 1)]);
        let freya = t.spawn(FREYA, 20);
        t.talk(1, freya);
        t.take();
        t.bypass(1, "Script FreyaCelebration give_potion");
        assert_eq!(t.count(1, FREYA_POTION), 0);
        assert!(names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn the_gift_skills_cast_at_freya_are_thanked() {
        let mut t = setup(Some(INSIDE));
        t.enter(1, 40, vec![]);
        let freya = t.spawn(FREYA, 20);
        let npc = ScriptNpc {
            object_id: freya,
            id: FREYA,
        };
        let mut gifts = 0;
        for _ in 0..200 {
            t.world.run_as_script("FreyaCelebration", |ctx| {
                FreyaCelebration.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(9150, 1), &[freya], false);
            });
            gifts = t.count(1, FREYA_GIFT);
            if gifts > 0 {
                break;
            }
        }
        assert_eq!(gifts, 1, "5% of the casts bring a gift");
        // Another skill, or another target, does nothing.
        t.world.run_as_script("FreyaCelebration", |ctx| {
            for _ in 0..200 {
                FreyaCelebration.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(1, 1), &[freya], false);
                FreyaCelebration.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(9150, 1), &[], false);
            }
        });
        assert_eq!(t.count(1, FREYA_GIFT), 1);
    }
}
