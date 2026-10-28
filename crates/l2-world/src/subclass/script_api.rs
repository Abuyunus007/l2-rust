//! What the class change, class master and certification scripts (S-VM-01) call on a player:
//! Java's `setPlayerClass`, `setBaseClass`, `getRace`, `setNoble`, `store`, the village master
//! checks of an NPC and the player variables.
// Java: entity/actor/Player.java (setPlayerClass, setBaseClass, getRace, setNoble, store,
//       giveAvailableSkills, getVariables), entity/actor/instance/VillageMaster.java
//       (checkVillageMaster)

use l2_data::{ClassId, Race};
use l2_util::ObjectId;

use super::village_master::VillageMasterKind;
use crate::rewards::exp::SkillWrites;
use crate::world::World;

impl World {
    /// Java `player.getRace()`: the race of the class template, or of the base class while a
    /// subclass is active.
    pub fn player_race(&self, id: ObjectId) -> Option<Race> {
        let p = self.players.get(&id)?;
        let class = if p.data.profile.class_index > 0 {
            p.data.base_class
        } else {
            p.data.class_id
        };
        u16::try_from(class).ok().map(ClassId).and_then(ClassId::race)
    }

    /// Java `player.setPlayerClass(id)`: the class change with its effect, messages, hennas,
    /// party and clan icons and skills (`admin/edit_char.rs`).
    pub fn set_player_class(&mut self, id: ObjectId, class_id: i32) {
        crate::admin::edit_char::set_player_class(self, id, class_id);
    }

    /// Java `player.setBaseClass(id)`: `characters.base_class` with the next store.
    pub fn set_base_class(&mut self, id: ObjectId, class_id: i32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.data.base_class = class_id;
        }
    }

    /// Java `player.getSubClasses().get(player.getClassIndex()).setPlayerClass(classId)`: the
    /// holder of the active subclass takes the class id.
    pub fn set_active_sub_class_holder_class(&mut self, id: ObjectId, class_id: i32) {
        if let Some(p) = self.players.get_mut(&id) {
            let index = p.data.profile.class_index;
            if let Some(holder) = p.data.profile.subclasses.subs.get_mut(&index) {
                holder.class_id = class_id;
            }
        }
    }

    /// Java `player.setNoble(value)`: the noble skills, the flag and the skill list.
    pub fn set_noble(&mut self, id: ObjectId, value: bool) {
        crate::admin::edit_char::set_noble(self, id, value);
    }

    /// Java `player.store()` / `storeMe()`: the whole character is written with the next outbox
    /// turn.
    pub fn store_player(&mut self, id: ObjectId) {
        crate::admin::edit_char::store_me(self, id);
    }

    /// Java `player.giveAvailableSkills(includeByFs, includeAutoGet, includeRequiredItems)` with
    /// its `storeSkill` writes. Returns how many skills were new.
    pub fn give_available_skills_now(
        &mut self,
        id: ObjectId,
        include_by_fs: bool,
        include_auto_get: bool,
        include_required_items: bool,
    ) -> i32 {
        let mut writes = SkillWrites::default();
        let count = self.give_available_skills(
            id,
            include_by_fs,
            include_auto_get,
            include_required_items,
            &mut writes,
        );
        self.flush_skill_writes(id, writes);
        count
    }

    /// `Player.ini` `AutoLearnSkills`, `AutoLearnForgottenScrollSkills` and
    /// `AutoLearnSkillsWithoutItems` (Java `PlayerConfig.AUTO_LEARN_SKILLS`, ...).
    pub fn auto_learn_settings(&self) -> (bool, bool, bool) {
        (
            self.settings.exp.auto_learn_skills,
            self.settings.learn.auto_learn_fs_skills,
            self.settings.learn.auto_learn_skills_without_items,
        )
    }

    /// `Player.ini` `MaxSubclass` (Java `PlayerConfig.MAX_SUBCLASS`).
    pub fn max_subclass(&self) -> i32 {
        self.settings.subclass.max_subclass
    }

    /// The `VillageMaster*` class of an NPC (its `type`), `None` for an NPC that is no village
    /// master (Java `npc instanceof VillageMasterPriest`, ...).
    pub fn npc_village_master_kind(&self, npc: ObjectId) -> Option<VillageMasterKind> {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .and_then(|t| VillageMasterKind::from_npc_type(&t.npc_type))
    }

    /// Java `((VillageMaster) npc).checkVillageMaster(classId)`: whether this master teaches the
    /// class (race and type, or `AltSubclassEverywhere`). `None` for an NPC that is no village
    /// master (Java's cast fails).
    pub fn check_village_master(&self, npc: ObjectId, class_id: i32) -> Option<bool> {
        let kind = self.npc_village_master_kind(npc)?;
        let class = u16::try_from(class_id)
            .ok()
            .map(ClassId)
            .filter(|c| c.is_player_class());
        let rules = super::SubclassRules {
            kind,
            everywhere: self.settings.subclass.alt_game_subclass_everywhere,
            race: None,
            female: false,
            base_class: 0,
            active_class: 0,
            subs: &[],
        };
        Some(rules.check_village_master(class))
    }

    /// Java `ClassListData.getInstance().getClass(id).getClassName()`, empty for an unknown id.
    pub fn class_name(&self, class_id: i32) -> String {
        self.admin
            .class_list
            .get_by_id(class_id)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    /// Java `player.getVariables().getString(name, null)`.
    pub fn player_variable_text(&self, id: ObjectId, name: &str) -> Option<&str> {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get(name))
            .map(String::as_str)
    }
}
