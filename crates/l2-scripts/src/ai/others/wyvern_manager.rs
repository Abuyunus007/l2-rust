//! Wyvern Manager: the castle, fortress and hall lords change their Strider into a Wyvern.
// Java: dist/game/data/scripts/ai/others/WyvernManager/WyvernManager.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ManagerType {
    Castle,
    ClanHall,
    Fort,
}

// Misc
const CRYSTAL_B_GRADE: i32 = 1460;
const WYVERN: i32 = 12621;
const WYVERN_FEE: i64 = 25;
const STRIDER_LEVEL: i32 = 55;
const STRIDERS: [i32; 8] = [12526, 12527, 12528, 16038, 16039, 16040, 16068, 13197];

// Seven Signs
const CABAL_DUSK: i32 = 1;
const SEAL_STRIFE: i32 = 3;

// NPCS
const MANAGERS: &[(i32, ManagerType)] = &[
    (35101, ManagerType::Castle),
    (35143, ManagerType::Castle),
    (35185, ManagerType::Castle),
    (35227, ManagerType::Castle),
    (35275, ManagerType::Castle),
    (35317, ManagerType::Castle),
    (35364, ManagerType::Castle),
    (35510, ManagerType::Castle),
    (35536, ManagerType::Castle),
    (35556, ManagerType::Castle),
    (35419, ManagerType::ClanHall),
    (35638, ManagerType::ClanHall),
    (36457, ManagerType::Fort),
    (36458, ManagerType::Fort),
    (36459, ManagerType::Fort),
    (36460, ManagerType::Fort),
    (36461, ManagerType::Fort),
    (36462, ManagerType::Fort),
    (36463, ManagerType::Fort),
    (36464, ManagerType::Fort),
    (36465, ManagerType::Fort),
    (36466, ManagerType::Fort),
    (36467, ManagerType::Fort),
    (36468, ManagerType::Fort),
    (36469, ManagerType::Fort),
    (36470, ManagerType::Fort),
    (36471, ManagerType::Fort),
    (36472, ManagerType::Fort),
    (36473, ManagerType::Fort),
    (36474, ManagerType::Fort),
    (36475, ManagerType::Fort),
    (36476, ManagerType::Fort),
    (36477, ManagerType::Fort),
];

fn manager_type(npc: ScriptNpc) -> Option<ManagerType> {
    MANAGERS.iter().find(|(id, _)| *id == npc.id).map(|(_, t)| *t)
}

/// Java `isOwnerClan(npc, player)`.
fn is_owner_clan(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> bool {
    if !ctx.is_clan_leader(player) {
        return false;
    }
    let facts = match manager_type(npc) {
        Some(ManagerType::Castle) => ctx.world().script_npc_castle_facts(npc.object_id),
        Some(ManagerType::ClanHall) => ctx.world().script_npc_hall_facts(npc.object_id),
        Some(ManagerType::Fort) => ctx.world().script_npc_fort_facts(npc.object_id),
        None => None,
    };
    facts.is_some_and(|f| ctx.clan_id_of(player) == f.owner_id)
}

/// Java `isInSiege(npc)`.
fn is_in_siege(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    match manager_type(npc) {
        Some(ManagerType::Castle) => ctx
            .world()
            .script_npc_castle_facts(npc.object_id)
            .is_some_and(|c| c.siege_active),
        Some(ManagerType::ClanHall) => match ctx.world().script_npc_hall_facts(npc.object_id) {
            Some(hall) => hall.siege_active,
            None => ctx
                .world()
                .script_npc_castle_facts(npc.object_id)
                .is_some_and(|c| ctx.world().script_castle_siege_in_progress(c.id)),
        },
        Some(ManagerType::Fort) => ctx
            .world()
            .script_npc_fort_facts(npc.object_id)
            .is_some_and(|f| f.siege_active),
        None => false,
    }
}

/// Java `getResidenceName(npc)`.
fn residence_name(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> String {
    let facts = match manager_type(npc) {
        Some(ManagerType::Castle) => ctx.world().script_npc_castle_facts(npc.object_id),
        Some(ManagerType::ClanHall) => ctx.world().script_npc_hall_facts(npc.object_id),
        Some(ManagerType::Fort) => ctx.world().script_npc_fort_facts(npc.object_id),
        None => None,
    };
    facts.map(|f| f.name).unwrap_or_default()
}

/// Java `replacePart(player, htmlFile)`.
fn replace_part(ctx: &ScriptCtx<'_>, player: ObjectId, html_file: &str) -> String {
    ctx.get_htm(player, html_file)
        .unwrap_or_default()
        .replace("%wyvern_fee%", &WYVERN_FEE.to_string())
        .replace("%strider_level%", &STRIDER_LEVEL.to_string())
}

/// Java `replaceAll(npc, player)`.
fn replace_all(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> String {
    replace_part(ctx, player, "wyvernmanager-01.html").replace("%residence_name%", &residence_name(ctx, npc))
}

/// `type == CASTLE && SevenSigns.isSealValidationPeriod() && sealOwner(STRIFE) == DUSK`.
fn dusk_blocks(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    manager_type(npc) == Some(ManagerType::Castle)
        && ctx.ss_is_seal_validation_period()
        && ctx.ss_seal_owner(SEAL_STRIFE) == CABAL_DUSK
}

/// The page of `onFirstTalk` and of the `Return` event.
fn main_page(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> String {
    if !is_owner_clan(ctx, npc, player) {
        "wyvernmanager-02.html".to_owned()
    } else if ctx.script_settings().allow_wyvern_always {
        replace_all(ctx, npc, player)
    } else if dusk_blocks(ctx, npc) {
        "wyvernmanager-dusk.html".to_owned()
    } else {
        replace_all(ctx, npc, player)
    }
}

/// Java `mountWyvern(npc, player)`.
fn mount_wyvern(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> String {
    if ctx.world().script_is_mounted_on(player, STRIDER_LEVEL, &STRIDERS) {
        if is_owner_clan(ctx, npc, player) && ctx.get_quest_items_count(player, CRYSTAL_B_GRADE) >= WYVERN_FEE
        {
            ctx.take_items(player, CRYSTAL_B_GRADE, WYVERN_FEE);
            ctx.world_mut().script_dismount_and_mount(player, WYVERN);
            return "wyvernmanager-04.html".to_owned();
        }
        return replace_part(ctx, player, "wyvernmanager-06.html");
    }

    if !ctx.world().script_is_on_strider(player) {
        ctx.send_system_message(
            player,
            SystemMessageId::YouMayOnlyRideAWyvernWhileYouReRidingAStrider,
        );
    }
    replace_part(ctx, player, "wyvernmanager-05.html")
}

/// Java `ai.others.WyvernManager`.
pub struct WyvernManager;

impl Script for WyvernManager {
    fn name(&self) -> &'static str {
        "WyvernManager"
    }

    fn path(&self) -> &'static str {
        "ai/others/WyvernManager"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = MANAGERS.iter().map(|(id, _)| *id).collect();
        s.add_start_npc(&ids);
        s.add_talk_id(&ids);
        s.add_first_talk_id(&ids);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);
        match event {
            "Return" => Some(main_page(ctx, npc, player)),
            "Help" => Some(if manager_type(npc) == Some(ManagerType::Castle) {
                replace_part(ctx, player, "wyvernmanager-03.html")
            } else {
                replace_part(ctx, player, "wyvernmanager-03b.html")
            }),
            "RideWyvern" => {
                if ctx.script_settings().allow_wyvern_always {
                    return Some(mount_wyvern(ctx, npc, player));
                }
                // If in siege and the config blocks wyvern riding, allow only the castle lord.
                if !ctx.script_settings().allow_wyvern_during_siege
                    && ctx.is_clan_leader(player)
                    && (ctx.world().is_in_siege(player) || is_in_siege(ctx, npc))
                {
                    // `CastleManager.getCastle(npc)` is the NPC's castle here.
                    let is_castle_lord = ctx
                        .world()
                        .script_npc_castle_facts(npc.object_id)
                        .is_some_and(|c| ctx.clan_id_of(player) == c.owner_id);
                    if !is_castle_lord {
                        ctx.send_message(player, "You cannot summon a wyvern during a siege.");
                        return None;
                    }
                }
                if dusk_blocks(ctx, npc) {
                    Some("wyvernmanager-dusk.html".to_owned())
                } else {
                    Some(mount_wyvern(ctx, npc, player))
                }
            }
            _ => None,
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(main_page(ctx, npc, player))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="35101" type="Folk" level="70" name="Wyvern Manager"/>"#;
    const ITEMS: &str = r#"<item id="1460" type="EtcItem" name="Crystal: B-Grade"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(WyvernManager);
        let pages = Pages::of(&WyvernManager).plain(&[
            "wyvernmanager-01.html",
            "wyvernmanager-02.html",
            "wyvernmanager-03.html",
            "wyvernmanager-03b.html",
            "wyvernmanager-05.html",
            "wyvernmanager-06.html",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let manager = t.spawn(35101, 20);
        t.enter(1, 60, vec![]);
        t.take();
        (
            t,
            ScriptNpc {
                object_id: manager,
                id: 35101,
            },
        )
    }

    fn event(t: &mut ScriptTest, npc: ScriptNpc, event: &str) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("WyvernManager", |ctx| {
            result = WyvernManager.on_event(ctx, event, Some(npc), Some(ObjectId(1)));
        });
        result
    }

    #[test]
    fn a_player_who_is_no_lord_gets_the_refusal_page() {
        let (mut t, npc) = setup();
        let mut page = None;
        t.world.run_as_script("WyvernManager", |ctx| {
            page = WyvernManager.on_first_talk(ctx, npc, ObjectId(1));
        });
        assert_eq!(page.as_deref(), Some("wyvernmanager-02.html"));
        assert_eq!(
            event(&mut t, npc, "Return").as_deref(),
            Some("wyvernmanager-02.html")
        );
    }

    #[test]
    fn help_depends_on_the_residence_type() {
        let (mut t, npc) = setup();
        assert!(
            event(&mut t, npc, "Help")
                .unwrap()
                .contains("wyvernmanager-03.html")
        );
    }

    #[test]
    fn riding_without_a_strider_tells_so() {
        let (mut t, npc) = setup();
        let page = event(&mut t, npc, "RideWyvern").unwrap();
        assert!(page.contains("wyvernmanager-05.html"));
        assert!(names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn a_strider_rider_who_is_no_lord_is_refused_and_pays_nothing() {
        let (mut t, npc) = setup();
        assert!(t.world.mount_npc(ObjectId(1), 12526, ObjectId(0), false));
        t.take();
        let page = event(&mut t, npc, "RideWyvern").unwrap();
        assert!(page.contains("wyvernmanager-06.html"));
        assert_eq!(t.world.mount_npc_id_of(ObjectId(1)), 12526);
        // Strider riders are not told off.
        assert!(!names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
    }
}
