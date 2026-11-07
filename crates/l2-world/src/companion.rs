//! What the engine knows about a hired companion (ours, `custom/companions`): whose it is, how
//! strong, and how it looks. Everything else about a companion is a script's.
//!
//! A companion is an NPC of the class `Companion`. It is told to the clients as a player
//! (`CharInfo`, the way fake players are), with a figure made for it: race, sex, hair and the
//! armor and weapon it wears. All of that, its owner and its strength are fixed *before* the
//! NPC is shown to anybody: a client that has seen a model cannot be told it is a player.

use l2_data::FakePlayerHolder;
use l2_protocol_game::server::{
    PartyDistributionType, PartyMemberVitals, PartySmallWindowAll, PartySmallWindowDeleteAll,
    PartySmallWindowUpdate, PartyWindowMember,
};
use l2_util::ObjectId;

use crate::objects::NpcScale;
use crate::world::World;

/// How a companion looks to the clients. The slots are item ids, 0 for nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionLook {
    pub name: String,
    pub title: String,
    /// A player race: 0 human, 1 elf, 2 dark elf, 3 orc, 4 dwarf, 5 kamael.
    pub race: i32,
    pub female: bool,
    /// The class shown in the party window and the target window.
    pub class_id: i32,
    pub hair: i32,
    pub hair_color: i32,
    pub face: i32,
    pub head: i32,
    pub right_hand: i32,
    pub left_hand: i32,
    pub gloves: i32,
    pub chest: i32,
    pub legs: i32,
    pub feet: i32,
}

impl CompanionLook {
    /// The same figure as the look a fake player template gives, which is what the packet is
    /// built from. Nothing a template could add (a store, a clan, a title colour) is set.
    pub(crate) fn holder(&self) -> FakePlayerHolder {
        FakePlayerHolder {
            class_id: self.class_id,
            hair: self.hair,
            hair_color: self.hair_color,
            face: self.face,
            name_color: 0xFFFFFF,
            title_color: 0xECF9A2,
            equip_head: self.head,
            equip_r_hand: self.right_hand,
            equip_l_hand: self.left_hand,
            equip_gloves: self.gloves,
            equip_chest: self.chest,
            equip_legs: self.legs,
            equip_feet: self.feet,
            equip_cloak: 0,
            equip_hair: 0,
            equip_hair2: 0,
            agathion_id: 0,
            weapon_enchant_level: 0,
            armor_enchant_level: 0,
            fishing: false,
            bait_location_x: 0,
            bait_location_y: 0,
            bait_location_z: 0,
            recommends: 0,
            noble_level: 0,
            hero: false,
            clan_id: 0,
            pledge_status: 0,
            sitting: false,
            private_store_type: 0,
            private_store_message: String::new(),
            fake_player_talkable: false,
        }
    }
}

/// What a companion is born with.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionSetup {
    /// The player it works for: its kills pay him.
    pub owner: ObjectId,
    pub scale: NpcScale,
    pub look: CompanionLook,
}

impl World {
    /// A companion of template `npc_id` next to a player (`Quest.addSpawn`): made for its owner,
    /// at the strength and with the figure of `setup`, from its first moment in the world.
    /// `None` when the NPC cannot be made.
    pub fn spawn_companion(
        &mut self,
        npc_id: i32,
        position: (i32, i32, i32),
        heading: i32,
        instance_id: i32,
        setup: CompanionSetup,
    ) -> Option<ObjectId> {
        self.npc_world.pending_companion = Some(setup);
        let id = self.script_add_spawn(npc_id, position, heading, false, 0, instance_id);
        // Whatever the spawn did not take (it failed before the NPC was made) must not go to the
        // next NPC.
        self.npc_world.pending_companion = None;
        id
    }

    /// The player a companion works for, while he is in the world.
    pub fn companion_owner(&self, id: ObjectId) -> Option<ObjectId> {
        self.npcs
            .get(&id)
            .and_then(|n| n.owner)
            .filter(|owner| self.players.contains_key(owner))
    }

    /// Java `asPlayer()` for what a kill earns: the player itself, a summon's owner, and the owner
    /// of a companion. Experience, SP, drops and the quests' kill credit go to this player.
    pub(crate) fn kill_credit_player(&self, id: ObjectId) -> Option<ObjectId> {
        self.acting_player(id).or_else(|| self.companion_owner(id))
    }

    /// Does `id` look like a player (a companion with a figure)?
    pub(crate) fn has_companion_look(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.look.is_some())
    }

    // ------------------------------------------------------------------ party window

    /// What the party window writes for a companion: the numbers of an NPC under a player's name
    /// and class. No CP.
    fn companion_vitals(&self, id: ObjectId) -> Option<PartyMemberVitals> {
        let npc = self.npcs.get(&id)?;
        let look = npc.look.as_ref()?;
        let view = self.stats(id)?;
        Some(PartyMemberVitals {
            object_id: id.0,
            name: look.name.clone(),
            cp: 0,
            max_cp: 0,
            hp: npc.core.cur_hp as i32,
            max_hp: view.max_hp(),
            mp: npc.core.cur_mp as i32,
            max_mp: view.max_mp(),
            level: view.level(),
            class_id: look.class_id,
        })
    }

    /// The party window of `owner`, with `companions` in it: his real party (if any) and the
    /// companions after it. Only he gets it: the others of his party see no companion in theirs.
    ///
    /// A companion is not a member of the party (a party is players only, and every rule of the
    /// party would have to learn about NPCs): the window is the whole of it. The client takes a
    /// player without a party to be the leader of the one that is shown.
    ///
    /// The window is emptied first: a client builds it from the first `PartySmallWindowAll` and
    /// takes no new members from a second one, so the next companion hired would not show.
    pub fn send_companion_window(&self, owner: ObjectId, companions: &[ObjectId]) {
        let mut window = match self
            .party_of(owner)
            .and_then(|party| self.party_small_window_all(owner, party))
        {
            Some(window) => window,
            None => PartySmallWindowAll {
                leader_object_id: owner.0,
                distribution_type: PartyDistributionType::FindersKeepers,
                members: Vec::new(),
            },
        };
        window.members.extend(companions.iter().filter_map(|id| {
            Some(PartyWindowMember {
                vitals: self.companion_vitals(*id)?,
                race: self.npcs.get(id)?.look.as_ref()?.race,
                summon: None,
            })
        }));
        self.send(owner, &PartySmallWindowDeleteAll);
        self.send(owner, &window);
    }

    /// The party window of `owner` without any companion: his party's, or none.
    pub fn clear_companion_window(&self, owner: ObjectId) {
        self.send(owner, &PartySmallWindowDeleteAll);
        if let Some(window) = self
            .party_of(owner)
            .and_then(|party| self.party_small_window_all(owner, party))
        {
            self.send(owner, &window);
        }
    }

    /// The HP and MP of a companion in the party window of `owner`.
    pub fn update_companion_window(&self, owner: ObjectId, companion: ObjectId) {
        if let Some(member) = self.companion_vitals(companion) {
            self.send(owner, &PartySmallWindowUpdate { member });
        }
    }
}
