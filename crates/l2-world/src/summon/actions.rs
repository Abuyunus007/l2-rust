//! What a player tells its summon: the summon actions of `RequestActionUse`, the summon's skills
//! (`useMagic`), the attack checks (`canAttack`) and the click on a summon.
// Java: network/clientpackets/RequestActionUse.java (the common checks, the summon actions,
//       useSkill, canControl, validateSummon)
// Java: entity/actor/Summon.java (useMagic, canAttack, doCast, PASSIVE_SUMMONS)
// Java: data/xml/PetSkillData.java (getAvailableLevel)
// Java: dist/game/data/scripts/handlers/actions/click/SummonClick.java
//
// The player's own actions (sit, walk, socials, couple actions) are `actions.rs`; the stores are
// `trade/store.rs`. The manufacture and mount actions are `craft/shop.rs` and `mount.rs`; the airship and
// bot report actions belong to their own systems and are logged as not ported here.

use l2_data::NpcTemplate;
use l2_data::npc::{NpcParam, NpcRace};
use l2_data::skill::TargetType;
use l2_protocol_game::server::{ActionFailed, ChatType, NpcSay, PetStatusShow, SystemMessage};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::effects::SkillKey;
use crate::effects::apply::has_negative_effect;
use crate::objects::ObjectKind;
use crate::world::World;

/// `RequestActionUse.SIN_EATER_ID`.
const SIN_EATER_ID: i32 = 12564;
/// `RequestActionUse.SWITCH_STANCE_ID`.
const SWITCH_STANCE_ID: i32 = 6054;
/// `RequestActionUse.NPC_STRINGS`: what the Sin Eater says (action 1001).
const NPC_STRINGS: [NpcStringId; 4] = [
    NpcStringId::UsingASpecialSkillHereCouldTriggerABloodbath,
    NpcStringId::HeyWhatDoYouExpectOfMe,
    NpcStringId::UggggghPushItSNotComingOut,
    NpcStringId::AhIMissedTheMark,
];
/// `Summon.PASSIVE_SUMMONS`: "Sin eater, Big Boom, Wyvern can't attack with attack button."
const PASSIVE_SUMMONS: [i32; 37] = [
    12564, 12621, 14702, 14703, 14704, 14705, 14706, 14707, 14708, 14709, 14710, 14711, 14712, 14713, 14714,
    14715, 14716, 14717, 14718, 14719, 14720, 14721, 14722, 14723, 14724, 14725, 14726, 14727, 14728, 14729,
    14730, 14731, 14732, 14733, 14734, 14735, 14736,
];
/// `Summon.canAttack`: an owner farther than this from the target sends the summon back.
const MAX_ATTACK_ORDER_DISTANCE: f64 = 3000.0;

/// The skill a summon action uses: a fixed id (`PetSkillData` gives the level) or a name of the
/// NPC template's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionSkill {
    Id(i32),
    Name(&'static str),
}

/// What an action id does with the summon: `(skill, the owner as target, pet)`.
fn action_skill(action_id: i32) -> Option<(ActionSkill, bool, bool)> {
    use ActionSkill::{Id, Name};
    Some(match action_id {
        32 => (Name("DDMagic"), false, false), // Wild Hog Cannon - Wild Cannon
        36 => (Name("RangeDebuff"), false, false), // Soulless - Toxic Smoke
        39 => (Name("RangeDD"), false, false), // Soulless - Parasite Burst
        42 => (Name("HealMagic"), false, false), // Kai the Cat - Self Damage Shield
        43 => (Name("DDMagic"), false, false), // Merrow the Unicorn - Hydro Screw
        44 => (Name("DDMagic"), false, false), // Big Boom - Boom Attack
        45 => (Name("HealMagic"), true, false), // Boxer the Unicorn - Master Recharge
        46 => (Name("DDMagic"), false, false), // Mew the Cat - Mega Storm Strike
        47 => (Name("DDMagic"), false, false), // Silhouette - Steal Blood
        48 => (Name("DDMagic"), false, false), // Mechanic Golem - Mech. Cannon
        1003 => (Name("PhysicalSpecial"), false, true), // Wind Hatchling/Strider - Wild Stun
        1004 => (Name("Buff"), true, true),    // Wind Hatchling/Strider - Wild Defense
        1005 => (Name("DDMagic"), false, true), // Star Hatchling/Strider - Bright Burst
        1006 => (Name("Heal"), true, true),    // Star Hatchling/Strider - Bright Heal
        1007 => (Name("Buff1"), true, false),  // Feline Queen - Blessing of Queen
        1008 => (Name("Buff2"), true, false),  // Feline Queen - Gift of Queen
        1009 => (Name("DDMagic"), false, false), // Feline Queen - Cure of Queen
        1010 => (Name("Buff1"), true, false),  // Unicorn Seraphim - Blessing of Seraphim
        1011 => (Name("Buff2"), true, false),  // Unicorn Seraphim - Gift of Seraphim
        1012 => (Name("DDMagic"), false, false), // Unicorn Seraphim - Cure of Seraphim
        1013 => (Name("DeBuff1"), false, false), // Nightshade - Curse of Shade
        1014 => (Name("DeBuff2"), false, false), // Nightshade - Mass Curse of Shade
        1015 => (Name("Heal"), false, false),  // Nightshade - Shade Sacrifice
        1016 => (Name("PhysicalSpecial1"), false, false), // Cursed Man - Cursed Blow
        1017 => (Name("PhysicalSpecial2"), false, false), // Cursed Man - Cursed Strike
        1031 => (Name("PhysicalSpecial1"), false, false), // Feline King - Slash
        1032 => (Name("PhysicalSpecial2"), false, false), // Feline King - Spinning Slash
        1033 => (Name("PhysicalSpecial3"), false, false), // Feline King - Hold of King
        1034 => (Name("PhysicalSpecial1"), false, false), // Magnus the Unicorn - Whiplash
        1035 => (Name("PhysicalSpecial2"), false, false), // Magnus the Unicorn - Tridal Wave
        1036 => (Name("PhysicalSpecial1"), false, false), // Spectral Lord - Corpse Kaboom
        1037 => (Name("PhysicalSpecial2"), false, false), // Spectral Lord - Dicing Death
        1038 => (Name("PhysicalSpecial3"), false, false), // Spectral Lord - Dark Curse
        1039 => (Id(5110), false, false),      // Swoop Cannon - Cannon Fodder
        1040 => (Id(5111), false, false),      // Swoop Cannon - Big Bang
        1041 => (Name("Skill01"), false, true), // Great Wolf - Bite Attack
        1042 => (Name("Skill03"), false, true), // Great Wolf - Maul
        1043 => (Name("Skill02"), false, true), // Great Wolf - Cry of the Wolf
        1044 => (Name("Skill04"), false, true), // Great Wolf - Awakening
        1045 => (Id(5584), false, true),       // Great Wolf - Howl
        1046 => (Id(5585), false, true),       // Strider - Roar
        1047 => (Id(5580), false, false),      // Divine Beast - Bite
        1048 => (Id(5581), false, false),      // Divine Beast - Stun Attack
        1049 => (Id(5582), false, false),      // Divine Beast - Fire Breath
        1050 => (Id(5583), false, false),      // Divine Beast - Roar
        1051 => (Name("buff3"), false, false), // Feline Queen - Bless The Body
        1052 => (Name("buff4"), false, false), // Feline Queen - Bless The Soul
        1053 => (Name("buff5"), false, false), // Feline Queen - Haste
        1054 => (Name("buff3"), false, false), // Unicorn Seraphim - Acumen
        1055 => (Name("buff4"), false, false), // Unicorn Seraphim - Clarity
        1056 => (Name("buff5"), false, false), // Unicorn Seraphim - Empower
        1057 => (Name("buff6"), false, false), // Unicorn Seraphim - Wild Magic
        1058 => (Name("buff3"), false, false), // Nightshade - Death Whisper
        1059 => (Name("buff4"), false, false), // Nightshade - Focus
        1060 => (Name("buff5"), false, false), // Nightshade - Guidance
        1061 => (Id(5745), false, true),       // Wild Beast Fighter, White Weasel - Death blow
        1062 => (Id(5746), false, true),       // Wild Beast Fighter - Double attack
        1063 => (Id(5747), false, true),       // Wild Beast Fighter - Spin attack
        1064 => (Id(5748), false, true),       // Wild Beast Fighter - Meteor Shower
        1065 => (Id(5753), false, true),       // ... - Awakening
        1066 => (Id(5749), false, true),       // Fox Shaman, Spirit Shaman - Thunder Bolt
        1067 => (Id(5750), false, true),       // Fox Shaman, Spirit Shaman - Flash
        1068 => (Id(5751), false, true),       // Fox Shaman, Spirit Shaman - Lightning Wave
        1069 => (Id(5752), false, true),       // Fox Shaman, Fairy Princess - Flare
        1070 => (Id(5771), false, true),       // ... - Buff control
        1071 => (Name("DDMagic"), false, true), // Tigress - Power Strike
        1072 => (Id(6046), false, true),       // Toy Knight - Piercing attack
        1073 => (Id(6047), false, true),       // Toy Knight - Whirlwind
        1074 => (Id(6048), false, true),       // Toy Knight - Lance Smash
        1075 => (Id(6049), false, true),       // Toy Knight - Battle Cry
        1076 => (Id(6050), false, true),       // Turtle Ascetic - Power Smash
        1077 => (Id(6051), false, true),       // Turtle Ascetic - Energy Burst
        1078 => (Id(6052), false, true),       // Turtle Ascetic - Shockwave
        1079 => (Id(6053), false, true),       // Turtle Ascetic - Howl
        1080 => (Id(6041), false, false),      // Phoenix Rush
        1081 => (Id(6042), false, false),      // Phoenix Cleanse
        1082 => (Id(6043), false, false),      // Phoenix Flame Feather
        1083 => (Id(6044), false, false),      // Phoenix Flame Beak
        1086 => (Id(6094), false, false),      // Panther Cancel
        1087 => (Id(6095), false, false),      // Panther Dark Claw
        1088 => (Id(6096), false, false),      // Panther Fatal Claw
        1089 => (Id(6199), false, true),       // Deinonychus - Tail Strike
        1090 => (Id(6205), false, true),       // Guardian's Strider - Strider Bite
        1091 => (Id(6206), false, true),       // Guardian's Strider - Strider Fear
        1092 => (Id(6207), false, true),       // Guardian's Strider - Strider Dash
        1093 => (Id(6618), false, true),       // Maguen - Maguen Strike
        1094 => (Id(6681), false, true),       // Maguen - Maguen Wind Walk
        1095 => (Id(6619), false, true),       // Elite Maguen - Maguen Power Strike
        1096 => (Id(6682), false, true),       // Elite Maguen - Elite Maguen Wind Walk
        1097 => (Id(6683), false, true),       // Maguen - Maguen Return
        1098 => (Id(6684), false, true),       // Elite Maguen - Maguen Party Return
        5000 => (Id(23155), false, true),      // Baby Rudolph - Reindeer Scratch
        5001 => (Id(23167), false, true),      // ... - Rosy Seduction
        5002 => (Id(23168), false, true),      // ... - Critical Seduction
        5003 => (Id(5749), false, true),       // Hyum, Lapham - Thunder Bolt
        5004 => (Id(5750), false, true),       // Hyum, Lapham - Flash
        5005 => (Id(5751), false, true),       // Hyum, Lapham - Lightning Wave
        5006 => (Id(5771), false, true),       // ... - Buff Control
        5007 => (Id(6046), false, true),       // Deseloph, Lilias - Piercing Attack
        5008 => (Id(6047), false, true),       // Deseloph, Lilias - Spin Attack
        5009 => (Id(6048), false, true),       // Deseloph, Lilias - Smash
        5010 => (Id(6049), false, true),       // Deseloph, Lilias - Ignite
        5011 => (Id(6050), false, true),       // Rekang, Mafum - Power Smash
        5012 => (Id(6051), false, true),       // Rekang, Mafum - Energy Burst
        5013 => (Id(6052), false, true),       // Rekang, Mafum - Shockwave
        5014 => (Id(6053), false, true),       // Rekang, Mafum - Ignite
        5015 => (Id(SWITCH_STANCE_ID), false, true), // ... - Switch Stance
        _ => return None,
    })
}

impl World {
    /// Java `RequestActionUse.runImpl`: the checks every action goes through, then the summon
    /// actions.
    pub(crate) fn on_action_use(
        &mut self,
        player: ObjectId,
        action_id: i32,
        ctrl_pressed: bool,
        shift_pressed: bool,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        // Don't do anything if player is dead or confused.
        if (self.is_fake_death(player) && action_id != 0)
            || self.is_dead(player)
            || self.is_out_of_control(player)
        {
            self.send(player, &ActionFailed);
            return;
        }
        if !self.check_bot_penalty(player, action_id) {
            return;
        }
        // Don't allow to do some action if player is transformed.
        if !self.transformed_action_allowed(player, action_id) {
            self.send(player, &ActionFailed);
            let name = self.players.get(&player).map_or("?", |p| p.data.name.as_str());
            warn!(
                "{name} used action which he does not have! Id = {action_id} transform: {}",
                self.transformation_id(player)
            );
            return;
        }

        let summon = self.summon_of(player);
        let target = self.target_of(player);
        match action_id {
            // Change Movement Mode (Pets / Servitors).
            15 | 21 => {
                if let Some(summon) = self.validate_summon(player, summon, action_id == 15) {
                    self.notify_follow_status_change(summon);
                }
            }
            // Attack (Pets / Servitors).
            16 | 22 => {
                if let Some(summon) = self.validate_summon(player, summon, action_id == 16)
                    && self.summon_can_attack(summon, ctrl_pressed)
                {
                    self.do_summon_attack(summon, target);
                }
            }
            // Stop (Pets / Servitors).
            17 | 23 => {
                if let Some(summon) = self.validate_summon(player, summon, action_id == 17) {
                    self.summon_cancel_action(summon);
                }
            }
            // Dwarven Manufacture and General Manufacture (M6-CRAFT-01, `craft/shop.rs`).
            37 => self.action_manufacture(player, true),
            51 => self.action_manufacture(player, false),
            // Mount/Dismount (`mount.rs`).
            38 => {
                self.mount_player(player, summon);
            }
            // Unsummon Pet.
            19 => self.action_unsummon_pet(player, summon),
            // Unsummon Servitor.
            52 => {
                if let Some(summon) = self.validate_summon(player, summon, false) {
                    if self.is_movement_disabled(summon) {
                        self.send_system(
                            player,
                            SystemMessageId::YourPetServitorIsCurrentlyInAStateOfDistress,
                        );
                    } else if self.is_attacking_now(summon) || self.summon_in_combat(summon) {
                        self.send_system(
                            player,
                            SystemMessageId::AServitorWhomIsEngagedInBattleCannotBeDeActivated,
                        );
                    } else {
                        self.unsummon(summon, Some(player));
                    }
                }
            }
            // Move to target (Servitors / Pets).
            53 | 54 => {
                if let Some(summon) = self.validate_summon(player, summon, action_id == 54)
                    && let Some(target) = target.filter(|t| *t != summon)
                    && !self.is_movement_disabled(summon)
                {
                    self.set_follow_status(summon, false);
                    if let Some((x, y, z)) = self.position_of(target) {
                        self.summon_set_intention_move_to(summon, x, y, z);
                    }
                }
            }
            // Wild Hog Cannon - Attack.
            41 => {
                if self.validate_summon(player, summon, false).is_some() {
                    let siege_target = target.is_some_and(|t| {
                        self.objects.get(&t).is_some_and(|e| e.kind == ObjectKind::Door)
                            || self.npc_template(t).is_some_and(|n| n.npc_type == "SiegeFlag")
                    });
                    if siege_target {
                        self.summon_use_skill(
                            player,
                            ActionSkill::Id(4230),
                            target,
                            false,
                            ctrl_pressed,
                            shift_pressed,
                        );
                    } else {
                        self.send_system(player, SystemMessageId::InvalidTarget);
                    }
                }
            }
            // Siege Golem - Siege Hammer.
            1000 => {
                if target.is_some_and(|t| self.objects.get(&t).is_some_and(|e| e.kind == ObjectKind::Door)) {
                    self.summon_use_skill(
                        player,
                        ActionSkill::Id(4079),
                        target,
                        false,
                        ctrl_pressed,
                        shift_pressed,
                    );
                }
            }
            // Sin Eater - Ultimate Bombastic Buster.
            1001 => {
                if let Some(summon) = self.validate_summon(player, summon, true)
                    && self.summon_npc_id(summon) == Some(SIN_EATER_ID)
                {
                    let index = self.rng().below(NPC_STRINGS.len() as i32);
                    let string = NPC_STRINGS[usize::try_from(index).unwrap_or(0)];
                    let packet = NpcSay {
                        object_id: summon.0,
                        text_type: ChatType::NpcGeneral.client_id(),
                        npc_id: SIN_EATER_ID,
                        npc_string: string as i32,
                        text: String::new(),
                        parameters: Vec::new(),
                    };
                    self.broadcast_packet(summon, &packet);
                }
            }
            // Switch State (baby pets only).
            1084 => {
                if summon.is_some_and(|s| self.is_baby_pet(s)) {
                    self.summon_use_skill(
                        player,
                        ActionSkill::Id(SWITCH_STANCE_ID),
                        target,
                        true,
                        ctrl_pressed,
                        shift_pressed,
                    );
                }
            }
            other => match action_skill(other) {
                Some((skill, owner_target, pet)) => {
                    let skill_target = if owner_target { Some(player) } else { target };
                    self.summon_use_skill(player, skill, skill_target, pet, ctrl_pressed, shift_pressed);
                }
                // The player's own actions: sit, walk, socials, couple actions (`actions.rs`).
                None => {
                    if !self.on_player_action(player, other, target) {
                        debug!("RequestActionUse {other} of {player}: not a summon action, not ported yet");
                    }
                }
            },
        }
    }

    fn send_system(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    /// Action 19: `Unsummon Pet`.
    fn action_unsummon_pet(&mut self, player: ObjectId, summon: Option<ObjectId>) {
        let Some(summon) = self.validate_summon(player, summon, true) else {
            return;
        };
        if self.is_dead(summon) {
            self.send_system(
                player,
                SystemMessageId::DeadPetsCannotBeReturnedToTheirSummoningItem,
            );
            return;
        }
        if self.is_movement_disabled(summon) {
            self.send_system(
                player,
                SystemMessageId::YourPetServitorIsCurrentlyInAStateOfDistress,
            );
            return;
        }
        if self.is_attacking_now(summon) || self.summon_in_combat(summon) {
            self.send_system(player, SystemMessageId::APetCannotBeUnsummonedDuringBattle);
            return;
        }
        if self.is_pet(summon) {
            let fed = self.pet_current_fed(summon) as f32;
            let max_fed = self.pet_max_fed(summon) as f32;
            let current_fed_percent = (fed / max_fed) * 100.0_f32;
            if current_fed_percent < 40.0 {
                self.send_system(player, SystemMessageId::YouMayNotRestoreAHungryPet);
                return;
            }
            self.unsummon(summon, Some(player));
            return;
        }
        self.send_system(
            player,
            SystemMessageId::TheHuntingHelperPetCannotBeReturnedBecauseThereIsNotMuchTimeRemainingUntilItLeaves,
        );
    }

    /// Java `RequestActionUse.validateSummon(player, summon, checkPet)`: the summon when it may
    /// take the order (a pet for the pet actions, a servitor for both).
    fn validate_summon(
        &mut self,
        player: ObjectId,
        summon: Option<ObjectId>,
        check_pet: bool,
    ) -> Option<ObjectId> {
        if let Some(summon) = summon
            && ((check_pet && self.is_pet(summon)) || self.is_servitor(summon))
        {
            if self.is_pet(summon) && self.pet_is_uncontrollable(summon) {
                // NOTE(java-quirk): the message is the siege war status window one.
                self.send_system(
                    player,
                    SystemMessageId::OnlyAClanLeaderThatIsANoblesseCanViewTheSiegeWarStatusWindowDuringASiegeWar,
                );
                return None;
            }
            if self.is_betrayed(summon) {
                self.send_system(
                    player,
                    SystemMessageId::YourPetServitorIsUnresponsiveAndWillNotObeyAnyOrders,
                );
                return None;
            }
            return Some(summon);
        }
        if check_pet {
            self.send_system(player, SystemMessageId::YouDoNotHaveAPet);
        } else {
            self.send_system(player, SystemMessageId::YouDoNotHaveAServitor);
        }
        None
    }

    /// Java `RequestActionUse.canControl(player, summon)`.
    fn can_control(&self, player: ObjectId, summon: ObjectId) -> bool {
        // NOTE(java-quirk): the switch stance skill goes through here too, so a baby pet in the
        // auxiliary mode cannot be switched back with the button (`switchMode()` is never reached).
        if self.is_baby_pet(summon) && !self.baby_is_in_support_mode(summon) {
            self.send_system(player, SystemMessageId::APetOnAuxiliaryModeCannotUseSkills);
            return false;
        }
        if self.is_pet(summon) {
            let level = self.stats(summon).map_or(0, |s| s.level());
            let owner_level = self.core_of(player).map_or(0, |c| c.level);
            if level - owner_level > 20 {
                self.send_system(player, SystemMessageId::YourPetIsTooHighLevelToControl);
                return false;
            }
        }
        true
    }

    /// Java `RequestActionUse.useSkill(player, skill, target, pet)` (by id and by name).
    fn summon_use_skill(
        &mut self,
        player: ObjectId,
        skill: ActionSkill,
        target: Option<ObjectId>,
        pet: bool,
        ctrl_pressed: bool,
        shift_pressed: bool,
    ) {
        let summon = self.summon_of(player);
        let Some(summon) = self.validate_summon(player, summon, pet) else {
            return;
        };
        if !self.can_control(player, summon) {
            return;
        }
        match skill {
            ActionSkill::Id(skill_id) => {
                // A pet's level comes from `PetDataTable`.
                let level = if self.is_pet(summon) {
                    self.pet_available_skill_level(summon, skill_id)
                } else {
                    self.pet_skill_available_level(summon, skill_id)
                };
                if level > 0 {
                    self.set_creature_target(summon, target);
                    self.summon_use_magic(
                        summon,
                        SkillKey::new(skill_id, level),
                        ctrl_pressed,
                        shift_pressed,
                    );
                }
                if skill_id == SWITCH_STANCE_ID {
                    // `summon.switchMode()`: does nothing but for baby pets.
                    self.summon_switch_mode(summon);
                }
            }
            ActionSkill::Name(name) => {
                if self.is_baby_pet(summon) && !self.baby_is_in_support_mode(summon) {
                    self.send_system(player, SystemMessageId::APetOnAuxiliaryModeCannotUseSkills);
                    return;
                }
                let holder = self.summon_template(summon).and_then(|t| template_skill(t, name));
                let Some(key) = holder else {
                    return;
                };
                if self
                    .skill_table()
                    .is_some_and(|t| t.get(key.id, key.level).is_some())
                {
                    self.set_creature_target(summon, target);
                    self.summon_use_magic(summon, key, ctrl_pressed, shift_pressed);
                    if key.id == SWITCH_STANCE_ID {
                        self.summon_switch_mode(summon);
                    }
                }
            }
        }
    }

    /// Java `PetSkillData.getAvailableLevel(summon, skillId)`.
    pub fn pet_skill_available_level(&self, summon: ObjectId, skill_id: i32) -> i32 {
        let Some(npc_id) = self.summon_npc_id(summon) else {
            return 0;
        };
        let Some(skills) = self.summons.pet_skills.as_ref().and_then(|d| d.skills_of(npc_id)) else {
            return 0;
        };
        let table = self.skill_table();
        let summon_level = self.stats(summon).map_or(0, |s| s.level());
        let mut level = 0;
        for holder in skills.iter().filter(|h| h.skill_id == skill_id) {
            if holder.skill_level == 0 {
                if summon_level < 70 {
                    level = summon_level / 10;
                    if level <= 0 {
                        level = 1;
                    }
                } else {
                    level = 7 + ((summon_level - 70) / 5);
                }
                // "formula usable for skill that have 10 or more skill levels"
                let max_level = table.as_ref().map_or(0, |t| t.max_level(holder.skill_id));
                if level > max_level {
                    level = max_level;
                }
                break;
            } else if 1 <= summon_level && holder.skill_level > level {
                level = holder.skill_level;
            }
        }
        level
    }

    /// Java `Summon.useMagic(skill, forceUse, dontMove)`: `true` when the CAST intention was set.
    pub fn summon_use_magic(
        &mut self,
        summon: ObjectId,
        key: SkillKey,
        force_use: bool,
        dont_move: bool,
    ) -> bool {
        let _ = dont_move;
        let Some(owner) = self.summon_owner(summon) else {
            return false;
        };
        if self.is_dead(summon) {
            return false;
        }
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return false;
        };
        // "Just ignore the passive skill request. Why does the client send it anyway??"
        if skill.is_passive() {
            return false;
        }
        // If a skill is currently being used.
        if self.is_casting_now(summon) {
            return false;
        }
        // `_owner.setCurrentPetSkill(skill, forceUse, dontMove)`: Java never reads it back.

        // Get the target for the skill.
        let target = match skill.target_type {
            // OWNER_PET should be cast even if no target has been found.
            TargetType::OwnerPet => Some(owner),
            // PARTY, AURA, SELF should be cast even if no target has been found.
            TargetType::Party
            | TargetType::Aura
            | TargetType::FrontAura
            | TargetType::BehindAura
            | TargetType::SelfTarget
            | TargetType::AuraCorpseMob
            | TargetType::CommandChannel => Some(summon),
            // Get the first target of the list.
            _ => {
                let selected = self.cast_selected_target(summon);
                self.cast_target_list(skill, summon, true, selected)
                    .first()
                    .copied()
            }
        };
        let Some(target) = target else {
            self.send_system(owner, SystemMessageId::YourTargetCannotBeFound);
            return false;
        };
        // Check if this skill is enabled (e.g. reuse time).
        if self.is_skill_disabled(summon, skill) {
            self.send_system(
                owner,
                SystemMessageId::ThatPetServitorSkillCannotBeUsedBecauseItIsRecharging,
            );
            return false;
        }
        // Check if the summon has enough MP.
        let mp = self.core_of(summon).map_or(0.0, |c| c.cur_mp);
        if mp < f64::from(self.mp_consume(summon, skill) + self.mp_initial_consume(summon, skill)) {
            self.send_system(owner, SystemMessageId::NotEnoughMp);
            return false;
        }
        // Check if the summon has enough HP.
        let hp = self.core_of(summon).map_or(0.0, |c| c.cur_hp);
        if hp <= f64::from(skill.hp_consume) {
            self.send_system(owner, SystemMessageId::NotEnoughHp);
            return false;
        }
        // The path check of a physical skill: see `ai.rs` (no synchronous path search).

        // Check if this is bad magic skill.
        if has_negative_effect(skill) {
            if owner == target {
                return false;
            }
            // "Summons can cast skills on NPCs inside peace zones." (`allowPeaceAttack` is not
            // loaded: false.)
            if self.is_inside_peace_zone(summon, summon, target) {
                self.send_system(owner, SystemMessageId::YouMayNotAttackThisTargetInAPeacefulZone);
                return false;
            }
            // Olympiad and siege friends: not modelled.
            let is_door = self
                .objects
                .get(&target)
                .is_some_and(|e| e.kind == ObjectKind::Door);
            if is_door {
                if !self.is_auto_attackable_for(target, owner) {
                    return false;
                }
            } else {
                if !self.can_be_attacked(target) {
                    return false;
                }
                // Check if a Forced attack is in progress on non-attackable target.
                if !self.is_auto_attackable_for(target, summon)
                    && !force_use
                    && !self.is_npc(target)
                    && !matches!(
                        skill.target_type,
                        TargetType::Aura
                            | TargetType::FrontAura
                            | TargetType::BehindAura
                            | TargetType::Clan
                            | TargetType::Party
                            | TargetType::SelfTarget
                    )
                {
                    return false;
                }
            }
        }
        // Notify the AI with CAST and target.
        self.set_intention_cast(summon, key, Some(target));
        true
    }

    /// Java `Summon.canAttack(ctrlPressed)`.
    pub fn summon_can_attack(&mut self, summon: ObjectId, ctrl_pressed: bool) -> bool {
        let Some(owner) = self.summon_owner(summon) else {
            return false;
        };
        let Some(target) = self.target_of(owner) else {
            return false;
        };
        if target == summon || target == owner {
            return false;
        }
        // Sin eater, Big Boom, Wyvern can't attack with attack button.
        if self
            .summon_npc_id(summon)
            .is_some_and(|id| PASSIVE_SUMMONS.contains(&id))
        {
            self.send(owner, &ActionFailed);
            return false;
        }
        if self.is_betrayed(summon) {
            self.send_system(
                owner,
                SystemMessageId::YourPetServitorIsUnresponsiveAndWillNotObeyAnyOrders,
            );
            self.send(owner, &ActionFailed);
            return false;
        }
        if self.is_pet(summon) {
            let level = self.stats(summon).map_or(0, |s| s.level());
            let owner_level = self.core_of(owner).map_or(0, |c| c.level);
            if level - owner_level > 20 {
                self.send_system(owner, SystemMessageId::YourPetIsTooHighLevelToControl);
                self.send(owner, &ActionFailed);
                return false;
            }
        }
        // Olympiad and siege friends: not modelled.
        if self.is_inside_peace_zone(owner, summon, target) {
            self.send_system(owner, SystemMessageId::YouMayNotAttackThisTargetInAPeacefulZone);
            return false;
        }
        if self.is_locked_target(summon) {
            self.send_system(owner, SystemMessageId::FailedToChangeAttackTarget);
            return false;
        }
        // "Summons can attack NPCs even when the owner cannot."
        if !self.is_auto_attackable_for(target, owner) && !ctrl_pressed && !self.is_npc(target) {
            self.set_follow_status(summon, false);
            self.summon_set_intention_follow(summon, Some(target));
            self.send_system(owner, SystemMessageId::InvalidTarget);
            return false;
        }
        // Siege golems AI doesn't support attacking other than doors/walls at the moment.
        let is_door = self
            .objects
            .get(&target)
            .is_some_and(|e| e.kind == ObjectKind::Door);
        if is_door
            && self
                .summon_template(summon)
                .is_none_or(|t| t.race != NpcRace::SiegeWeapon)
        {
            return false;
        }
        let far = match (self.position_of(owner), self.position_of(target)) {
            (Some(a), Some(b)) => crate::target::distance_3d(a, b) > MAX_ATTACK_ORDER_DISTANCE,
            _ => true,
        };
        if far {
            self.summon_set_intention_follow(summon, Some(owner));
            self.send_system(owner, SystemMessageId::YourTargetIsOutOfRange);
            return false;
        }
        if !self.can_see_target(summon, target) {
            self.summon_set_intention_follow(summon, Some(owner));
            self.send_system(owner, SystemMessageId::CannotSeeTarget);
            return false;
        }
        true
    }

    /// Java `CreatureAI.setIntentionMoveTo(loc)` of a summon.
    pub(crate) fn summon_set_intention_move_to(&mut self, summon: ObjectId, x: i32, y: i32, z: i32) {
        self.stop_follow(summon);
        if self.intention_of(summon) == crate::movement::Intention::Rest
            || self.is_all_skills_disabled(summon)
            || self.is_casting_now(summon)
        {
            self.client_action_failed(summon);
            return;
        }
        self.set_intention_value(summon, crate::movement::Intention::MoveTo);
        self.client_stop_auto_attack(summon);
        self.abort_attack(summon);
        self.ai_move_to(summon, x, y, z);
    }

    /// Java `SummonClick.onAction(player, target, interact)`.
    pub(crate) fn summon_click(&mut self, player: ObjectId, summon: ObjectId, interact: bool) {
        // `PetClick` for a pet.
        if self.is_pet(summon) {
            self.pet_click(player, summon, interact);
            return;
        }
        // Aggression target lock effect.
        if self.is_locked_target(player) && self.locked_target(player) != Some(summon) {
            self.send_system(player, SystemMessageId::FailedToChangeAttackTarget);
            return;
        }
        let owner = self.summon_owner(summon);
        if owner == Some(player) && self.target_of(player) == Some(summon) {
            let summon_type = self.summon(summon).map_or(1, |s| s.kind.summon_type());
            self.send(player, &PetStatusShow { summon_type });
            self.update_not_move_until(player);
            self.send(player, &ActionFailed);
            // ON_PLAYER_SUMMON_TALK: script event (D-05).
        } else if self.target_of(player) != Some(summon) {
            self.set_target(player, Some(summon));
        } else if interact {
            if self.summon_auto_attackable_by(summon, player) {
                self.set_intention_attack(player, summon);
                self.on_action_request(player);
            } else {
                // This Action Failed packet avoids player getting stuck when clicking three or
                // more times.
                self.send(player, &ActionFailed);
                let near = match (self.position_of(summon), self.position_of(player)) {
                    (Some(a), Some(b)) => {
                        let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
                        (dx * dx + dy * dy).sqrt() < 150.0
                    }
                    _ => false,
                };
                if near {
                    self.update_not_move_until(player);
                }
                self.creature_set_intention_follow(player, summon);
            }
        }
    }
}

/// `summon.getTemplate().getParameters().getSkillHolder(name)`: a `<skill name=..>` parameter.
fn template_skill(template: &NpcTemplate, name: &str) -> Option<SkillKey> {
    match template.parameters.get(name)? {
        NpcParam::Skill { id, level } => Some(SkillKey::new(*id, *level)),
        _ => None,
    }
}
