//! Soulshots, spiritshots and blessed spiritshots: the item handlers, the charge state of a
//! weapon, the automatic use (`RequestAutoSoulShot`, `ExAutoSoulShot`, `rechargeShots`) and the
//! shots of NPCs.
// Java: dist/game/data/scripts/handlers/items/{SoulShots,SpiritShot,BlessedSpiritShot}.java
// Java: entity/actor/Player.java (rechargeShots, addAutoSoulShot, removeAutoSoulShot,
//       disableAutoShot, disableAutoShotByCrystalType, disableAutoShotsAll, isChargedShot,
//       setChargedShot)
// Java: entity/actor/Npc.java (isChargedShot, setChargedShot, rechargeShots)
// Java: entity/item/instance/Item.java (isChargedShot, setChargedShot, unChargeAllShots)
// Java: entity/item/enums/ShotType.java
// Java: network/clientpackets/RequestAutoSoulShot.java
// Java: mechanics/skill/Skill.java (useSoulShot, useSpiritShot, the end of activateSkill)
//
// # Where the charge lives
// Java keeps the charge as a bit mask on the weapon `Item` (a player's `isChargedShot` asks the
// item in the right hand) and on the `Npc` itself. Here the masks are in a side table keyed by
// the *weapon's object id* (or the NPC's id): the item moves between the inventory and the
// paperdoll with its mask, as in Java, and a weapon that is taken off keeps its charge.
// `ShotsWorld::owned` remembers which items of a player have an entry so they go when the player
// leaves. The mask is not stored in the database (Java neither).
//
// # Who reads it
// - `combat/attack.rs`: the soulshot flag of `Attack` and the damage, discharged after a hit.
// - `skills/cast.rs`: the 0.6 factor of the cast time with spiritshots, `rechargeShots` at the hit
//   of a cast.
// - `effects/apply.rs`: the discharge at the end of `Skill.activateSkill`, the blessed spiritshot
//   in the landing rate of a debuff.
// - The damage and heal effects (M5-EFX-04/05) call [`World::is_charged_shot`] and
//   [`World::skill_uses_soul_shot`] / [`World::skill_uses_spirit_shot`] like the Java handlers.
//
// # Left out
// - Beast shots (`BeastSoulShot`, `BeastSpiritShot`) are `summon/shots.rs`; fishing shots are
//   not ported.
// - The olympiad (`OlympiadConfig.OLYMPIAD_DISABLE_BLESSED_SPIRITSHOTS`, `disableAutoShotsAll`
//   at the start of a match): no olympiad yet.
// - Fake players (`FAKE_PLAYER_USE_SHOTS`).

use std::collections::{BTreeSet, HashMap};

use l2_data::SkillTemplate;
use l2_data::item::types::ActionType;
use l2_data::item::{CrystalType, ItemTemplate, SkillRef};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{ExAutoSoulShot, MagicSkillUse, SkillPosition, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::EffectType;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::player::ItemData;
use crate::world::World;

/// The range of the soulshot animation (`broadcastToSelfAndVisiblePlayersInRange(.., 600)`).
const SOULSHOT_ANIMATION_RANGE: i32 = 600;
/// The skill an NPC's soulshot animation shows (`Npc.rechargeShots`).
const NPC_SOULSHOT_SKILL: i32 = 2154;
/// The skill an NPC's spiritshot animation shows (`Npc.rechargeShots`).
const NPC_SPIRITSHOT_SKILL: i32 = 2159;

/// Java `ShotType`. The mask of a type is `1 << ordinal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotType {
    Soulshots,
    Spiritshots,
    BlessedSpiritshots,
    FishSoulshots,
}

impl ShotType {
    /// Java `ShotType.getMask()`.
    pub const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

/// The three item handlers of this module, by the name the item template gives
/// (`<set name="handler" val="SoulShots" />`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotHandler {
    SoulShots,
    SpiritShot,
    BlessedSpiritShot,
    /// The summon's soulshots (`summon/shots.rs`).
    BeastSoulShot,
    /// The summon's spiritshots and blessed spiritshots (`summon/shots.rs`).
    BeastSpiritShot,
}

impl ShotHandler {
    /// The handler registered under a handler name, if it is one of the shot handlers.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "SoulShots" => Some(Self::SoulShots),
            "SpiritShot" => Some(Self::SpiritShot),
            "BlessedSpiritShot" => Some(Self::BlessedSpiritShot),
            "BeastSoulShot" => Some(Self::BeastSoulShot),
            "BeastSpiritShot" => Some(Self::BeastSpiritShot),
            _ => None,
        }
    }

    /// Java `ItemHandler.getHandler(item.getEtcItem())` for the shot handlers.
    pub fn of_template(template: &ItemTemplate) -> Option<Self> {
        template.etc()?.handler.as_deref().and_then(Self::from_name)
    }
}

/// The shot state of every creature (a side table, D-11).
#[derive(Debug, Default)]
pub struct ShotsWorld {
    /// `_shotsMask`: by weapon item object id (players) or NPC id.
    charged: HashMap<ObjectId, u8>,
    /// The weapons of a player that have an entry in `charged`.
    owned: HashMap<ObjectId, Vec<ObjectId>>,
    /// `Player._activeSoulShots`: the item ids set to be used automatically.
    // NOTE: Java iterates a hash set; a sorted set gives the same order for the small ids shots
    // have, and the order only decides in which order the messages of a recharge arrive.
    auto: HashMap<ObjectId, BTreeSet<i32>>,
    /// `Npc._soulshotamount` / `_spiritshotamount`: what an NPC has left. Missing = the template's
    /// values (`Npc.onSpawn` resets them; the entry goes when the NPC leaves the world).
    npc_ammo: HashMap<ObjectId, (i32, i32)>,
}

/// What a weapon says about shots (`Weapon.getSoulShotCount()` & co).
#[derive(Debug, Clone, Copy, Default)]
struct WeaponShots {
    soulshot_count: i32,
    spiritshot_count: i32,
    reduced_soulshot: i32,
    reduced_soulshot_chance: i32,
}

impl World {
    // ------------------------------------------------------------------ the charge

    /// The object that carries the charge of `id`: a player's weapon in the right hand
    /// (`getActiveWeaponInstance`), an NPC itself.
    fn shot_holder(&self, id: ObjectId) -> Option<ObjectId> {
        if self.players.contains_key(&id) {
            self.paperdoll_object(id, paperdoll::RHAND)
        } else if self.npcs.contains_key(&id) {
            Some(id)
        } else {
            None
        }
    }

    /// Java `isChargedShot(type)` (`Player`: the weapon in hand, `Npc`: itself; nothing else is
    /// ported yet).
    pub fn is_charged_shot(&self, id: ObjectId, shot: ShotType) -> bool {
        self.shot_holder(id)
            .and_then(|holder| self.shots.charged.get(&holder))
            .is_some_and(|mask| mask & shot.mask() == shot.mask())
    }

    /// Java `setChargedShot(type, charged)`.
    pub fn set_charged_shot(&mut self, id: ObjectId, shot: ShotType, charged: bool) {
        let Some(holder) = self.shot_holder(id) else {
            return;
        };
        if charged {
            if holder != id {
                let owned = self.shots.owned.entry(id).or_default();
                if !owned.contains(&holder) {
                    owned.push(holder);
                }
            }
            *self.shots.charged.entry(holder).or_insert(0) |= shot.mask();
        } else if let Some(mask) = self.shots.charged.get_mut(&holder) {
            *mask &= !shot.mask();
        }
    }

    /// Java `Item.unChargeAllShots()` for the weapon of a player.
    pub(crate) fn un_charge_all_shots(&mut self, item: ObjectId) {
        self.shots.charged.remove(&item);
    }

    /// Everything the shots know about a creature that left the world.
    pub(crate) fn forget_shots(&mut self, id: ObjectId) {
        self.shots.charged.remove(&id);
        for item in self.shots.owned.remove(&id).unwrap_or_default() {
            self.shots.charged.remove(&item);
        }
        self.shots.auto.remove(&id);
        self.shots.npc_ammo.remove(&id);
    }

    // ------------------------------------------------------------------ skills

    /// Java `Skill.useSoulShot()`: the skill has a physical attack effect.
    pub fn skill_uses_soul_shot(&mut self, skill: &SkillTemplate) -> bool {
        self.skill_has_effect_type(
            skill,
            &[EffectType::PhysicalAttack, EffectType::PhysicalAttackHpLink],
        )
    }

    /// Java `Skill.useSpiritShot()`.
    pub fn skill_uses_spirit_shot(&self, skill: &SkillTemplate) -> bool {
        skill.is_magic()
    }

    /// The end of Java `Skill.activateSkill` (no cubic): the charge of the shots the skill uses
    /// is gone.
    pub(crate) fn discharge_skill_shots(&mut self, caster: ObjectId, skill: &SkillTemplate) {
        if self.skill_uses_spirit_shot(skill) {
            let shot = if self.is_charged_shot(caster, ShotType::BlessedSpiritshots) {
                ShotType::BlessedSpiritshots
            } else {
                ShotType::Spiritshots
            };
            self.set_charged_shot(caster, shot, false);
        } else if self.skill_uses_soul_shot(skill) {
            self.set_charged_shot(caster, ShotType::Soulshots, false);
        }
    }

    // ------------------------------------------------------------------ rechargeShots

    /// Java `rechargeShots(physical, magic)`: a player uses its automatic shots, an NPC loads from
    /// what it has left.
    pub fn recharge_shots(&mut self, id: ObjectId, physical: bool, magic: bool) {
        if self.players.contains_key(&id) {
            self.recharge_player_shots(id, physical, magic);
        } else if self.is_summon(id) {
            // `Summon.rechargeShots`: the owner's automatic beast shots.
            self.summon_recharge_shots(id, physical, magic);
        } else if self.npcs.contains_key(&id) {
            self.recharge_npc_shots(id, physical, magic);
        }
    }

    /// Java `Player.rechargeShots`.
    fn recharge_player_shots(&mut self, id: ObjectId, physical: bool, magic: bool) {
        let Some(shots) = self.shots.auto.get(&id).filter(|s| !s.is_empty()).cloned() else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        for item_id in shots {
            let Some(item) = self.first_item_with_id(id, item_id) else {
                self.remove_auto_soul_shot(id, item_id);
                continue;
            };
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            if magic && template.default_action == ActionType::SpiritShot {
                self.use_shot_handler_of(id, &item, template);
            }
            if physical && template.default_action == ActionType::SoulShot {
                self.use_shot_handler_of(id, &item, template);
            }
        }
    }

    /// `ItemHandler.getInstance().getHandler(item.getEtcItem())` + `handler.onItemUse(this, item,
    /// false)`.
    fn use_shot_handler_of(&mut self, id: ObjectId, item: &ItemData, template: &ItemTemplate) {
        if let Some(handler) = ShotHandler::of_template(template) {
            self.use_shot_item(id, item, handler);
        }
    }

    /// Java `Npc.rechargeShots`.
    fn recharge_npc_shots(&mut self, id: ObjectId, physical: bool, magic: bool) {
        let Some((soul_chance, spirit_chance, template_ammo)) =
            self.stats(id).and_then(|v| v.npc_template()).map(|t| {
                (
                    t.soul_shot_chance,
                    t.spirit_shot_chance,
                    (t.soul_shot, t.spirit_shot),
                )
            })
        else {
            return;
        };
        // Fake players with `FakePlayerUseShots`: not ported.
        let (mut soul, mut spirit) = self.shots.npc_ammo.get(&id).copied().unwrap_or(template_ammo);
        if soul > 0 || spirit > 0 {
            // NOTE(java-quirk): a failed soulshot roll returns, so an NPC that wants both kinds
            // never gets the spiritshot in that call; and the roll is `get(100) > chance`, so a
            // chance of 100 always passes and a chance of 0 still passes once in a hundred.
            'recharge: {
                if physical {
                    if soul == 0 || self.rng().below(100) > soul_chance {
                        break 'recharge;
                    }
                    soul -= 1;
                    self.broadcast_self_skill_use(id, NPC_SOULSHOT_SKILL, 1);
                    self.set_charged_shot(id, ShotType::Soulshots, true);
                }
                if magic {
                    if spirit == 0 || self.rng().below(100) > spirit_chance {
                        break 'recharge;
                    }
                    spirit -= 1;
                    self.broadcast_self_skill_use(id, NPC_SPIRITSHOT_SKILL, 1);
                    self.set_charged_shot(id, ShotType::Spiritshots, true);
                }
            }
        }
        self.shots.npc_ammo.insert(id, (soul, spirit));
    }

    /// `new MagicSkillUse(creature, creature, skillId, level, 0, 0)`; `None` when the creature
    /// has no position.
    fn self_skill_use(&self, id: ObjectId, skill_id: i32, level: i32) -> Option<MagicSkillUse> {
        let (x, y, z) = self.position_of(id)?;
        let here = SkillPosition { x, y, z };
        // The constructor reads the ground point of a player's last ground skill.
        let ground = if self.is_player(id) {
            self.cast_state(id)
                .and_then(|s| s.current_skill_world_position)
                .map(|(x, y, z)| SkillPosition { x, y, z })
        } else {
            None
        };
        Some(MagicSkillUse {
            creature_id: id.0,
            target_id: id.0,
            skill_id,
            skill_level: level,
            hit_time: 0,
            reuse_delay: 0,
            creature: here,
            ground_location: ground,
            target: here,
        })
    }

    /// `broadcastPacket(new MagicSkillUse(this, this, skillId, level, 0, 0))`.
    fn broadcast_self_skill_use(&self, id: ObjectId, skill_id: i32, level: i32) {
        if let Some(packet) = self.self_skill_use(id, skill_id, level) {
            self.broadcast_creature_packet(id, &packet);
        }
    }

    // ------------------------------------------------------------------ auto shots

    /// Java `Player.addAutoSoulShot`.
    pub(crate) fn add_auto_soul_shot(&mut self, id: ObjectId, item_id: i32) {
        self.shots.auto.entry(id).or_default().insert(item_id);
    }

    /// Java `Player.removeAutoSoulShot`: whether it was set.
    pub(crate) fn remove_auto_soul_shot(&mut self, id: ObjectId, item_id: i32) -> bool {
        self.shots.auto.get_mut(&id).is_some_and(|s| s.remove(&item_id))
    }

    /// Java `Player.getAutoSoulShot().contains(itemId)`.
    pub fn has_auto_soul_shot(&self, id: ObjectId, item_id: i32) -> bool {
        self.shots.auto.get(&id).is_some_and(|s| s.contains(&item_id))
    }

    /// Java `Player.getAutoSoulShot()`, in ascending order.
    pub fn auto_soul_shots(&self, id: ObjectId) -> Vec<i32> {
        self.shots
            .auto
            .get(&id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Java `Player.disableAutoShot(itemId)`: switches it off and tells the client; whether it was
    /// on.
    pub fn disable_auto_shot(&mut self, id: ObjectId, item_id: i32) -> bool {
        if !self.has_auto_soul_shot(id, item_id) {
            return false;
        }
        self.remove_auto_soul_shot(id, item_id);
        self.send(
            id,
            &ExAutoSoulShot {
                item_id,
                shot_type: ExAutoSoulShot::OFF,
            },
        );
        let templates = self.item_templates();
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::TheAutomaticUseOfS1HasBeenDeactivated)
                .with(item_name_param(templates.as_deref(), item_id)),
        );
        true
    }

    /// Java `Player.disableAutoShotByCrystalType(crystalType)`: every automatic shot of that
    /// grade is switched off.
    pub fn disable_auto_shot_by_crystal_type(&mut self, id: ObjectId, crystal_type: i32) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        for item_id in self.auto_soul_shots(id) {
            if templates
                .get(item_id)
                .is_some_and(|t| t.crystal_type.level() == crystal_type)
            {
                self.disable_auto_shot(id, item_id);
            }
        }
    }

    /// Java `Player.disableAutoShotsAll()`.
    pub fn disable_auto_shots_all(&mut self, id: ObjectId) {
        let templates = self.item_templates();
        for item_id in self.auto_soul_shots(id) {
            self.send(
                id,
                &ExAutoSoulShot {
                    item_id,
                    shot_type: ExAutoSoulShot::OFF,
                },
            );
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheAutomaticUseOfS1HasBeenDeactivated)
                    .with(item_name_param(templates.as_deref(), item_id)),
            );
        }
        self.shots.auto.remove(&id);
    }

    /// The part of Java `Shortcuts.deleteShortcut` that concerns shots: a shortcut to a shot item
    /// is removed, so its automatic use ends (the client is told before the shortcut list).
    pub(crate) fn shortcut_removed_for_item(&mut self, id: ObjectId, item_id: i32, is_shot: bool) {
        if is_shot && self.remove_auto_soul_shot(id, item_id) {
            self.send(
                id,
                &ExAutoSoulShot {
                    item_id,
                    shot_type: ExAutoSoulShot::OFF,
                },
            );
        }
    }

    /// The end of `Shortcuts.deleteShortcut`: every automatic shot is announced again after the
    /// new shortcut list.
    pub(crate) fn resend_auto_soul_shots(&self, id: ObjectId) {
        for item_id in self.auto_soul_shots(id) {
            self.send(
                id,
                &ExAutoSoulShot {
                    item_id,
                    shot_type: ExAutoSoulShot::ON,
                },
            );
        }
    }

    /// Java `RequestAutoSoulShot.runImpl`.
    pub(crate) fn on_request_auto_soul_shot(&mut self, id: ObjectId, item_id: i32, shot_type: i32) {
        // `!isInStoreMode() && getActiveRequester() == null`: no stores yet.
        if self.active_requester(id).is_some() || self.is_dead(id) {
            return;
        }
        let Some(item) = self.first_item_with_id(id, item_id) else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item.item_id) else {
            return;
        };
        // Disabling blessed spiritshots in the olympiad: no olympiad yet.
        if shot_type == ExAutoSoulShot::ON {
            if !self.can_manipulate_with_item_id(id, item.item_id) {
                self.send_message(id, "Cannot use this item.");
                return;
            }
            // Fishing shots are not automatic on retail.
            if (6535..=6540).contains(&item_id) {
                return;
            }
            // Attempt to charge first shot on activation: the beast shots (`summon/shots.rs`).
            if self.auto_soul_shot_beast_on(id, &item, shot_type) {
                return;
            }
            // `getActiveWeaponItem() != getFistsWeaponItem()` means a weapon is in hand.
            let weapon_crystal = self.weapon_instance(id).and_then(|_| {
                self.stats(id)
                    .and_then(|v| v.active_weapon_item().map(|w| w.crystal_type_plus()))
            });
            // NOTE(java-quirk): the shot's own grade is compared with the weapon's grade plus
            // (S80 and S84 count as S), not with the shot's `getCrystalTypePlus()`.
            if weapon_crystal != Some(template.crystal_type) {
                let spirit = matches!(item_id, 2509..=2514 | 3947..=3952 | 5790 | 22072..=22081);
                let message = if spirit {
                    SystemMessageId::YourSpiritshotDoesNotMatchTheWeaponSGrade
                } else {
                    SystemMessageId::TheSoulshotYouAreAttemptingToUseDoesNotMatchTheGradeOfYourEquippedWeapon
                };
                self.send(id, &SystemMessage::new(message));
            }
            // NOTE(java-quirk): a shot of the wrong grade is switched on all the same.
            self.add_auto_soul_shot(id, item_id);
            self.send(id, &ExAutoSoulShot { item_id, shot_type });
            // Start the auto soulshot use.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheAutomaticUseOfS1HasBeenActivated)
                    .with(item_name_param(Some(&templates), item.item_id)),
            );
            self.recharge_shots(id, true, true);
        } else if shot_type == ExAutoSoulShot::OFF {
            self.remove_auto_soul_shot(id, item_id);
            self.send(id, &ExAutoSoulShot { item_id, shot_type });
            // Cancel the auto soulshot use.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheAutomaticUseOfS1HasBeenDeactivated)
                    .with(item_name_param(Some(&templates), item.item_id)),
            );
        }
    }

    // ------------------------------------------------------------------ the handlers

    /// The first item of the player with this template id (`Inventory.getItemByItemId`).
    fn first_item_with_id(&self, id: ObjectId, item_id: i32) -> Option<ItemData> {
        self.players
            .get(&id)?
            .data
            .items
            .iter()
            .find(|i| i.item_id == item_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
            .cloned()
    }

    /// Java `getActiveWeaponInstance()`.
    fn weapon_instance(&self, id: ObjectId) -> Option<ItemData> {
        let object_id = self.paperdoll_object(id, paperdoll::RHAND)?;
        self.inventory_item(id, object_id).cloned()
    }

    /// The weapon in hand with its shot numbers and grade; `None` without a weapon instance
    /// (`weaponInst == null`).
    fn weapon_for_shots(&self, id: ObjectId) -> Option<(WeaponShots, CrystalType)> {
        self.weapon_instance(id)?;
        let weapon = self.stats(id).and_then(|v| v.active_weapon_item())?;
        let shots = weapon
            .weapon()
            .map_or_else(WeaponShots::default, |w| WeaponShots {
                soulshot_count: w.soulshot_count,
                spiritshot_count: w.spiritshot_count,
                reduced_soulshot: w.reduced_soulshot,
                reduced_soulshot_chance: w.reduced_soulshot_chance,
            });
        Some((shots, weapon.crystal_type_plus()))
    }

    /// `UseItem` for a shot: the handler named by the template. `true` when it charged a shot.
    pub(crate) fn use_shot_item(&mut self, id: ObjectId, item: &ItemData, handler: ShotHandler) -> bool {
        match handler {
            ShotHandler::BeastSoulShot => return self.use_beast_shot(id, item, false),
            ShotHandler::BeastSpiritShot => return self.use_beast_shot(id, item, true),
            _ => {}
        }
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        let Some(skill) = self.shot_skill(template, handler) else {
            return false;
        };
        match handler {
            ShotHandler::SoulShots => self.use_soul_shots(id, item, template, skill),
            ShotHandler::SpiritShot => {
                self.use_spirit_shots(id, item, template, skill, ShotType::Spiritshots)
            }
            ShotHandler::BlessedSpiritShot => {
                self.use_spirit_shots(id, item, template, skill, ShotType::BlessedSpiritshots)
            }
            ShotHandler::BeastSoulShot | ShotHandler::BeastSpiritShot => false,
        }
    }

    /// `item.getTemplate().getSkills()`; `skills[0]` is the animation.
    fn shot_skill(&self, template: &ItemTemplate, handler: ShotHandler) -> Option<SkillRef> {
        let skill = template
            .skills
            .item_skills
            .as_ref()
            .and_then(|skills| skills.first().copied());
        if skill.is_none() {
            warn!("{handler:?}: is missing skills!");
        }
        skill
    }

    /// Java `SoulShots.onItemUse`.
    fn use_soul_shots(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        skill: SkillRef,
    ) -> bool {
        let item_id = item.item_id;
        // Check if Soul shot can be used.
        let weapon = self.weapon_for_shots(id).filter(|(w, _)| w.soulshot_count != 0);
        let Some((weapon, weapon_grade)) = weapon else {
            if !self.has_auto_soul_shot(id, item_id) {
                self.send(id, &SystemMessage::new(SystemMessageId::CannotUseSoulshots));
            }
            return false;
        };
        let grade_check = template.is_etc_item()
            && template.default_action == ActionType::SoulShot
            && weapon_grade == template.crystal_type_plus();
        if !grade_check {
            if !self.has_auto_soul_shot(id, item_id) {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::TheSoulshotYouAreAttemptingToUseDoesNotMatchTheGradeOfYourEquippedWeapon,
                    ),
                );
            }
            return false;
        }
        // Check if Soul shot is already active.
        if self.is_charged_shot(id, ShotType::Soulshots) {
            return false;
        }
        // Consume Soul shots if player has enough of them.
        let mut count = weapon.soulshot_count;
        if weapon.reduced_soulshot > 0 && self.rng().below(100) < weapon.reduced_soulshot_chance {
            count = weapon.reduced_soulshot;
        }
        if !self.destroy_item_by_item_id(id, item_id, i64::from(count), false) {
            if !self.disable_auto_shot(id, item_id) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSoulshotsForThat),
                );
            }
            return false;
        }
        // Charge soul shot.
        self.set_charged_shot(id, ShotType::Soulshots, true);
        // Send message to client.
        self.send(id, &SystemMessage::new(SystemMessageId::YourSoulshotsAreEnabled));
        if let Some(packet) = self.self_skill_use(id, skill.id, skill.level) {
            self.send(id, &packet);
            self.broadcast_packet_in_range(id, &packet, SOULSHOT_ANIMATION_RANGE);
        }
        true
    }

    /// Java `SpiritShot.onItemUse` and `BlessedSpiritShot.onItemUse` (they differ in the charge
    /// they set and in the olympiad rule, which is not modelled).
    fn use_spirit_shots(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        skill: SkillRef,
        shot: ShotType,
    ) -> bool {
        let item_id = item.item_id;
        // Check if Spirit shot can be used.
        let weapon = self.weapon_for_shots(id).filter(|(w, _)| w.spiritshot_count != 0);
        let Some((weapon, weapon_grade)) = weapon else {
            if !self.has_auto_soul_shot(id, item_id) {
                self.send(id, &SystemMessage::new(SystemMessageId::YouMayNotUseSpiritshots));
            }
            return false;
        };
        // Check if Spirit shot is already active (a blessed one can be charged over a spirit one).
        if self.is_charged_shot(id, shot) {
            return false;
        }
        // Check for correct grade.
        let grade_check = template.is_etc_item()
            && template.default_action == ActionType::SpiritShot
            && weapon_grade == template.crystal_type_plus();
        if !grade_check {
            if !self.has_auto_soul_shot(id, item_id) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YourSpiritshotDoesNotMatchTheWeaponSGrade),
                );
            }
            return false;
        }
        // Consume Spirit shot if player has enough of them.
        if !self.destroy_item_by_item_id(id, item_id, i64::from(weapon.spiritshot_count), false) {
            if !self.disable_auto_shot(id, item_id) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSpiritshotForThat),
                );
            }
            return false;
        }
        // Charge Spirit shot.
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YourSpiritshotHasBeenEnabled),
        );
        self.set_charged_shot(id, shot, true);
        self.broadcast_self_skill_use(id, skill.id, skill.level);
        true
    }
}
