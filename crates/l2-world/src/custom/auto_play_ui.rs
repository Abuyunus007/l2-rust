//! The voiced commands of the auto play: `.play`, `.playskills`, `.playitems`, `.playpotion` and
//! the pages they open.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/AutoPlay.java (onCommand)
// Java: util/HtmlUtil.java (countPageNumber)
//
// The lists Java builds from `ConcurrentHashMap`s (the skills of a creature by skill id, the items
// of an inventory by object id) are shown in the order of the hash table (`java_hash_set_order`).

use l2_data::SkillTemplate;
use l2_data::stats::AbnormalType;
use l2_protocol_game::server::{ActionFailed, ExShowScreenMessage, NpcHtmlMessage};
use l2_util::ObjectId;

use crate::effects::apply::has_negative_effect;
use crate::world::World;

use super::auto_play::AUTO_ATTACK_ACTION;
use super::java_hash_set_order;

/// `AutoPlay.PAGE_LIMIT`.
const PAGE_LIMIT: usize = 7;

/// Which of the three list pages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListPage {
    Skills,
    Items,
    Potions,
}

impl ListPage {
    fn bypass(self) -> &'static str {
        match self {
            Self::Skills => ".playskills",
            Self::Items => ".playitems",
            Self::Potions => ".playpotion",
        }
    }
}

/// One row of a list page.
struct Row {
    id: i32,
    icon: String,
    name: String,
    active: bool,
}

/// Java `String.split(" ")`: trailing empty strings are dropped, and the text `""` is one entry.
fn java_split(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(' ').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `StringUtil.isNumeric`: digits only (and not empty).
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

impl World {
    /// `AutoPlay.onCommand` through `VoicedCommandHandler.getHandler(word)`. `None`: auto play is
    /// off (`MasterHandler` registers the handler only with `EnableAutoPlay`) or the word is
    /// another.
    pub(crate) fn auto_play_voiced_handler(
        &mut self,
        player: ObjectId,
        word: &str,
        params: Option<&str>,
    ) -> Option<bool> {
        let rules = std::sync::Arc::clone(&self.auto_play.rules);
        if !rules.enable_auto_play || !matches!(word, "play" | "playskills" | "playitems" | "playpotion") {
            return None;
        }
        if rules.auto_play_premium && !self.has_premium_status(player) {
            self.send(
                player,
                &ExShowScreenMessage::text("This command is only available to premium players.", 5000),
            );
            self.send_message(player, "This command is only available to premium players.");
            self.send(player, &ActionFailed);
            return Some(false);
        }
        match word {
            "play" => self.play_command(player, params),
            "playskills" => return self.play_list(player, params, ListPage::Skills),
            "playitems" => return self.play_list(player, params, ListPage::Items),
            _ => return self.play_list(player, params, ListPage::Potions),
        }
        Some(true)
    }

    /// The `play` case.
    fn play_command(&mut self, player: ObjectId, params: Option<&str>) {
        let rules = std::sync::Arc::clone(&self.auto_play.rules);
        if let Some(params) = params {
            let lower = params.to_lowercase();
            let args = java_split(&lower);
            let state = self.auto_player_mut(player);
            match args[0] {
                "attack" => {
                    if let Some(at) = state.actions.iter().position(|a| *a == AUTO_ATTACK_ACTION) {
                        state.actions.remove(at);
                    } else {
                        state.actions.push(AUTO_ATTACK_ACTION);
                    }
                }
                "loot" => state.pickup = !state.pickup,
                "respect" => state.respectful_hunting = !state.respectful_hunting,
                "range" => state.short_range = !state.short_range,
                "mode0" => state.next_target_mode = 0,
                "mode1" => state.next_target_mode = 1,
                "mode2" => state.next_target_mode = 2,
                "mode3" => state.next_target_mode = 3,
                "percent" => {
                    if args.len() > 1 && is_numeric(args[1]) {
                        // `Integer.parseInt` throws for a number over the int range.
                        if let Ok(value) = args[1].parse::<i32>() {
                            state.potion_percent = value.clamp(0, 100);
                        }
                    }
                }
                "start" => {
                    self.start_auto_play(player);
                    self.start_auto_use_task(player);
                }
                "stop" => {
                    self.stop_auto_play(player);
                    self.stop_auto_use_task(player);
                }
                _ => {}
            }
        }

        let mut content = self
            .get_htm(player, "data/html/mods/AutoPlay/Main.htm")
            .unwrap_or_default();
        let state = self.auto_player_mut(player).clone();
        let check = |on: bool| {
            if on {
                "L2UI.CheckBox_checked"
            } else {
                "L2UI.CheckBox"
            }
        };
        let radio = |on: bool| {
            if on {
                "L2UI_CH3.radiobutton2"
            } else {
                "L2UI_CH3.radiobutton1"
            }
        };
        let replace = |content: &mut String, key: &str, value: &str| *content = content.replace(key, value);
        replace(
            &mut content,
            "%attack%",
            check(state.actions.contains(&AUTO_ATTACK_ACTION)),
        );
        replace(&mut content, "%loot%", check(state.pickup));
        replace(&mut content, "%respect%", check(state.respectful_hunting));
        replace(&mut content, "%range%", check(!state.short_range));
        for mode in 0..4 {
            replace(
                &mut content,
                &format!("%mode{mode}%"),
                radio(state.next_target_mode == mode),
            );
        }
        let button = |action: &str, label: &str| {
            format!(
                "<br><table width=295><tr><td height=31><center><button action=\"bypass voice {action}\" value=\"{label}\" width=200 height=31 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></center></td></tr></table>"
            )
        };
        replace(
            &mut content,
            "%skill_button%",
            &if rules.enable_auto_skill {
                button(".playskills", "Select Skills")
            } else {
                String::new()
            },
        );
        replace(
            &mut content,
            "%item_button%",
            &if rules.enable_auto_item {
                button(".playitems", "Select Supply Items")
            } else {
                String::new()
            },
        );
        replace(
            &mut content,
            "%potion_button%",
            &if rules.enable_auto_potion {
                "<br><table width=295><tr><td height=31><center><button action=\"bypass voice .playpotion\" value=\"Select Healing Potion\" width=200 height=31 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></center></td></tr><tr><td height=31><center><table width=150><tr><td width=120><font color=\"CDB67F\">HP Percent (%percent%)</font></td><td><edit var=\"percentbox\" width=30 height=15></td><td><button value=\"Apply\" action=\"bypass voice .play percent $percentbox\" width=45 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td></tr></table></center></td></tr></table>".to_owned()
            } else {
                String::new()
            },
        );
        replace(&mut content, "%percent%", &state.potion_percent.to_string());
        let (action, label) = if state.auto_playing {
            ("stop", "Stop")
        } else {
            ("start", "Start")
        };
        replace(
            &mut content,
            "%status_button%",
            &format!(
                "<table width=295><tr><td height=31><center><table width=295><tr><td height=31><center><button action=\"bypass voice .play {action}\" value=\"{label}\" width=200 height=31 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td></tr></table></center></td></tr></table>"
            ),
        );
        self.send_html(
            player,
            &NpcHtmlMessage {
                npc_obj_id: 0,
                html: content,
                item_id: 0,
            },
        );
    }

    /// The `playskills`, `playitems` and `playpotion` cases: toggle the entry named by the second
    /// word, then show the page of the first.
    fn play_list(&mut self, player: ObjectId, params: Option<&str>, page: ListPage) -> Option<bool> {
        let rules = std::sync::Arc::clone(&self.auto_play.rules);
        let file = match page {
            ListPage::Skills => "Skills.htm",
            ListPage::Items => "Items.htm",
            ListPage::Potions => "Potion.htm",
        };
        let content = self
            .get_htm(player, &format!("data/html/mods/AutoPlay/{file}"))
            .unwrap_or_default();
        let table = self.skill_table()?;
        let templates = self.item_templates()?;
        let args: Vec<&str> = params.map_or_else(Vec::new, java_split);

        let rows: Vec<Row> = match page {
            ListPage::Skills => {
                let skills = self.play_skill_candidates(player, &rules.disabled_auto_skills);
                // Manage skill activation.
                if args.len() > 1
                    && let Ok(skill_id) = args[1].parse::<i32>()
                {
                    let known = self.known_skill(player, skill_id);
                    if known.is_none()
                        && let Some(summon) = self.summon_of(player)
                        && self.known_skill(summon, skill_id).is_some()
                    {
                        // NOTE(java-quirk): a skill only the summon knows `break`s out of the
                        // switch in Java: nothing is toggled and no page is sent.
                        return Some(true);
                    }
                    if rules.enable_auto_skill
                        && let Some(key) = known
                        && skills.contains(&(key.id, key.level))
                    {
                        let negative = table.get(key.id, key.level).is_some_and(has_negative_effect);
                        let state = self.auto_player_mut(player);
                        let list = if negative {
                            &mut state.skills
                        } else {
                            &mut state.buffs
                        };
                        if let Some(at) = list.iter().position(|s| *s == skill_id) {
                            list.remove(at);
                        } else {
                            list.push(skill_id);
                        }
                    }
                }
                let state = self.auto_player_mut(player).clone();
                skills
                    .iter()
                    .filter_map(|(id, level)| table.get(*id, *level))
                    .map(|skill| Row {
                        id: skill.id,
                        icon: skill.icon.clone(),
                        name: skill.name.clone(),
                        active: state.buffs.contains(&skill.id) || state.skills.contains(&skill.id),
                    })
                    .collect()
            }
            ListPage::Items | ListPage::Potions => {
                let potions = page == ListPage::Potions;
                let items = self.play_item_candidates(player, potions, &rules.disabled_auto_items);
                // Manage item activation.
                if args.len() > 1
                    && let Ok(item_id) = args[1].parse::<i32>()
                {
                    if potions {
                        if rules.enable_auto_potion && items.contains(&item_id) {
                            let state = self.auto_player_mut(player);
                            state.potion_item = if state.potion_item == item_id { 0 } else { item_id };
                        }
                    } else if rules.enable_auto_item && items.contains(&item_id) {
                        let state = self.auto_player_mut(player);
                        if let Some(at) = state.supply_items.iter().position(|i| *i == item_id) {
                            state.supply_items.remove(at);
                        } else {
                            state.supply_items.push(item_id);
                        }
                    }
                }
                let state = self.auto_player_mut(player).clone();
                items
                    .iter()
                    .filter_map(|id| templates.get(*id))
                    .map(|template| Row {
                        id: template.id,
                        icon: template.icon.clone().unwrap_or_else(|| "null".to_owned()),
                        name: template.name.clone(),
                        active: if potions {
                            state.potion_item == template.id
                        } else {
                            state.supply_items.contains(&template.id)
                        },
                    })
                    .collect()
            }
        };

        // Calculate page number.
        let max = rows.len().div_ceil(PAGE_LIMIT);
        let mut number: i64 = match params {
            None => 1,
            // `Integer.parseInt(paramArray[0])` throws for a word that is no number.
            Some(_) => args[0].parse::<i64>().ok()?,
        };
        if number > max as i64 {
            number = max as i64;
        }
        let first = usize::try_from((number - 1) * PAGE_LIMIT as i64).unwrap_or(0);
        let last = usize::try_from(number * PAGE_LIMIT as i64)
            .unwrap_or(0)
            .min(rows.len());
        let shown = if first < last { &rows[first..last] } else { &[][..] };
        let command = page.bypass();
        let mut sb = String::new();
        if shown.is_empty() {
            sb.push_str(match page {
                ListPage::Skills => "<center><br>No skills found.<br></center>",
                ListPage::Items => "<center><br>No items found.<br></center>",
                ListPage::Potions => "<center><br>No potions found.<br></center>",
            });
        } else {
            // Java never increments `row`: the first background colour is used for every row.
            for row in shown {
                sb.push_str("<table width=\"295\" bgcolor=\"000000\"><tr>");
                if row.active {
                    sb.push_str(&format!(
                        "<td height=40 width=40><img src=\"{}\" width=32 height=32></td><td width=190>{}</td><td><button value=\" \" action=\"bypass voice {command} {number} {}\" width=32 height=32 back=\"L2UI_CH3.mapbutton_zoomout2\" fore=\"L2UI_CH3.mapbutton_zoomout1\"></td>",
                        row.icon, row.name, row.id
                    ));
                } else {
                    sb.push_str(&format!(
                        "<td height=40 width=40><img src=\"{}\" width=32 height=32></td><td width=190><font color=\"B09878\">{}</font></td><td><button value=\" \" action=\"bypass voice {command} {number} {}\" width=32 height=32 back=\"L2UI_CH3.mapbutton_zoomin2\" fore=\"L2UI_CH3.mapbutton_zoomin1\"></td>",
                        row.icon, row.name, row.id
                    ));
                }
                sb.push_str("</tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>");
            }
            // Generate page footer.
            sb.push_str(
                "<br><img src=\"L2UI.SquareGray\" width=295 height=1><table width=\"100%\" bgcolor=000000><tr>",
            );
            if number > 1 {
                sb.push_str(&format!(
                    "<td align=left width=70><a action=\"bypass voice {command} {}\"><font color=\"CDB67F\">Previous</font></a></td>",
                    number - 1
                ));
            } else {
                sb.push_str("<td align=left width=70><font color=\"B09878\">Previous</font></td>");
            }
            sb.push_str(&format!("<td align=center width=100>Page {number} of {max}</td>"));
            if number < max as i64 {
                sb.push_str(&format!(
                    "<td align=right width=70><a action=\"bypass voice {command} {}\"><font color=\"CDB67F\">Next</font></a></td>",
                    number + 1
                ));
            } else {
                sb.push_str("<td align=right width=70><font color=\"B09878\">Next</font></td>");
            }
            sb.push_str("</tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>");
        }

        // Replace and send the html.
        let key = if page == ListPage::Skills {
            "%skills%"
        } else {
            "%items%"
        };
        self.send_html(
            player,
            &NpcHtmlMessage {
                npc_obj_id: 0,
                html: content.replace(key, &sb),
                item_id: 0,
            },
        );
        Some(true)
    }

    /// The skills `.playskills` offers, `(id, level)` in the order of Java's list.
    fn play_skill_candidates(&self, player: ObjectId, disabled: &[i32]) -> Vec<(i32, i32)> {
        let Some(table) = self.skill_table() else {
            return Vec::new();
        };
        let mut skills: Vec<(i32, i32)> = Vec::new();
        let allowed = |skills: &[(i32, i32)], id: i32, level: i32| {
            table
                .get(id, level)
                .is_some_and(|s| !s.is_passive() && !s.is_toggle())
                && !skills.contains(&(id, level))
                && !disabled.contains(&id)
        };
        // `SkillData.getSiegeSkills(true, true)`.
        let siege = |id: i32, level: i32| level == 1 && matches!(id, 246 | 247 | 326 | 844 | 845);
        let own = self.creature_skills_in_java_order(player);
        for (id, level) in own {
            if !siege(id, level) && allowed(&skills, id, level) {
                skills.push((id, level));
            }
        }
        if let Some(summon) = self.summon_of(player) {
            // NOTE: `PetSkillData.getKnownSkills(summon)` is left out.
            for (id, level) in self.creature_skills_in_java_order(summon) {
                if allowed(&skills, id, level) {
                    skills.push((id, level));
                }
            }
        }
        // Remove item skills.
        let worn: Vec<i32> = self
            .players
            .get(&player)
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| i.loc == "PAPERDOLL")
                    .map(|i| i.item_id)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(templates) = self.item_templates() {
            for item_id in worn {
                if let Some(item_skills) = templates.get(item_id).and_then(|t| t.skills.item_skills.as_ref())
                {
                    for holder in item_skills {
                        skills.retain(|s| *s != (holder.id, holder.level));
                    }
                }
            }
        }
        skills
    }

    /// `getAllSkills()` of a creature: a `ConcurrentHashMap` by skill id.
    fn creature_skills_in_java_order(&self, id: ObjectId) -> Vec<(i32, i32)> {
        let Some(core) = self.core_of(id) else {
            return Vec::new();
        };
        let ids: Vec<i32> = core.skills.iter().map(|(skill_id, _)| *skill_id).collect();
        java_hash_set_order(&ids)
            .into_iter()
            .filter_map(|skill_id| core.skills.iter().find(|(s, _)| *s == skill_id).copied())
            .collect()
    }

    /// The items `.playitems` (`potions == false`) and `.playpotion` offer, as template ids in the
    /// order of the inventory's `ConcurrentHashMap` (by object id).
    fn play_item_candidates(&self, player: ObjectId, potions: bool, disabled: &[i32]) -> Vec<i32> {
        let (Some(templates), Some(table), Some(p)) = (
            self.item_templates(),
            self.skill_table(),
            self.players.get(&player),
        ) else {
            return Vec::new();
        };
        let in_inventory: Vec<&crate::player::ItemData> = p
            .data
            .items
            .iter()
            .filter(|i| i.loc == "INVENTORY" || i.loc == "PAPERDOLL")
            .collect();
        let object_ids: Vec<i32> = in_inventory.iter().map(|i| i.object_id.0).collect();
        let mut out: Vec<i32> = Vec::new();
        for object_id in java_hash_set_order(&object_ids) {
            let Some(item) = in_inventory.iter().find(|i| i.object_id.0 == object_id) else {
                continue;
            };
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            if !template.is_etc_item() || !template.has_skills() || disabled.contains(&item.item_id) {
                continue;
            }
            let matches_skill = |skill: &SkillTemplate| {
                if potions {
                    skill.abnormal_type == AbnormalType::HpRecover
                } else {
                    skill.operate_type.is_continuous() && skill.abnormal_type != AbnormalType::HpRecover
                }
            };
            let wanted = template
                .skills
                .item_skills
                .iter()
                .flatten()
                .filter_map(|h| table.get(h.id, h.level))
                .any(matches_skill);
            if wanted && !out.contains(&template.id) {
                out.push(template.id);
            }
        }
        out
    }
}
