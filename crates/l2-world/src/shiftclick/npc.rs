//! `NpcShiftClick`: the GM window of an NPC (`npcinfo.htm`).
// Java: dist/game/data/scripts/handlers/actions/shiftclick/NpcShiftClick.java
// Java: entity/spawns/Spawn.java (isTerritoryBased, hasRespawnRandom)
// Java: managers/WalkingManager.java (getRouteName)
//
// Numbers are written like Java's `String.valueOf`: an `int` as is, a `double` or `float` through
// `{:?}` (`300.0`), which is Java's `Double.toString` for the values these stats take.

use l2_util::ObjectId;

use crate::ai::siege_guard::SiegeGuardAiKind;
use crate::movement::Intention;
use crate::npc::NpcClass;
use crate::target::{distance_2d, distance_3d};
use crate::world::World;

/// `<font color=FF0000>--</font>`: what a field shows when the NPC has no spawn.
const NO_SPAWN: &str = "<font color=FF0000>--</font>";

/// Java `Elementals.getElementName(element)`.
pub(crate) fn element_name(element: i32) -> &'static str {
    match element {
        0 => "Fire",
        1 => "Water",
        2 => "Wind",
        3 => "Earth",
        5 => "Dark",
        4 => "Holy",
        _ => "None",
    }
}

/// Java `Intention.name()`.
fn intention_name(intention: Intention) -> &'static str {
    match intention {
        Intention::Idle => "IDLE",
        Intention::Active => "ACTIVE",
        Intention::Rest => "REST",
        Intention::Attack => "ATTACK",
        Intention::Cast => "CAST",
        Intention::MoveTo => "MOVE_TO",
        Intention::Follow => "FOLLOW",
        Intention::PickUp => "PICK_UP",
        Intention::Interact => "INTERACT",
    }
}

/// One `<tr>` row of the AI block: a labelled value, in the dark or the plain table.
fn ai_row(label: &str, value: &str, dark: bool) -> String {
    let background = if dark { " bgcolor=131210" } else { "" };
    format!(
        "<tr><td><table width=270 border=0{background}><tr><td width=100><font color=FFAA00>{label}</font></td><td align=right width=170>{value}</td></tr></table></td></tr>"
    )
}

/// The respawn text: `None`, `min-max sec` or `min sec` (Java divides the milliseconds by 1000).
fn respawn_text(min_ms: i32, max_ms: i32) -> String {
    if min_ms == 0 {
        "None".to_owned()
    } else if min_ms != max_ms {
        format!("{}-{} sec", min_ms / 1000, max_ms / 1000)
    } else {
        format!("{} sec", min_ms / 1000)
    }
}

/// `spawnFile.substring(spawnFile.lastIndexOf('\\') + 1)`: Java stores the file with the separators
/// of the machine, so on Windows this is the file name and on Linux (no backslash) the whole path.
// NOTE(java-quirk): the Rust spawn file path uses `/` on every system; the part after the last
// separator is shown, as Java does on Windows.
fn spawn_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

impl World {
    /// Java `NpcShiftClick.onAction` for a GM.
    pub(super) fn npc_shift_click(&mut self, gm: ObjectId, target: ObjectId) {
        if !self.is_gm(gm) {
            // `else if (NpcConfig.ALT_GAME_VIEWNPC)`: the page of `NpcViewMod`.
            if self.script_handlers.settings.alt_game_viewnpc {
                self.npc_view_shift_click(gm, target);
            }
            return;
        }
        // Set the target of the GM.
        self.set_target(gm, Some(target));
        let Some(values) = self.npc_info_values(gm, target) else {
            return;
        };
        let mut html = self.admin_html_file(gm, "data/html/admin/npcinfo.htm");
        for (pattern, value) in &values {
            html.replace(pattern, value);
        }
        self.send_admin_html(gm, &html);
    }

    /// Every `html.replace(pattern, value)` of `NpcShiftClick`, in Java's order.
    fn npc_info_values(&self, gm: ObjectId, target: ObjectId) -> Option<Vec<(&'static str, String)>> {
        let npc = self.npcs.get(&target)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let view = self.stats(target)?;
        let gm_pos = self.position_of(gm)?;
        let pos = (npc.core.x, npc.core.y, npc.core.z);

        let attackable = self.is_attackable_npc(target);
        let mut v: Vec<(&'static str, String)> = Vec::with_capacity(64);
        v.push(("%objid%", target.0.to_string()));
        // `target.isFakePlayer() ? "Fake Player - " : ""` + the Java class name, which is the
        // template's type.
        let prefix = if template.fake_player {
            "Fake Player - "
        } else {
            ""
        };
        v.push(("%class%", format!("{prefix}{}", template.npc_type)));
        v.push(("%race%", template.race.name().to_owned()));
        v.push(("%id%", template.id.to_string()));
        v.push(("%lvl%", template.level.to_string()));
        v.push(("%name%", template.name.clone()));
        v.push(("%tmplid%", template.id.to_string()));
        v.push((
            "%aggro%",
            if attackable {
                self.aggro_range_of(target)
            } else {
                0
            }
            .to_string(),
        ));
        v.push(("%hp%", (npc.core.cur_hp as i32).to_string()));
        v.push(("%hpmax%", view.max_hp().to_string()));
        v.push(("%mp%", (npc.core.cur_mp as i32).to_string()));
        v.push(("%mpmax%", view.max_mp().to_string()));

        v.push(("%patk%", (view.p_atk(None) as i32).to_string()));
        v.push(("%matk%", (view.m_atk(None, None) as i32).to_string()));
        v.push(("%pdef%", (view.p_def(None) as i32).to_string()));
        v.push(("%mdef%", (view.m_def(None, None) as i32).to_string()));
        v.push(("%accu%", view.accuracy().to_string()));
        v.push(("%evas%", view.evasion_rate(None).to_string()));
        v.push(("%crit%", view.critical_hit(None, None).to_string()));
        v.push(("%rspd%", (view.run_speed() as i32).to_string()));
        v.push(("%aspd%", format!("{:?}", view.p_atk_spd())));
        v.push(("%cspd%", view.m_atk_spd().to_string()));
        v.push(("%atkType%", template.base_attack_type.name().to_owned()));
        v.push(("%atkRng%", template.base_attack_range.to_string()));
        v.push(("%str%", view.str().to_string()));
        v.push(("%dex%", view.dex().to_string()));
        v.push(("%con%", view.con().to_string()));
        v.push(("%int%", view.int().to_string()));
        v.push(("%wit%", view.wit().to_string()));
        v.push(("%men%", view.men().to_string()));
        v.push(("%loc%", format!("{} {} {}", pos.0, pos.1, pos.2)));
        v.push(("%heading%", npc.core.heading.to_string()));
        v.push(("%collision_radius%", format!("{:?}", view.collision_radius())));
        v.push(("%collision_height%", format!("{:?}", view.collision_height())));
        v.push(("%loc2d%", (distance_2d(gm_pos, pos) as i32).to_string()));
        v.push(("%loc3d%", (distance_3d(gm_pos, pos) as i32).to_string()));

        let attack_element = view.attack_element();
        v.push(("%ele_atk%", element_name(i32::from(attack_element)).to_owned()));
        v.push((
            "%ele_atk_value%",
            view.attack_element_value(attack_element).to_string(),
        ));
        for (pattern, element) in [
            ("%ele_dfire%", 0),
            ("%ele_dwater%", 1),
            ("%ele_dwind%", 2),
            ("%ele_dearth%", 3),
            ("%ele_dholy%", 4),
            ("%ele_ddark%", 5),
        ] {
            v.push((pattern, view.defense_element_value(element).to_string()));
        }

        self.push_spawn_values(target, &mut v);
        self.push_ai_values(target, &mut v);
        // `WalkingManager.getRouteName(npc)`.
        let route = self.walking_route_name(target);
        v.push((
            "%route%",
            if route.is_empty() {
                String::new()
            } else {
                format!(
                    "<tr><td><table width=270 border=0><tr><td width=100><font color=LEVEL>Route:</font></td><td align=right width=170>{route}</td></tr></table></td></tr>"
                )
            },
        ));
        Some(v)
    }

    /// The spawn rows: territory, type, file, place and respawn.
    fn push_spawn_values(&self, npc: ObjectId, v: &mut Vec<(&'static str, String)>) {
        let Some(spawn) = self.npc_spawn(npc) else {
            for pattern in ["%spawnfile%", "%territory%", "%spawntype%"] {
                v.push((pattern, NO_SPAWN.to_owned()));
            }
            v.push(("%spawn%", "<font color=FF0000>null</font>".to_owned()));
            v.push(("%resp%", NO_SPAWN.to_owned()));
            return;
        };
        let point = self.npcs.get(&npc).map_or((0, 0, 0), |n| n.spawn_point);
        v.push((
            "%territory%",
            spawn.territory.clone().unwrap_or_else(|| "None".to_owned()),
        ));
        // `isTerritoryBased()`: a territory and no fixed location.
        let territory_based = spawn.territory.is_some() && spawn.location.x == 0 && spawn.location.y == 0;
        v.push((
            "%spawntype%",
            if territory_based { "Random" } else { "Fixed" }.to_owned(),
        ));
        v.push(("%spawn%", format!("{} {} {}", point.0, point.1, point.2)));
        v.push(("%resp%", respawn_text(spawn.respawn_min_ms, spawn.respawn_max_ms)));
        v.push(("%spawnfile%", spawn_file_name(&spawn.source).to_owned()));
    }

    /// The AI rows (empty strings for an NPC that has no AI yet).
    fn push_ai_values(&self, npc: ObjectId, v: &mut Vec<(&'static str, String)>) {
        let patterns = [
            "%ai_intention%",
            "%ai%",
            "%ai_type%",
            "%ai_clan%",
            "%ai_enemy_clan%",
        ];
        let Some(template) = self.npc_template(npc).filter(|_| self.has_ai(npc)) else {
            for pattern in patterns {
                v.push((pattern, String::new()));
            }
            return;
        };
        // `StringUtil.implode(set.toArray(), ", ")`: a sorted set here, a hash set in Java (small
        // ids iterate in ascending order).
        let implode = |ids: &std::collections::BTreeSet<i32>| {
            ids.iter().map(i32::to_string).collect::<Vec<_>>().join(", ")
        };
        v.push((
            "%ai_intention%",
            ai_row("Intention:", intention_name(self.intention_of(npc)), true),
        ));
        v.push(("%ai%", ai_row("AI", self.ai_class_name(npc), false)));
        v.push(("%ai_type%", ai_row("AIType", template.ai_type.name(), true)));
        v.push((
            "%ai_clan%",
            ai_row(
                "Clan & Range:",
                &format!("{} {}", implode(&template.clans), template.clan_help_range),
                false,
            ),
        ));
        v.push((
            "%ai_enemy_clan%",
            ai_row(
                "Ignore & Range:",
                &format!(
                    "{} {}",
                    implode(&template.ignore_clan_npc_ids),
                    template.aggro_range
                ),
                true,
            ),
        ));
    }

    /// Java `getAI().getClass().getSimpleName()` of an NPC: `Creature.initAI`, `Attackable.initAI`
    /// and `Defender.initAI`.
    fn ai_class_name(&self, npc: ObjectId) -> &'static str {
        match self.npcs.get(&npc).map(|n| n.class) {
            Some(NpcClass::Summon) => "SummonAI",
            Some(NpcClass::Defender | NpcClass::FortCommander) => {
                match self.siege_guard_state(npc).map(|s| s.kind) {
                    Some(SiegeGuardAiKind::Castle) => "SiegeGuardAI",
                    Some(SiegeGuardAiKind::Hall) => "SpecialSiegeGuardAI",
                    _ => "FortSiegeGuardAI",
                }
            }
            _ if self.is_attackable_npc(npc) => "AttackableAI",
            _ => "CreatureAI",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respawn_text_follows_the_java_branches() {
        assert_eq!(respawn_text(0, 0), "None");
        assert_eq!(respawn_text(60_000, 60_000), "60 sec");
        assert_eq!(respawn_text(60_000, 90_500), "60-90 sec");
    }

    #[test]
    fn spawn_file_is_the_last_path_part() {
        assert_eq!(spawn_file_name("data/spawns/Others/a.xml"), "a.xml");
        assert_eq!(spawn_file_name("data\\spawns\\a.xml"), "a.xml");
        assert_eq!(spawn_file_name("a.xml"), "a.xml");
    }

    #[test]
    fn element_names_follow_elementals() {
        assert_eq!(element_name(0), "Fire");
        assert_eq!(element_name(4), "Holy");
        assert_eq!(element_name(5), "Dark");
        assert_eq!(element_name(-2), "None");
    }

    #[test]
    fn ai_rows_are_the_java_strings() {
        assert_eq!(
            ai_row("Intention:", "IDLE", true),
            "<tr><td><table width=270 border=0 bgcolor=131210><tr><td width=100><font color=FFAA00>Intention:</font></td><td align=right width=170>IDLE</td></tr></table></td></tr>"
        );
        assert_eq!(
            ai_row("AI", "CreatureAI", false),
            "<tr><td><table width=270 border=0><tr><td width=100><font color=FFAA00>AI</font></td><td align=right width=170>CreatureAI</td></tr></table></td></tr>"
        );
    }
}
