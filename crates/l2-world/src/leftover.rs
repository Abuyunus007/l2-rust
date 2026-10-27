//! Client leftovers of L-02 that are too small for a module of their own: the key mapping, the
//! inventory order, the mini map, "record info", rotating with the keyboard, the skill list, the
//! dispel of a buff, a skill on the ground and the item link in the chat. Macros, the crystallize
//! and the wear shop have their own modules (`macros.rs`, `crystallize.rs`, `preview.rs`).
// Java: network/clientpackets/{RequestSaveKeyMapping,RequestSaveInventoryOrder,RequestShowMiniMap,
// Java: RequestRecordInfo,StartRotating,FinishRotating,RequestSkillList,RequestDispel,
// Java: RequestExMagicSkillUseGround,RequestExRqItemLink,MoveWithDelta}.java
//
// # What is not Java's
// - `Item.publish()` (an item shown in the chat can be looked up by its link) is a set on the
//   *player who showed it* (`LeftoverState::published`). Java keeps the flag on the item object,
//   which stays published after a trade or while it lies on the ground; here the flag is gone
//   when the player who published the item leaves. Java drops the player's items from the world
//   at logout and loads new objects at the next login, so for the items the player keeps it is
//   the same.
// - `Player.isInCrystallize()` has no state: the world handles one packet at a time.
// - The rotation of a captain turns the air ship (`vehicle/airship.rs`), as in Java.

use std::collections::HashSet;

use l2_protocol_game::client::{
    RequestCrystallizeItem, RequestDispel, RequestExMagicSkillUseGround, RequestMakeMacro, RequestPreviewItem,
};
use l2_protocol_game::server::{
    ActionFailed, ExRpItemLink, ShowMiniMap, StartRotation, StopRotation, ValidateLocation,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillFinishType;
use crate::movement::heading_from_points;
use crate::world::World;
use l2_data::stats::AbnormalType;

/// The name of the player variable (`PlayerVariables.UI_KEY_MAPPING`).
pub const UI_KEY_MAPPING: &str = "UI_KEY_MAPPING";
/// Java `ShowMiniMap(1665)`: the id of the map `RequestShowMiniMap` shows.
pub const MINI_MAP_ID: i32 = 1665;
const LOC_INVENTORY: &str = "INVENTORY";

/// The configuration values these packets read. `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverSettings {
    /// `PlayerConfig.DANCE_CANCEL_BUFF` (`DanceCancelBuff`).
    pub dance_cancel_buff: bool,
    /// `GeneralConfig.WEAR_PRICE` (`WearPrice`): the adena one tried-on item costs.
    pub wear_price: i32,
    /// `GeneralConfig.WEAR_DELAY` (`WearDelay`): the seconds the try-on lasts.
    pub wear_delay: i32,
}

impl Default for LeftoverSettings {
    fn default() -> Self {
        Self {
            dance_cancel_buff: false,
            wear_price: 10,
            wear_delay: 5,
        }
    }
}

/// What the world keeps per player for these packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverState {
    /// Java `MacroList._revision`. The enter-world list is revision 2 (`macro_lists`).
    pub macro_revision: i32,
    /// Java `MacroList._macroId`: the next id for a new macro.
    pub macro_id: i32,
    /// The items this player showed in the chat (Java `Item.publish()`).
    pub published: HashSet<ObjectId>,
}

impl Default for LeftoverState {
    fn default() -> Self {
        Self {
            macro_revision: 2,
            macro_id: 1000,
            published: HashSet::new(),
        }
    }
}

/// The timers of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftoverTimer {
    /// Java `RequestPreviewItem.RemoveWearItemsTask`: the try-on of this player is over.
    WearEnd(ObjectId),
}

/// The packets of this module as the session hands them to the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeftoverInput {
    MakeMacro(RequestMakeMacro),
    DeleteMacro {
        id: i32,
    },
    Crystallize(RequestCrystallizeItem),
    PreviewItem(RequestPreviewItem),
    ExRqItemLink {
        object_id: i32,
    },
    /// `None`: the packet had no bytes (Java does nothing).
    SaveKeyMapping(Option<Vec<u8>>),
    /// `(object id, order)` pairs.
    SaveInventoryOrder(Vec<(i32, i32)>),
    ShowMiniMap,
    RecordInfo,
    StartRotating {
        degree: i32,
        side: i32,
    },
    FinishRotating {
        degree: i32,
    },
    SkillList,
    Dispel(RequestDispel),
    MagicSkillUseGround(RequestExMagicSkillUseGround),
}

impl World {
    /// Hands a packet of this module to its handler.
    pub(crate) fn on_leftover_input(&mut self, id: ObjectId, input: LeftoverInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            LeftoverInput::MakeMacro(request) => self.on_request_make_macro(id, request),
            LeftoverInput::DeleteMacro { id: macro_id } => self.on_request_delete_macro(id, macro_id),
            LeftoverInput::Crystallize(request) => self.on_request_crystallize_item(id, request),
            LeftoverInput::PreviewItem(request) => self.on_request_preview_item(id, request),
            LeftoverInput::ExRqItemLink { object_id } => self.on_request_ex_rq_item_link(id, object_id),
            LeftoverInput::SaveKeyMapping(bytes) => self.on_request_save_key_mapping(id, bytes),
            LeftoverInput::SaveInventoryOrder(order) => self.on_request_save_inventory_order(id, &order),
            LeftoverInput::ShowMiniMap => self.on_request_show_mini_map(id),
            LeftoverInput::RecordInfo => self.on_request_record_info(id),
            LeftoverInput::StartRotating { degree, side } => self.on_start_rotating(id, degree, side),
            LeftoverInput::FinishRotating { degree } => self.on_finish_rotating(id, degree),
            // Java `player.sendSkillList()`.
            LeftoverInput::SkillList => self.send_skill_list(id),
            LeftoverInput::Dispel(request) => self.on_request_dispel(id, &request),
            LeftoverInput::MagicSkillUseGround(request) => {
                self.on_request_magic_skill_use_ground(id, &request)
            }
        }
    }

    /// The timers of this module.
    pub(crate) fn on_leftover_timer(&mut self, timer: LeftoverTimer) {
        match timer {
            LeftoverTimer::WearEnd(id) => self.on_wear_end(id),
        }
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, message, DEFAULT_PUNISH)`.
    pub(crate) fn leftover_illegal_action(&mut self, id: ObjectId, message: &str) {
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, message, punishment);
    }

    /// Java `RequestSaveKeyMapping.runImpl`: the bytes are stored as the player variable
    /// `UI_KEY_MAPPING`, each as a signed decimal followed by a tab.
    fn on_request_save_key_mapping(&mut self, id: ObjectId, bytes: Option<Vec<u8>>) {
        let Some(bytes) = bytes else {
            return;
        };
        if !self.settings.store_ui_settings {
            return;
        }
        // `sb.append(b).append(SPLIT_VAR)` with a boxed `Byte`: negative bytes print as such.
        let mut text = String::with_capacity(bytes.len() * 4);
        for byte in bytes {
            text.push_str(&(byte as i8).to_string());
            text.push('\t');
        }
        self.set_player_variable_text(id, UI_KEY_MAPPING, text);
    }

    /// Java `PlayerVariables.set(name, String)`: the value is stored with the next store of the
    /// character (`character_variables`).
    pub(crate) fn set_player_variable_text(&mut self, id: ObjectId, name: &str, value: String) {
        if let Some(player) = self.players.get_mut(&id) {
            let profile = &mut player.data.profile;
            profile.variables.insert(name.to_owned(), value);
            profile.variables_removed.remove(name);
            profile.variables_changed.insert(name.to_owned());
        }
    }

    /// Java `RequestSaveInventoryOrder.runImpl`: an item in the inventory (not worn) gets the slot
    /// the client says. The next store writes `loc_data`.
    fn on_request_save_inventory_order(&mut self, id: ObjectId, order: &[(i32, i32)]) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        for &(object_id, slot) in order {
            let item = player
                .data
                .items
                .iter_mut()
                .find(|i| i.object_id.0 == object_id && i.loc == LOC_INVENTORY);
            if let Some(item) = item {
                item.loc_data = slot;
            }
        }
    }

    /// Java `RequestShowMiniMap.runImpl`.
    fn on_request_show_mini_map(&mut self, id: ObjectId) {
        self.send(
            id,
            &ShowMiniMap {
                map_id: MINI_MAP_ID,
                // Java `SevenSigns.getInstance().getCurrentPeriod()` (M8-SS-01).
                seven_signs_period: self.ss_current_period() as u8,
            },
        );
    }

    /// Java `RequestRecordInfo.runImpl`: the client gets its own info and the info of everything
    /// it sees again.
    fn on_request_record_info(&mut self, id: ObjectId) {
        self.send_user_info(id);
        for object in self.visible_objects(id) {
            if !self.is_visible_for(object, id) {
                continue;
            }
            self.send_info(object, id);
            // `creature.hasAI()` -> `describeStateToPlayer(player)`.
            let creature = self.objects.get(&object).is_some_and(|e| e.kind.is_creature());
            if creature {
                self.describe_state_to_player(object, id);
            }
        }
    }

    /// Java `StartRotating.runImpl`.
    fn on_start_rotating(&mut self, id: ObjectId, degree: i32, side: i32) {
        if !self.settings.keyboard_movement {
            return;
        }
        // `isInAirShip() && getAirShip().isCaptain(this)`: the ship turns, not the player.
        if let Some(ship) = self.steered_air_ship(id) {
            self.broadcast_packet(
                ship,
                &StartRotation {
                    object_id: ship.0,
                    degree,
                    side,
                    speed: 0,
                },
            );
            return;
        }
        self.broadcast_packet(
            id,
            &StartRotation {
                object_id: id.0,
                degree,
                side,
                speed: 0,
            },
        );
    }

    /// Java `FinishRotating.runImpl`.
    fn on_finish_rotating(&mut self, id: ObjectId, degree: i32) {
        if !self.settings.keyboard_movement {
            return;
        }
        if let Some(ship) = self.steered_air_ship(id) {
            self.set_air_ship_heading(ship, degree);
            self.broadcast_packet(
                ship,
                &StopRotation {
                    object_id: ship.0,
                    degree,
                    speed: 0,
                },
            );
            return;
        }
        self.set_heading(id, degree);
        self.broadcast_packet(
            id,
            &StopRotation {
                object_id: id.0,
                degree,
                speed: 0,
            },
        );
    }

    /// Java `RequestDispel.runImpl`: the player takes a buff off itself or its summon.
    fn on_request_dispel(&mut self, id: ObjectId, request: &RequestDispel) {
        if request.skill_id <= 0 || request.skill_level <= 0 {
            return;
        }
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get_skill(request.skill_id, request.skill_level) else {
            return;
        };
        if !skill.can_be_dispeled || skill.stay_after_death || skill.is_debuff {
            return;
        }
        if skill.abnormal_type == AbnormalType::Transform {
            return;
        }
        if skill.is_dance() && !self.settings.leftover.dance_cancel_buff {
            return;
        }
        if id.0 == request.object_id {
            self.stop_skill_effects(id, SkillFinishType::Removed, request.skill_id);
        } else if let Some(summon) = self.summon_of(id)
            && summon.0 == request.object_id
        {
            self.stop_skill_effects(summon, SkillFinishType::Removed, request.skill_id);
        }
    }

    /// Java `RequestExMagicSkillUseGround.runImpl`.
    fn on_request_magic_skill_use_ground(&mut self, id: ObjectId, request: &RequestExMagicSkillUseGround) {
        // Get the level of the used skill.
        let Some(key) = self.known_skill(id, request.skill_id) else {
            self.send(id, &ActionFailed);
            return;
        };
        let exists = self
            .skill_table()
            .and_then(|t| t.get_skill(key.id, key.level).map(|_| ()))
            .is_some();
        if !exists {
            self.send(id, &ActionFailed);
            warn!("No skill found with id {} and level {} !!", key.id, key.level);
            return;
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        self.set_current_skill_world_position(id, Some((request.x, request.y, request.z)));
        // Normally magicskilluse packet turns char client side but for these skills, it doesn't
        // (even with correct target).
        let heading = heading_from_points(x, y, request.x, request.y);
        self.set_heading(id, heading);
        let packet = ValidateLocation {
            object_id: id.0,
            x,
            y,
            z,
            heading,
        };
        // `World.broadcastToVisiblePlayers`: no check of the invisibility, the player itself
        // is left out.
        for viewer in self.visible_players(id) {
            self.send(viewer, &packet);
        }
        self.use_magic(id, key, request.ctrl_pressed, request.shift_pressed);
    }

    /// Java `RequestExRqItemLink.runImpl`: the item a player showed in the chat, found by its
    /// object id, goes to whoever clicked the link.
    fn on_request_ex_rq_item_link(&mut self, id: ObjectId, object_id: i32) {
        let object_id = ObjectId(object_id);
        if !self
            .players
            .values()
            .any(|p| p.leftover.published.contains(&object_id))
        {
            return;
        }
        // `World.findObject(id)`: an item of any player, or one on the ground.
        let item = self
            .players
            .values()
            .find_map(|p| p.data.items.iter().find(|i| i.object_id == object_id))
            .cloned()
            .or_else(|| self.ground_item(object_id).map(|g| g.data.clone()));
        let Some(item) = item else {
            return;
        };
        let info = self.update_entry(&item, 0).item;
        self.send(id, &ExRpItemLink { item: info });
    }

    /// Java `Item.publish()` for every item of a chat text that the speaker owns
    /// (`Say2.parseAndPublishItem`).
    pub(crate) fn publish_items_of(&mut self, id: ObjectId, items: Vec<ObjectId>) {
        if let Some(player) = self.players.get_mut(&id) {
            player.leftover.published.extend(items);
        }
    }
}
