//! The Oracle of Dusk and Dawn teleporters, the temple priests, the dimensional rift posters and
//! the ziggurat teleporters.
// Java: dist/game/data/scripts/ai/others/OracleTeleport/OracleTeleport.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const TOWN_DAWN: [i32; 11] = [
    31078, 31079, 31080, 31081, 31083, 31084, 31082, 31692, 31694, 31997, 31168,
];

const TOWN_DUSK: [i32; 11] = [
    31085, 31086, 31087, 31088, 31090, 31091, 31089, 31693, 31695, 31998, 31169,
];

const TEMPLE_PRIEST: [i32; 10] = [
    31127, 31128, 31129, 31130, 31131, 31137, 31138, 31139, 31140, 31141,
];

const RIFT_POSTERS: [i32; 6] = [31488, 31489, 31490, 31491, 31492, 31493];

const TELEPORTERS: [i32; 64] = [
    31078, 31079, 31080, 31081, 31082, 31083, 31084, 31692, 31694, 31997, 31168, 31085, 31086, 31087, 31088,
    31089, 31090, 31091, 31693, 31695, 31998, 31169, 31494, 31495, 31496, 31497, 31498, 31499, 31500, 31501,
    31502, 31503, 31504, 31505, 31506, 31507, 31095, 31096, 31097, 31098, 31099, 31100, 31101, 31102, 31103,
    31104, 31105, 31106, 31107, 31108, 31109, 31110, 31114, 31115, 31116, 31117, 31118, 31119, 31120, 31121,
    31122, 31123, 31124, 31125,
];

const RETURN_LOCS: [(i32, i32, i32); 64] = [
    (-80555, 150337, -3040),
    (-13953, 121404, -2984),
    (16354, 142820, -2696),
    (83369, 149253, -3400),
    (111386, 220858, -3544),
    (83106, 53965, -1488),
    (146983, 26595, -2200),
    (148256, -55454, -2779),
    (45664, -50318, -800),
    (86795, -143078, -1341),
    (115136, 74717, -2608),
    (-82368, 151568, -3120),
    (-14748, 123995, -3112),
    (18482, 144576, -3056),
    (81623, 148556, -3464),
    (112486, 220123, -3592),
    (82819, 54607, -1520),
    (147570, 28877, -2264),
    (149888, -56574, -2979),
    (44528, -48370, -800),
    (85129, -142103, -1542),
    (116642, 77510, -2688),
    (-41572, 209731, -5087),
    (-52872, -250283, -7908),
    (45256, 123906, -5411),
    (46192, 170290, -4981),
    (111273, 174015, -5437),
    (-20604, -250789, -8165),
    (-21726, 77385, -5171),
    (140405, 79679, -5427),
    (-52366, 79097, -4741),
    (118311, 132797, -4829),
    (172185, -17602, -4901),
    (83000, 209213, -5439),
    (-19500, 13508, -4901),
    (12525, -248496, -9580),
    (-41561, 209225, -5087),
    (45242, 124466, -5413),
    (110711, 174010, -5439),
    (-22341, 77375, -5173),
    (-52889, 79098, -4741),
    (117760, 132794, -4831),
    (171792, -17609, -4901),
    (82564, 209207, -5439),
    (-41565, 210048, -5085),
    (45278, 123608, -5411),
    (111510, 174013, -5437),
    (-21489, 77372, -5171),
    (-52016, 79103, -4739),
    (118557, 132804, -4829),
    (172570, -17605, -4899),
    (83347, 209215, -5437),
    (42495, 143944, -5381),
    (45666, 170300, -4981),
    (77138, 78389, -5125),
    (139903, 79674, -5429),
    (-20021, 13499, -4901),
    (113418, 84535, -6541),
    (-52940, -250272, -7907),
    (46499, 170301, -4979),
    (-20280, -250785, -8163),
    (140673, 79680, -5437),
    (-19182, 13503, -4899),
    (12837, -248483, -9579),
];

// Item
const DIMENSIONAL_FRAGMENT: i32 = 7079;

/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `RETURN_LOCS[index]`: the index outside the table throws in Java.
fn return_location(index: i32) -> Option<(i32, i32, i32)> {
    usize::try_from(index)
        .ok()
        .and_then(|i| RETURN_LOCS.get(i))
        .copied()
}

/// Java's `for (int id1 : TELEPORTERS) { if (id1 == npcId) break; i++; }`: the index of the NPC,
/// or the table size when it is not in it.
fn teleporter_index(npc_id: i32) -> usize {
    TELEPORTERS
        .iter()
        .position(|&t| t == npc_id)
        .unwrap_or(TELEPORTERS.len())
}

/// The adena a ziggurat teleport costs by level (Java's `if` chain in `onEvent`).
fn ziggurat_price(level: i32) -> Option<i64> {
    match level {
        20..=29 => Some(2000),
        30..=39 => Some(4500),
        40..=49 => Some(8000),
        50..=59 => Some(12500),
        60..=69 => Some(18000),
        70.. => Some(24500),
        _ => None,
    }
}

/// Java `ai.others.OracleTeleport.OracleTeleport`.
pub struct OracleTeleport;

impl Script for OracleTeleport {
    fn name(&self) -> &'static str {
        "OracleTeleport"
    }

    fn path(&self) -> &'static str {
        "ai/others/OracleTeleport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&RIFT_POSTERS);
        s.add_start_npc(&TELEPORTERS);
        s.add_start_npc(&TEMPLE_PRIEST);
        s.add_start_npc(&TOWN_DAWN);
        s.add_start_npc(&TOWN_DUSK);
        s.add_talk_id(&RIFT_POSTERS);
        s.add_talk_id(&TELEPORTERS);
        s.add_talk_id(&TEMPLE_PRIEST);
        s.add_talk_id(&TOWN_DAWN);
        s.add_talk_id(&TOWN_DUSK);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);
        let mut htmltext = String::new();
        let qs = ctx.get_quest_state(player, true)?;
        let npc_id = npc.id;
        if event.eq_ignore_ascii_case("Return") {
            if TEMPLE_PRIEST.contains(&npc_id) && qs.state(ctx) == STARTED {
                // NOTE(java-quirk): `RETURN_LOCS[qs.getInt("id")]` throws outside the table.
                let (x, y, z) = return_location(qs.get_int(ctx, "id"))?;
                ctx.teleport_player(player, x, y, z, 0, false);
                ctx.world_mut().set_in_7s_dungeon(player, false);
                qs.exit_quest(ctx, true, false);
            } else if RIFT_POSTERS.contains(&npc_id) && qs.state(ctx) == STARTED {
                let (x, y, z) = return_location(qs.get_int(ctx, "id"))?;
                ctx.teleport_player(player, x, y, z, 0, false);
                htmltext = "rift_back.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else {
                if let Some((x, y, z)) = ctx.get_random_entry(&RETURN_LOCS) {
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
                htmltext = "rift_back_unknown.htm".to_owned();
                ctx.world_mut().set_in_7s_dungeon(player, false);
                qs.exit_quest(ctx, true, false);
            }
        } else if event.eq_ignore_ascii_case("Festival") {
            let id = qs.get_int(ctx, "id");
            // NOTE(java-quirk): `id` is an index into `TELEPORTERS`, but it is looked up among the
            // NPC ids of `TOWN_DAWN` / `TOWN_DUSK`, so neither matches and the page is shown.
            if TOWN_DAWN.contains(&id) {
                ctx.teleport_player(player, -80157, 111344, -4901, 0, false);
                ctx.world_mut().set_in_7s_dungeon(player, true);
            } else if TOWN_DUSK.contains(&id) {
                ctx.teleport_player(player, -81261, 86531, -5157, 0, false);
                ctx.world_mut().set_in_7s_dungeon(player, true);
            } else {
                htmltext = "oracle1.htm".to_owned();
            }
        } else if event.eq_ignore_ascii_case("Dimensional") {
            htmltext = "oracle.htm".to_owned();
            ctx.teleport_player(player, -114755, -179466, -6752, 0, false);
        } else if event.eq_ignore_ascii_case("5.htm") {
            let id = qs.get_int(ctx, "id");
            if id > -1 {
                htmltext = "5a.htm".to_owned();
            }

            let i = teleporter_index(npc_id);
            qs.set(ctx, "id", &i.to_string());
            qs.set_state(ctx, STARTED);
            ctx.teleport_player(player, -114755, -179466, -6752, 0, false);
        } else if event.eq_ignore_ascii_case("6.htm") {
            htmltext = "6.htm".to_owned();
            qs.exit_quest(ctx, true, false);
        } else if event.eq_ignore_ascii_case("zigurratDimensional") {
            let player_level = ctx.player_level(player);
            if let Some(price) = ziggurat_price(player_level) {
                ctx.take_items(player, ADENA_ID, price);
            }

            let i = teleporter_index(npc_id);
            qs.set(ctx, "id", &i.to_string());
            qs.set_state(ctx, STARTED);
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            htmltext = "ziggurat_rift.htm".to_owned();
            ctx.teleport_player(player, -114755, -179466, -6752, 0, false);
        }

        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = String::new();
        let qs = ctx.get_quest_state(player, true)?;
        let npc_id = npc.id;
        if TOWN_DAWN.contains(&npc_id) {
            qs.set_state(ctx, STARTED);
            let i = teleporter_index(npc_id);
            qs.set(ctx, "id", &i.to_string());
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            ctx.teleport_player(player, -80157, 111344, -4901, 0, false);
            ctx.world_mut().set_in_7s_dungeon(player, true);
        }

        if TOWN_DUSK.contains(&npc_id) {
            qs.set_state(ctx, STARTED);
            let i = teleporter_index(npc_id);
            qs.set(ctx, "id", &i.to_string());
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            ctx.teleport_player(player, -81261, 86531, -5157, 0, false);
            ctx.world_mut().set_in_7s_dungeon(player, true);
        } else if (31494..=31507).contains(&npc_id) {
            if ctx.player_level(player) < 20 {
                htmltext = "1.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else if ctx.world().active_quest_count(player) > 23 {
                htmltext = "1a.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else if !ctx.has_quest_items(player, DIMENSIONAL_FRAGMENT) {
                htmltext = "3.htm".to_owned();
            } else {
                qs.set_state(ctx, CREATED);
                htmltext = "4.htm".to_owned();
            }
        } else if (31095..=31111).contains(&npc_id) || (31114..=31126).contains(&npc_id) {
            let player_level = ctx.player_level(player);
            if player_level < 20 {
                htmltext = "ziggurat_lowlevel.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else if ctx.world().active_quest_count(player) > 40 {
                ctx.send_system_message(player, SystemMessageId::YouHaveTooManyOngoingQuests);
                qs.exit_quest(ctx, true, false);
            } else if !ctx.has_quest_items(player, DIMENSIONAL_FRAGMENT) {
                htmltext = "ziggurat_nofrag.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else if ziggurat_price(player_level).is_some_and(|price| ctx.adena_of(player) < price) {
                htmltext = "ziggurat_noadena.htm".to_owned();
                qs.exit_quest(ctx, true, false);
            } else {
                htmltext = "ziggurat.htm".to_owned();
            }
        }

        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="31078" type="Folk" level="70" name="Dawn Priest"/>
        <npc id="31095" type="Folk" level="70" name="Ziggurat"/>
        <npc id="31494" type="Folk" level="70" name="Poster"/>
        <npc id="31127" type="Folk" level="70" name="Temple Priest"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7079" type="EtcItem" name="Dimensional Fragment"><set name="is_stackable" val="true"/></item>"#;

    fn setup(level: i32, items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(OracleTeleport);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        for id in [31078, 31095, 31494, 31127] {
            t.spawn(id, 20);
        }
        t.enter(1, level, items);
        t.take_writes();
        t
    }

    fn npc(t: &ScriptTest, id: i32) -> ScriptNpc {
        let object = t.npcs_of(id)[0];
        ScriptNpc {
            object_id: object,
            id,
        }
    }

    fn talk(t: &mut ScriptTest, id: i32) -> Option<String> {
        let n = npc(t, id);
        let mut out = None;
        t.world.run_as_script("OracleTeleport", |ctx| {
            out = OracleTeleport.on_talk(ctx, n, ObjectId(1))
        });
        out
    }

    fn event(t: &mut ScriptTest, id: i32, name: &str) -> Option<String> {
        let n = npc(t, id);
        let mut out = None;
        t.world.run_as_script("OracleTeleport", |ctx| {
            out = OracleTeleport.on_event(ctx, name, Some(n), Some(ObjectId(1)));
        });
        out
    }

    fn at(t: &ScriptTest) -> (i32, i32) {
        let p = t.world.position_of(ObjectId(1)).unwrap();
        (p.0, p.1)
    }

    #[test]
    fn the_tables_are_javas() {
        assert_eq!(TELEPORTERS.len(), 64);
        assert_eq!(teleporter_index(31078), 0);
        assert_eq!(teleporter_index(31082), 4);
        assert_eq!(teleporter_index(1), 64);
        assert_eq!(return_location(0), Some((-80555, 150337, -3040)));
        assert_eq!(return_location(64), None);
        assert_eq!(ziggurat_price(19), None);
        assert_eq!(ziggurat_price(75), Some(24500));
    }

    #[test]
    fn a_dawn_priest_sends_the_player_into_the_festival_and_remembers_the_town() {
        let mut t = setup(60, vec![]);
        assert_eq!(talk(&mut t, 31078).as_deref(), Some(""));
        assert_eq!(at(&t), (-80157, 111344));
        assert!(t.world.player(ObjectId(1)).unwrap().data.profile.in_7s_dungeon);
        let writes = t.take_writes();
        assert_eq!(writes.len(), 2, "{writes:?}");
    }

    #[test]
    fn the_rift_poster_checks_level_fragment_and_quests() {
        let mut t = setup(19, vec![]);
        assert_eq!(talk(&mut t, 31494).as_deref(), Some("1.htm"));
        let mut t = setup(30, vec![]);
        assert_eq!(talk(&mut t, 31494).as_deref(), Some("3.htm"));
        let mut t = setup(30, vec![item(5000, 7079, 1)]);
        assert_eq!(talk(&mut t, 31494).as_deref(), Some("4.htm"));
    }

    #[test]
    fn the_ziggurat_checks_the_adena_and_takes_it_for_the_dimensional_teleport() {
        let mut t = setup(25, vec![item(5000, 7079, 1), item(5001, 57, 1999)]);
        assert_eq!(talk(&mut t, 31095).as_deref(), Some("ziggurat_noadena.htm"));
        let mut t = setup(25, vec![item(5000, 7079, 1), item(5001, 57, 5000)]);
        assert_eq!(talk(&mut t, 31095).as_deref(), Some("ziggurat.htm"));
        assert_eq!(
            event(&mut t, 31095, "zigurratDimensional").as_deref(),
            Some("ziggurat_rift.htm")
        );
        assert_eq!(t.count(1, 57), 3000);
        assert_eq!(at(&t), (-114755, -179466));
    }

    #[test]
    fn return_sends_the_player_back_from_the_dimension() {
        let mut t = setup(25, vec![]);
        // A priest whose player has no state: the unknown-origin branch.
        assert_eq!(
            event(&mut t, 31127, "Return").as_deref(),
            Some("rift_back_unknown.htm")
        );
        assert!(RETURN_LOCS.iter().any(|&(x, y, _)| (x, y) == at(&t)));
        assert_eq!(event(&mut t, 31078, "Festival").as_deref(), Some("oracle1.htm"));
        assert_eq!(event(&mut t, 31078, "Dimensional").as_deref(), Some("oracle.htm"));
        assert_eq!(event(&mut t, 31078, "6.htm").as_deref(), Some("6.htm"));
    }
}
