//! The castle observation towers: the broadcasting tower NPC's pages and the `observe`,
//! `observesiege` and `observeoracle` bypasses.
// Java: entity/actor/instance/BroadcastingTower.java (showChatWindow)
// Java: dist/game/data/scripts/handlers/bypass/npc/Observation.java
// Java: entity/actor/Player.java (enterObserverMode, `observer.rs`)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::bypass::BypassHandlers;
use crate::target::distance_2d;
use crate::world::World;

/// `Observation.LOCATIONS`: where the client looks from, and the Adena it costs.
const LOCATIONS: [(i32, i32, i32, i64); 31] = [
    // Gludio
    (-18347, 114000, -2360, 500),
    (-18347, 113255, -2447, 500),
    // Dion
    (22321, 155785, -2604, 500),
    (22321, 156492, -2627, 500),
    // Giran
    (112000, 144864, -2445, 500),
    (112657, 144864, -2525, 500),
    // Innadril
    (116260, 244600, -775, 500),
    (116260, 245264, -721, 500),
    // Oren
    (78100, 36950, -2242, 500),
    (78744, 36950, -2244, 500),
    // Aden
    (147457, 9601, -233, 500),
    (147457, 8720, -252, 500),
    // Goddard
    (147542, -43543, -1328, 500),
    (147465, -45259, -1328, 500),
    // Rune
    (20598, -49113, -300, 500),
    (18702, -49150, -600, 500),
    // Schuttgart
    (77541, -147447, 353, 500),
    (77541, -149245, 353, 500),
    // Coliseum
    (148416, 46724, -3000, 80),
    (149500, 46724, -3000, 80),
    (150511, 46724, -3000, 80),
    // Dusk
    (-77200, 88500, -4800, 500),
    (-75320, 87135, -4800, 500),
    (-76840, 85770, -4800, 500),
    (-76840, 85770, -4800, 500),
    (-79950, 85165, -4800, 500),
    // Dawn
    (-79185, 112725, -4300, 500),
    (-76175, 113330, -4300, 500),
    (-74305, 111965, -4300, 500),
    (-75915, 110600, -4300, 500),
    (-78930, 110005, -4300, 500),
];

/// The Oracle's towers (`BroadcastingTower.showChatWindow`'s four `isInsideRadius2D` points).
const ORACLE_TOWERS: [(i32, i32); 4] = [
    (-79884, 86529),
    (-78858, 111358),
    (-76973, 87136),
    (-75850, 111968),
];

/// Registers `Observation`'s commands.
pub(super) fn register_bypass_handlers(handlers: &mut BypassHandlers) {
    for command in ["observesiege", "observeoracle", "observe"] {
        handlers.register(command, observation_handler);
    }
}

/// Java `Observation.onCommand`.
fn observation_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    if !target.is_some_and(|t| world.is_broadcasting_tower(t)) {
        return false;
    }
    if world.summon_of(player).is_some() {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouMayNotObserveASiegeWithAPetOrServitorSummoned),
        );
        return false;
    }
    // `isRegisteredOnEvent()`: the events are scripts (D-05).
    let mut words = command.split(' ');
    let kind = words.next().unwrap_or("").to_lowercase();
    // `Integer.parseInt(command.split(" ")[1])`: a missing word throws as well.
    let Some(param) = words.next().and_then(|w| w.parse::<i32>().ok()) else {
        warn!("Exception in Observation: [{command}] from {player}");
        return false;
    };
    let Some(&(x, y, z, cost)) = usize::try_from(param).ok().and_then(|i| LOCATIONS.get(i)) else {
        return false;
    };
    match kind.as_str() {
        "observesiege" => {
            if world.siege_at(x, y, z).is_some() {
                world.observe_for_adena(player, (x, y, z), cost);
            } else {
                world.send(
                    player,
                    &SystemMessage::new(SystemMessageId::ObservationIsOnlyPossibleDuringASiege),
                );
            }
            true
        }
        // Oracle Dusk/Dawn, Observe
        "observeoracle" | "observe" => {
            world.observe_for_adena(player, (x, y, z), cost);
            true
        }
        _ => false,
    }
}

impl World {
    /// Whether the NPC is a `BroadcastingTower`.
    fn is_broadcasting_tower(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.npc_type == "BroadcastingTower")
    }

    /// `Observation.doObserve`: the fee, then the observer mode and the item list.
    fn observe_for_adena(&mut self, player: ObjectId, loc: (i32, i32, i32), cost: i64) {
        if self.reduce_adena(player, cost, true) {
            self.enter_observer_mode(player, loc);
            self.send_item_list(player, false);
        }
        self.send(player, &ActionFailed);
    }

    /// Java `BroadcastingTower.showChatWindow(player, value)`: `true` for a broadcasting tower.
    pub(crate) fn broadcasting_tower_show_chat_window(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        value: i32,
    ) -> bool {
        if !self.is_broadcasting_tower(npc) {
            return false;
        }
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return false;
        };
        let at_oracle = self.position_of(npc).is_some_and(|(x, y, z)| {
            ORACLE_TOWERS
                .iter()
                .any(|&(ox, oy)| distance_2d((x, y, z), (ox, oy, z)) < 50.0)
        });
        let suffix = if at_oracle { "-Oracle" } else { "" };
        let filename = if value == 0 {
            format!("data/html/observation/{npc_id}{suffix}.htm")
        } else {
            format!("data/html/observation/{npc_id}{suffix}-{value}.htm")
        };
        if let Some(mut html) = self.html_from_file(player, npc.0, &filename) {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
        true
    }
}
