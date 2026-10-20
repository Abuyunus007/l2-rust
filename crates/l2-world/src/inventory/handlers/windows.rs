//! The `Maps` and `Calculator` handlers: a double click opens a client window.
// Java: dist/game/data/scripts/handlers/items/Maps.java
// Java: dist/game/data/scripts/handlers/items/Calculator.java
// Java: network/serverpackets/ShowMiniMap.java, ShowCalculator.java
//
// Both start with `if (!playable.isPlayer())` (a pet cannot carry the item). `Playable` is always
// a player here (summons are M6), so that branch is left out.

use l2_protocol_game::server::{ShowCalculator, ShowMiniMap};
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

/// Java `SevenSigns.getInstance().getCurrentPeriod()`.
// The Seven Signs are not modelled; 0 is `PERIOD_COMP_RECRUITING`, the period of a fresh
// database (the client only colours the seals of the map by it).
const SEVEN_SIGNS_PERIOD: u8 = 0;

impl World {
    /// Java `Maps.onItemUse`: `playable.sendPacket(new ShowMiniMap(item.getId()))`.
    pub(crate) fn maps_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        self.send(
            id,
            &ShowMiniMap {
                map_id: item.item_id,
                seven_signs_period: SEVEN_SIGNS_PERIOD,
            },
        );
        true
    }

    /// Java `Calculator.onItemUse`: `playable.sendPacket(new ShowCalculator(item.getId()))`.
    pub(crate) fn calculator_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        self.send(
            id,
            &ShowCalculator {
                calculator_id: item.item_id,
            },
        );
        true
    }
}
