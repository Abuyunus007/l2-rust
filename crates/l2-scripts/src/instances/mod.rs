//! `data/scripts/instances`: the instance zone scripts (Kamaloka, Pailaka, ...).
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the same
//! order (Java's load order is the folder listing).

//pub mod kamaloka;
//pub mod pailaka_devils_legacy;
//pub mod pailaka_injured_dragon;
pub mod pailaka_song_of_ice_and_fire;
//pub mod rim_kamaloka;

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::ScriptRegistry;

/// Java `Npc._scriptValue` (`getScriptValue`, `isScriptValue`, `setScriptValue`) of the NPCs one
/// script cares about. A script holds no state (D-10), so the value lives in its `ctx.state`.
#[derive(Debug, Default)]
pub(crate) struct ScriptValues(HashMap<ObjectId, i32>);

impl ScriptValues {
    /// Java `npc.getScriptValue()`: 0 until it is set.
    pub(crate) fn get(&self, npc: ObjectId) -> i32 {
        self.0.get(&npc).copied().unwrap_or(0)
    }

    /// Java `npc.isScriptValue(value)`.
    pub(crate) fn is(&self, npc: ObjectId, value: i32) -> bool {
        self.get(npc) == value
    }

    /// Java `npc.setScriptValue(value)`.
    pub(crate) fn set(&mut self, npc: ObjectId, value: i32) {
        self.0.insert(npc, value);
    }

    /// The NPC is gone (Java's object is garbage collected).
    pub(crate) fn forget(&mut self, npc: ObjectId) {
        self.0.remove(&npc);
    }
}

/// Registers every instance script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    //r.add(kamaloka::Kamaloka);
    //r.add(pailaka_devils_legacy::PailakaDevilsLegacy);
    //r.add(pailaka_injured_dragon::PailakaInjuredDragon);
    r.add(pailaka_song_of_ice_and_fire::PailakaSongOfIceAndFire);
    //r.add(rim_kamaloka::RimKamaloka);
}

/// Helpers of the instance script tests.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::Arc;

    use l2_data::InstanceTemplates;

    /// The instance names (`InstanceNames.xml`) and templates (`data/instances/*.xml`) of a test:
    /// `(id, name)` and `(id, the children of <instance>)`.
    pub(crate) fn instance_data(names: &[(i32, &str)], templates: &[(i32, &str)]) -> Arc<InstanceTemplates> {
        let mut data = InstanceTemplates::default();
        let list: String = names
            .iter()
            .map(|(id, name)| format!(r#"<instance id="{id}" name="{name}" />"#))
            .collect();
        data.add_names(&format!("<list>{list}</list>")).unwrap();
        for (id, body) in templates {
            data.add_template(&format!(r#"<instance id="{id}">{body}</instance>"#), &format!("T{id}.xml"));
        }
        Arc::new(data)
    }
}
