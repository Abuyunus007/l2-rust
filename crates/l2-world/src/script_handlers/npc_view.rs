//! The `NpcViewMod` NPC bypass: the info, drop, spoil, skill and aggro pages of an NPC
//! (`AltGameViewNpc`).
// Java: dist/game/data/scripts/handlers/bypass/npc/NpcViewMod.java
// Java: dist/game/data/scripts/handlers/actions/shiftclick/NpcShiftClick.java (the `ALT_GAME_VIEWNPC`
//       branch), network/clientpackets/Action.java (who may shift click)
// Java: util/HtmlUtil.java (`getHpGauge`, `getMpGauge`)
//
// The drop rates shown are the ones a kill of this player would roll (`rewards/drop.rs`): the
// server rates and the player's bonus stats. The premium rates are left out because the premium
// system is off (`PremiumSystem.ini`, "premium answers as off").
//
// `DropSearchBoard` shares this drop code in Java; see the card's report for what is left.

use l2_data::NpcTemplate;
use l2_data::Stat;
use l2_data::item::ADENA_ID;
use l2_data::npc::{DropEntry, DropType};
use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::warn;

use crate::bypass::BypassHandlers;
use crate::npc::NpcClass;
use crate::rewards::drop::{DropKiller, group_list, ungrouped_list};
use crate::shiftclick::npc::element_name;
use crate::world::World;

/// `DROP_LIST_ITEMS_PER_PAGE`.
const DROP_LIST_ITEMS_PER_PAGE: i64 = 10;

/// Registers the handler of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    handlers.register("NpcViewMod", npc_view_mod_handler);
}

/// Java `HtmlUtil.getGauge` with the HP and MP images (`displayAsPercentage` is false here).
fn gauge(width: i64, current_value: i64, max: i64, background: &str, image: &str) -> String {
    let current = current_value.min(max);
    let fill = ((current as f64 / max as f64) * width as f64) as i64;
    let td_width = (width - 10) / 2;
    format!(
        "<table width={width} cellpadding=0 cellspacing=0><tr><td background=\"{background}\"><img src=\"{image}\" width={fill} height=17></td></tr><tr><td align=center><table cellpadding=0 cellspacing=-13><tr><td><table cellpadding=0 cellspacing=0><tr><td width={td_width} align=right>{current}</td><td width=10 align=center>/</td><td width={td_width}>{max}</td></tr></table></td></tr></table></td></tr></table>"
    )
}

/// Java `new DecimalFormat("#,###").format(value)`.
fn amount_format(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// Java `new DecimalFormat("0.00##").format(value)`: two to four decimals (half even).
pub(crate) fn chance_format(value: f64) -> String {
    let text = format!("{value:.4}");
    let Some((int, frac)) = text.split_once('.') else {
        return text;
    };
    let mut frac = frac.to_owned();
    while frac.len() > 2 && frac.ends_with('0') {
        frac.pop();
    }
    format!("{int}.{frac}")
}

/// The respawn text of `sendNpcView`: the delays in the largest unit both divide evenly into.
fn respawn_text(min_ms: i32, max_ms: i32, min_multiplier: f32, max_multiplier: f32) -> String {
    if min_ms == 0 {
        return "None".to_owned();
    }
    let min_delay = (min_ms as f32 * min_multiplier) as i64;
    let max_delay = (max_ms as f32 * max_multiplier) as i64;
    // The `TimeUnit`s from the milliseconds up: the one with the smallest exact value.
    let units: [(&str, i64); 5] = [
        ("Milliseconds", 1),
        ("Seconds", 1000),
        ("Minutes", 60_000),
        ("Hours", 3_600_000),
        ("Days", 86_400_000),
    ];
    let (mut name, mut size) = units[0];
    let mut best = i64::from(min_ms);
    for (unit_name, unit_size) in units {
        let min_in_unit = i64::from(min_ms) / unit_size;
        let max_in_unit = i64::from(max_ms) / unit_size;
        if min_in_unit * unit_size == i64::from(min_ms)
            && max_in_unit * unit_size == i64::from(max_ms)
            && best > min_in_unit
        {
            best = min_in_unit;
            name = unit_name;
            size = unit_size;
        }
    }
    let (min_shown, max_shown) = (min_delay / size, max_delay / size);
    // `hasRespawnRandom()`: the delays differ.
    if min_ms != max_ms {
        format!("{min_shown}-{max_shown} {name}")
    } else {
        format!("{min_shown} {name}")
    }
}

/// Java `NpcViewMod.onCommand`.
fn npc_view_mod_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    _origin: Option<ObjectId>,
) -> bool {
    let mut tokens = command.split_whitespace();
    tokens.next();
    let Some(actual) = tokens.next() else {
        warn!("Bypass[NpcViewMod] used without enough parameters.");
        return false;
    };
    let rest: Vec<&str> = tokens.collect();
    match actual.to_lowercase().as_str() {
        "view" | "skills" | "aggrolist" => {
            let target = if let Some(id) = rest.first() {
                match id.parse::<i32>() {
                    Ok(id) => Some(ObjectId(id)),
                    Err(_) => return false,
                }
            } else {
                world.target_of(player)
            };
            let Some(npc) = target.filter(|t| world.is_plain_npc(*t)) else {
                return false;
            };
            match actual.to_lowercase().as_str() {
                "view" => world.send_npc_view(player, npc),
                "skills" => world.send_npc_skill_view(player, npc),
                _ => world.send_aggro_list_view(player, npc),
            }
        }
        "droplist" => {
            if rest.len() < 2 {
                warn!("Bypass[NpcViewMod] used without enough parameters.");
                return false;
            }
            let Some(drop_type) = DropType::from_name(rest[0]) else {
                warn!("Bypass[NpcViewMod] unknown drop list scope: {}", rest[0]);
                return false;
            };
            let Ok(id) = rest[1].parse::<i32>() else {
                return false;
            };
            let Some(npc) = Some(ObjectId(id)).filter(|t| world.is_plain_npc(*t)) else {
                return false;
            };
            let page = match rest.get(2) {
                Some(p) => match p.parse::<i32>() {
                    Ok(p) => p,
                    Err(_) => return false,
                },
                None => 0,
            };
            world.send_npc_drop_list(player, npc, drop_type, page);
        }
        _ => {}
    }
    true
}

impl World {
    /// `target instanceof Npc` (a summon is not an `Npc` in Java).
    pub(crate) fn is_plain_npc(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some() && self.npcs.get(&id).is_some_and(|n| n.class != NpcClass::Summon)
    }

    /// The `ALT_GAME_VIEWNPC` branch of `NpcShiftClick.onAction` for a player: target the NPC and
    /// show its page while it is alive.
    pub(crate) fn npc_view_shift_click(&mut self, player: ObjectId, target: ObjectId) {
        if !self.is_plain_npc(target) {
            return;
        }
        self.set_target(player, Some(target));
        // Only show view if NPC is alive.
        if !self.is_dead(target) {
            self.send_npc_view(player, target);
        }
    }

    /// Java `NpcViewMod.sendNpcView`.
    pub(crate) fn send_npc_view(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(mut html) = self
            .html_from_file(player, 0, "data/html/mods/NpcView/Info.htm")
            .map(|m| m.html)
        else {
            return;
        };
        let (Some(n), Some(view)) = (self.npcs.get(&npc), self.stats(npc)) else {
            return;
        };
        let Some(template) = self.npc_world.template(n.npc_id) else {
            return;
        };
        let name = self.creature_name(npc);
        let max_hp = i64::from(view.max_hp());
        let max_mp = i64::from(view.max_mp());
        let mut replace = |pattern: &str, value: String| {
            html = html.replace(pattern, &value);
        };
        replace("%name%", name);
        replace("%npcId%", template.id.to_string());
        replace(
            "%hpGauge%",
            gauge(
                235,
                n.core.cur_hp as i64,
                max_hp,
                "L2UI_CT1.Gauges.Gauge_DF_Large_HP_bg_Center",
                "L2UI_CT1.Gauges.Gauge_DF_Large_HP_Center",
            ),
        );
        replace(
            "%mpGauge%",
            gauge(
                235,
                n.core.cur_mp as i64,
                max_mp,
                "L2UI_CT1.Gauges.Gauge_DF_Large_MP_bg_Center",
                "L2UI_CT1.Gauges.Gauge_DF_Large_MP_Center",
            ),
        );
        let respawn = match self.npc_spawn(npc) {
            Some(spawn) if spawn.respawn_min_ms != 0 => respawn_text(
                spawn.respawn_min_ms,
                spawn.respawn_max_ms,
                self.boss.settings.raid_min_respawn_multiplier,
                self.boss.settings.raid_max_respawn_multiplier,
            ),
            _ => "None".to_owned(),
        };
        replace("%respawn%", respawn);
        // `StringUtil.capitalizeFirst(getAttackType().name().toLowerCase())`.
        let attack_type = template.base_attack_type.name().to_lowercase();
        let mut chars = attack_type.chars();
        let capitalized = chars
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
            .unwrap_or_default();
        replace("%atktype%", capitalized);
        replace("%atkrange%", view.physical_attack_range().to_string());
        replace("%patk%", (view.p_atk(Some(player)) as i32).to_string());
        replace("%pdef%", (view.p_def(Some(player)) as i32).to_string());
        replace("%matk%", (view.m_atk(Some(player), None) as i32).to_string());
        replace("%mdef%", (view.m_def(Some(player), None) as i32).to_string());
        replace("%atkspd%", (view.p_atk_spd() as i32).to_string());
        replace("%castspd%", view.m_atk_spd().to_string());
        replace("%critrate%", view.critical_hit(Some(player), None).to_string());
        replace("%evasion%", view.evasion_rate(Some(player)).to_string());
        replace("%accuracy%", view.accuracy().to_string());
        replace("%speed%", (view.move_speed() as i32).to_string());
        let attack_element = view.attack_element();
        replace(
            "%attributeatktype%",
            element_name(i32::from(attack_element)).to_owned(),
        );
        replace(
            "%attributeatkvalue%",
            view.attack_element_value(attack_element).to_string(),
        );
        for (pattern, element) in [
            ("%attributefire%", 0),
            ("%attributewater%", 1),
            ("%attributewind%", 2),
            ("%attributeearth%", 3),
            ("%attributedark%", 5),
            ("%attributeholy%", 4),
        ] {
            replace(pattern, view.defense_element_value(element).to_string());
        }
        let buttons = self.npc_view_menu_buttons(npc, template);
        html = html.replace("%menuButtons%", &buttons);
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }

    /// Java `NpcViewMod.sendNpcSkillView`.
    fn send_npc_skill_view(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(mut html) = self
            .html_from_file(player, 0, "data/html/mods/NpcView/Skills.htm")
            .map(|m| m.html)
        else {
            return;
        };
        let Some(template) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
        else {
            return;
        };
        let mut skills = template.skills.clone();
        // NOTE: Java walks a hash map keyed by skill id; the ids are small, so ascending.
        skills.sort_by_key(|s| s.id);
        let table = self.skill_table();
        let mut sb = String::new();
        for reference in skills {
            let Some(skill) = table.as_ref().and_then(|t| t.get(reference.id, reference.level)) else {
                continue;
            };
            sb.push_str("<table width=260 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
            sb.push_str("<tr><td width=32>");
            sb.push_str(&format!("<img src=\"{}\" width=32 height=32>", skill.icon));
            sb.push_str(&format!("</td><td width=110>{}</td>", skill.name));
            sb.push_str(&format!("<td width=45 align=center>{}</td>", skill.id));
            sb.push_str(&format!(
                "<td width=35 align=center>{}</td></tr></table>",
                skill.level
            ));
        }
        html = html.replace("%skills%", &sb);
        html = html.replace("%npc_name%", &self.creature_name(npc));
        html = html.replace("%npcId%", &template.id.to_string());
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }

    /// Java `NpcViewMod.sendAggroListView`.
    fn send_aggro_list_view(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(mut html) = self
            .html_from_file(player, 0, "data/html/mods/NpcView/AggroList.htm")
            .map(|m| m.html)
        else {
            return;
        };
        let Some(template) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
        else {
            return;
        };
        let mut sb = String::new();
        if self.is_attackable_npc(npc) {
            let mut infos = self.aggro_list(npc).to_vec();
            // `comparingLong(getHate).thenComparingLong(getDamage).reversed()`.
            infos.sort_by_key(|a| std::cmp::Reverse((a.hate, a.damage)));
            for a in infos {
                sb.push_str("<table width=260 height=32 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
                sb.push_str("<tr><td width=10></td>");
                sb.push_str(&format!("<td width=100>{}</td>", self.creature_name(a.attacker)));
                sb.push_str(&format!("<td width=70 align=center>{}</td>", a.hate));
                sb.push_str(&format!("<td width=70 align=center>{}</td>", a.damage));
                sb.push_str("<td width=10></td></tr></table>");
            }
        }
        let buttons = self.npc_view_menu_buttons(npc, template);
        html = html.replace("%aggrolist%", &sb);
        html = html.replace("%menuButtons%", &buttons);
        html = html.replace("%npc_name%", &self.creature_name(npc));
        html = html.replace("%npcId%", &template.id.to_string());
        html = html.replace("%objid%", &npc.0.to_string());
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }

    /// Java `NpcViewMod.getMenuButtons`.
    fn npc_view_menu_buttons(&self, npc: ObjectId, template: &NpcTemplate) -> String {
        let has_groups = !template.drop_groups.is_empty();
        let has_death = !template.drops.is_empty();
        let has_spoil = !template.spoils.is_empty();
        let mut sb = String::new();
        if has_groups || has_death || has_spoil {
            let button = |label: &str, action: String| {
                format!(
                    "<td align=center><button value=\"{label}\" width=62 height=21 action=\"bypass {action}\" back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>"
                )
            };
            sb.push_str("<table width=270 border=0 bgcolor=\"444444\"><tr>");
            sb.push_str(&button("Info", format!("NpcViewMod view {}", npc.0)));
            if has_groups || has_death {
                sb.push_str(&button("Drop", format!("NpcViewMod dropList DROP {}", npc.0)));
            }
            if has_spoil {
                sb.push_str(&button("Spoil", format!("NpcViewMod dropList SPOIL {}", npc.0)));
            }
            sb.push_str(&button("Aggro", format!("NpcViewMod aggrolist {}", npc.0)));
            sb.push_str("</tr></table><br>");
        }
        sb
    }

    /// The player's bonus stats as a drop roll reads them (`rewards/drop.rs`).
    pub(crate) fn npc_view_killer(&self, player: ObjectId) -> DropKiller {
        let level = self.players.get(&player).map_or(1, |p| p.core.level);
        match self.stats(player) {
            Some(view) => DropKiller {
                level,
                bonus_drop_rate: 1.0 + view.calc(Stat::BonusDropRate, 0.0) / 100.0,
                bonus_drop_amount: 1.0 + view.calc(Stat::BonusDropAmount, 0.0) / 100.0,
                bonus_drop_adena: 1.0 + view.calc(Stat::BonusDropAdena, 0.0) / 100.0,
                bonus_spoil_rate: 1.0 + view.calc(Stat::BonusSpoilRate, 0.0) / 100.0,
            },
            None => DropKiller::plain(level),
        }
    }

    /// Java `NpcViewMod.sendNpcDropList`.
    fn send_npc_drop_list(&mut self, player: ObjectId, npc: ObjectId, drop_type: DropType, page_value: i32) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .cloned()
        else {
            return;
        };
        let settings = self.settings.drops.clone();
        let mut drop_list: Vec<DropEntry> = Vec::new();
        if drop_type == DropType::Spoil {
            drop_list = ungrouped_list(&settings, &templates, &template, DropType::Spoil);
        } else {
            drop_list.extend(ungrouped_list(&settings, &templates, &template, DropType::Drop));
            for group in group_list(&templates, &template) {
                // `dropGroup.getChance() / 100`.
                let chance = group.chance / 100.0;
                for entry in group.items {
                    drop_list.push(DropEntry {
                        chance: entry.chance * chance,
                        ..entry
                    });
                }
            }
        }
        // A list Java has as `null` (nothing in it): the command does nothing.
        if drop_list.is_empty() {
            return;
        }
        drop_list.sort_by_key(|d| d.item_id);

        let size = drop_list.len() as i64;
        let mut pages = size / DROP_LIST_ITEMS_PER_PAGE;
        if DROP_LIST_ITEMS_PER_PAGE * pages < size {
            pages += 1;
        }
        let mut pages_html = String::new();
        if pages > 1 {
            pages_html.push_str("<table><tr>");
            for i in 0..pages {
                pages_html.push_str(&format!(
                    "<td align=center><button value=\"{}\" width=20 height=20 action=\"bypass NpcViewMod dropList {} {} {}\" back=\"L2UI_CT1.Button_DF_Calculator_Down\" fore=\"L2UI_CT1.Button_DF_Calculator\"></td>",
                    i + 1,
                    drop_type.name(),
                    npc.0,
                    i
                ));
            }
            pages_html.push_str("</tr></table>");
        }
        let mut page = i64::from(page_value);
        if page >= pages {
            page = pages - 1;
        }
        let start = if page > 0 {
            page * DROP_LIST_ITEMS_PER_PAGE
        } else {
            0
        };
        let end = (page * DROP_LIST_ITEMS_PER_PAGE + DROP_LIST_ITEMS_PER_PAGE).min(size);

        let killer = self.npc_view_killer(player);
        let is_raid = self.is_raid(npc);
        let mut total = String::new();
        let mut limit_reached = String::new();
        let mut i = start;
        while i < end {
            let drop_item = drop_list[i as usize];
            i += 1;
            let item = templates.get(drop_item.item_id);
            let ex_immediate = item.is_some_and(|t| t.ex_immediate_effect);

            // real time server rate calculations
            let mut rate_chance = 1.0_f64;
            let mut rate_amount = 1.0_f64;
            if drop_type == DropType::Spoil {
                rate_chance = f64::from(settings.rate_spoil_drop_chance_multiplier);
                rate_amount = f64::from(settings.rate_spoil_drop_amount_multiplier);
                // Premium rates: the premium system is off. Bonus spoil rate effect:
                rate_chance *= killer.bonus_spoil_rate;
            } else {
                if let Some(by_id) = settings.rate_drop_chance_by_id.get(&drop_item.item_id) {
                    rate_chance *= f64::from(*by_id);
                    if drop_item.item_id == ADENA_ID && rate_chance > 100.0 {
                        rate_chance = 100.0;
                    }
                } else if ex_immediate {
                    rate_chance *= f64::from(settings.rate_herb_drop_chance_multiplier);
                } else if is_raid {
                    rate_chance *= f64::from(settings.rate_raid_drop_chance_multiplier);
                } else {
                    rate_chance *= f64::from(settings.rate_death_drop_chance_multiplier);
                }
                if let Some(by_id) = settings.rate_drop_amount_by_id.get(&drop_item.item_id) {
                    rate_amount *= f64::from(*by_id);
                } else if ex_immediate {
                    rate_amount *= f64::from(settings.rate_herb_drop_amount_multiplier);
                } else if is_raid {
                    rate_amount *= f64::from(settings.rate_raid_drop_amount_multiplier);
                } else {
                    rate_amount *= f64::from(settings.rate_death_drop_amount_multiplier);
                }
                // Premium rates: off. Bonus drop amount effect:
                rate_amount *= killer.bonus_drop_amount;
                if drop_item.item_id == ADENA_ID {
                    rate_amount *= killer.bonus_drop_adena;
                }
                // Bonus drop rate effect:
                rate_chance *= killer.bonus_drop_rate;
            }

            // Do not display zero chance drops.
            if rate_chance == 0.0 {
                continue;
            }

            let icon = item
                .and_then(|t| t.icon.clone())
                .unwrap_or_else(|| "icon.etc_question_mark_i00".to_owned());
            let mut sb = String::new();
            sb.push_str("<table width=332 cellpadding=2 cellspacing=0 background=\"L2UI_CT1.Windows.Windows_DF_TooltipBG\">");
            sb.push_str("<tr><td width=32 valign=top>");
            sb.push_str(&format!("<img src=\"{icon}\" width=32 height=32>"));
            sb.push_str("</td><td fixwidth=300 align=center><font name=\"hs9\" color=\"CD9000\">");
            sb.push_str(&format!("&#{};", drop_item.item_id));
            sb.push_str("</font></td></tr><tr><td width=32></td><td width=300><table width=295 cellpadding=0 cellspacing=0>");
            sb.push_str("<tr><td width=48 align=right valign=top><font color=\"LEVEL\">Amount:</font></td>");
            sb.push_str("<td width=247 align=center>");
            let min = (drop_item.min as f64 * rate_amount) as i64;
            let max = (drop_item.max as f64 * rate_amount) as i64;
            if min == max {
                sb.push_str(&amount_format(min));
            } else {
                sb.push_str(&amount_format(min));
                sb.push_str(" - ");
                sb.push_str(&amount_format(max));
            }
            sb.push_str(
                "</td></tr><tr><td width=48 align=right valign=top><font color=\"LEVEL\">Chance:</font></td>",
            );
            sb.push_str("<td width=247 align=center>");
            sb.push_str(&chance_format((drop_item.chance * rate_chance).min(100.0)));
            sb.push_str(
                "%</td></tr></table></td></tr><tr><td width=32></td><td width=300>&nbsp;</td></tr></table>",
            );
            // The client truncates the page above 17200.
            if sb.len() + total.len() < 16000 {
                total.push_str(&sb);
            } else {
                limit_reached = "<br><center>Too many drops! Could not display them all!</center>".to_owned();
            }
        }

        let body = format!("<table><tr><td>{total}</td></tr></table>");
        let Some(mut html) = self
            .html_from_file(player, 0, "data/html/mods/NpcView/DropList.htm")
            .map(|m| m.html)
        else {
            return;
        };
        let buttons = self.npc_view_menu_buttons(npc, &template);
        html = html.replace("%name%", &self.creature_name(npc));
        html = html.replace("%menuButtons%", &buttons);
        html = html.replace("%pages%", &pages_html);
        html = html.replace("%items%", &format!("{body}{limit_reached}"));
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_formatted_like_decimal_format() {
        assert_eq!(amount_format(0), "0");
        assert_eq!(amount_format(999), "999");
        assert_eq!(amount_format(1000), "1,000");
        assert_eq!(amount_format(1_234_567), "1,234,567");
        assert_eq!(chance_format(100.0), "100.00");
        assert_eq!(chance_format(0.5), "0.50");
        assert_eq!(chance_format(12.3456), "12.3456");
        assert_eq!(chance_format(12.34), "12.34");
        assert_eq!(chance_format(0.12345), "0.1235");
    }

    #[test]
    fn respawn_uses_the_largest_exact_unit() {
        assert_eq!(respawn_text(0, 0, 1.0, 1.0), "None");
        assert_eq!(respawn_text(60_000, 60_000, 1.0, 1.0), "1 Minutes");
        assert_eq!(respawn_text(60_000, 120_000, 1.0, 1.0), "1-2 Minutes");
        assert_eq!(respawn_text(90_000, 90_000, 2.0, 2.0), "180 Seconds");
        assert_eq!(respawn_text(1500, 1500, 1.0, 1.0), "1500 Milliseconds");
        assert_eq!(respawn_text(86_400_000, 86_400_000, 1.0, 1.0), "1 Days");
    }

    #[test]
    fn the_gauge_is_filled_by_the_share() {
        let html = gauge(235, 50, 100, "bg", "img");
        assert!(html.contains("width=117 height=17"), "{html}");
        assert!(html.contains("<td width=112 align=right>50</td>"), "{html}");
    }
}
