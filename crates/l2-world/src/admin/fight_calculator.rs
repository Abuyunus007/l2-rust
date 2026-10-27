//! `//fight_calculator`, `//fight_calculator_show`, `//fcs`: two creatures hit each other 10000
//! times with the real formulas and the page shows the averages.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminFightCalculator.java
//
// `//fight_calculator` picks two NPC templates (by level, then by name) and links to
// `//fight_calculator_show <id1> <id2>`, which builds two monsters from the templates. Without
// ids `//fcs` compares the GM ("me") with the target.
//
// Java builds `new Monster(template)` objects that are never spawned. The world has no off-world
// NPC, so the two monsters are created at (0, 0, 0) (where an unspawned Java object also sits)
// through the ordinary NPC creation, read, and deleted at the end of the command, all inside one
// call: nothing runs between them and no tick passes.
//
// NOTE(java-quirk): the first creature's attack adds the random damage multiplier to its attack
// (`+=`), the second multiplies by it (`*=`). The `shld` row shows the other creature's value.

use l2_data::Stat;
use l2_util::ObjectId;

use super::html::AdminHtml;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::combat::hit::{calc_crit_for, calc_hit_miss_for, calc_phys_dam_for, calc_shld_use_for};
use crate::formulas::{calculate_time_between_attacks, get_regenerate_period, random_damage_multiplier};
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// How many hits each side rolls (Java's two `i < 10000` loops).
const ROLLS: i32 = 10_000;

/// Java `AdminFightCalculator.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_fight_calculator", admin_fight_calculator),
        ("admin_fight_calculator_show", admin_fight_calculator_show),
        ("admin_fcs", admin_fight_calculator_show),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn no_such_element() -> JavaException {
    JavaException("java.util.NoSuchElementException".to_owned())
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// `//fight_calculator [lvl1 n] [lvl2 n] [mid1 id] [mid2 id]`: which page of the choice comes next.
fn admin_fight_calculator(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let (mut lvl1, mut lvl2, mut mid1, mut mid2) = (0, 0, 0, 0);
    while let Some(word) = st.next_token() {
        // `Integer.parseInt(st.nextToken())`: a missing value or a bad number is the exception.
        let slot = match word {
            "lvl1" => &mut lvl1,
            "lvl2" => &mut lvl2,
            "mid1" => &mut mid1,
            "mid2" => &mut mid2,
            _ => continue,
        };
        *slot = parse_int(st.next_token().ok_or_else(no_such_element)?)?;
    }

    let name_of = |world: &World, id: i32| {
        if id == 0 {
            None
        } else {
            world.npc_world.template(id).map(|t| (t.id, t.name.clone()))
        }
    };
    let npc1 = name_of(world, mid1);
    let npc2 = name_of(world, mid2);

    let html = if let (Some(n1), Some(n2)) = (&npc1, &npc2) {
        format!(
            "<html><title>Selected mobs to fight</title><body><table><tr><td>First</td><td>Second</td></tr><tr><td>level {lvl1}</td><td>level {lvl2}</td></tr><tr><td>id {}</td><td>id {}</td></tr><tr><td>{}</td><td>{}</td></tr></table><center><br><br><br><button value=\"OK\" action=\"bypass -h admin_fight_calculator_show {} {}\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
            n1.0, n2.0, n1.1, n2.1, n1.0, n2.0
        )
    } else if lvl1 != 0 && npc1.is_none() {
        let mut sb = String::from("<html><title>Select first mob to fight</title><body><table>");
        for (id, name) in templates_of_level(world, lvl1) {
            sb.push_str(&format!(
                "<tr><td><a action=\"bypass -h admin_fight_calculator lvl1 {lvl1} lvl2 {lvl2} mid1 {id} mid2 {mid2}\">{name}</a></td></tr>"
            ));
        }
        sb.push_str("</table></body></html>");
        sb
    } else if lvl2 != 0 && npc2.is_none() {
        let mut sb = String::from("<html><title>Select second mob to fight</title><body><table>");
        for (id, name) in templates_of_level(world, lvl2) {
            sb.push_str(&format!(
                "<tr><td><a action=\"bypass -h admin_fight_calculator lvl1 {lvl1} lvl2 {lvl2} mid1 {mid1} mid2 {id}\">{name}</a></td></tr>"
            ));
        }
        sb.push_str("</table></body></html>");
        sb
    } else {
        "<html><title>Select mobs to fight</title><body><table><tr><td>First</td><td>Second</td></tr><tr><td><edit var=\"lvl1\" width=80></td><td><edit var=\"lvl2\" width=80></td></tr></table><center><br><br><br><button value=\"OK\" action=\"bypass -h admin_fight_calculator lvl1 $lvl1 lvl2 $lvl2\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>".to_owned()
    };
    world.send_admin_html(gm, &AdminHtml::from_html(&html));
    Ok(())
}

/// Java `NpcData.getAllOfLevel(level)` (id and name), in id order (Java: map order).
fn templates_of_level(world: &World, level: i32) -> Vec<(i32, String)> {
    world
        .npc_world
        .templates_arc()
        .map(|templates| {
            templates
                .iter()
                .filter(|t| i32::from(t.level) == level)
                .map(|t| (t.id, t.name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// What one side of the calculation adds up over its 10000 hits.
#[derive(Default)]
struct Side {
    miss: i32,
    shld: i32,
    crit: i32,
    patk: f64,
    pdef: f64,
    dmg: f64,
    /// A hit landed: Java's `abortAttack()` ran.
    landed: bool,
}

/// Java's loop body for `attacker` against `target`. `add_random` is the `+=` of the first loop
/// and `false` the `*=` of the second.
fn roll(world: &mut World, attacker: ObjectId, target: ObjectId, add_random: bool) -> Side {
    world.with_rng(|world, rng| {
        let mut side = Side::default();
        for _ in 0..ROLLS {
            let missed = calc_hit_miss_for(world, attacker, target, rng);
            if missed {
                side.miss += 1;
            }
            let shld = calc_shld_use_for(world, attacker, target, rng);
            if shld > 0 {
                side.shld += 1;
            }
            let crit = calc_crit_for(world, attacker, target, rng);
            if crit {
                side.crit += 1;
            }
            let Some(view) = world.stats(attacker) else {
                continue;
            };
            let mut patk = view.p_atk(Some(target));
            let multiplier = random_damage_multiplier(
                view.active_weapon_item().is_some(),
                view.calc(Stat::RandomDamage, 0.0) as i32,
                view.level(),
                rng,
            );
            if add_random {
                patk += multiplier;
            } else {
                patk *= multiplier;
            }
            side.patk += patk;
            side.pdef += world.stats(target).map_or(0.0, |t| t.p_def(Some(attacker)));
            if !missed {
                side.dmg += calc_phys_dam_for(world, attacker, target, shld, crit, false, rng);
                side.landed = true;
            }
        }
        side
    })
}

/// Java `Formulas.calcHpRegen(c) * 100000 / getRegeneratePeriod(c)` as an `int`: HP regained in
/// 100 seconds.
fn regen_per_100s(world: &World, id: ObjectId) -> i32 {
    let is_door = world.objects.get(&id).map(|e| e.kind) == Some(ObjectKind::Door);
    ((world.hp_regen_per_period(id) * 100_000.0) / f64::from(get_regenerate_period(is_door))) as i32
}

/// `calculateTimeBetweenAttacks` and `100000 / sAtk`: attacks per 100 seconds.
fn attacks_per_100s(world: &World, id: ObjectId) -> Result<i32, JavaException> {
    let p_atk_spd = world.stats(id).map_or(0.0, |s| s.p_atk_spd());
    let between = calculate_time_between_attacks(p_atk_spd);
    100_000_i32
        .checked_div(between)
        .ok_or_else(|| JavaException("java.lang.ArithmeticException: / by zero".to_owned()))
}

fn is_creature(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|e| e.kind.is_creature()) && world.stats(id).is_some()
}

/// `//fight_calculator_show [id1 id2]` and `//fcs`: the averages page. Java catches only the
/// `StringIndexOutOfBoundsException` of the `substring`, which cannot happen for a registered word.
fn admin_fight_calculator_show(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let params = command.split_once(' ').map_or("", |(_, rest)| rest).trim();
    let with_ids = !params.is_empty();
    let (npc1, npc2, ids) = if with_ids {
        let mut st = Tokens::spaces(params);
        let mid1 = parse_int(st.next_token().ok_or_else(no_such_element)?)?;
        let mid2 = parse_int(st.next_token().ok_or_else(no_such_element)?)?;
        let (Some(t1), Some(t2)) = (world.npc_world.template(mid1), world.npc_world.template(mid2)) else {
            // `new Monster(null)` fails.
            return Err(null_pointer());
        };
        let names = (t1.id, t1.name.clone(), t2.id, t2.name.clone());
        let (Some(a), Some(b)) = (
            world.spawn_npc_at(mid1, 0, 0, 0, 0),
            world.spawn_npc_at(mid2, 0, 0, 0, 0),
        ) else {
            return Err(null_pointer());
        };
        (a, b, Some(names))
    } else {
        let Some(target) = world.target_of(gm).filter(|t| is_creature(world, *t)) else {
            world.send(
                gm,
                &l2_protocol_game::server::SystemMessage::new(
                    l2_protocol_game::SystemMessageId::InvalidTarget,
                ),
            );
            return Ok(());
        };
        (gm, target, None)
    };

    let result = show(world, gm, npc1, npc2, ids.as_ref());
    if with_ids {
        world.delete_npc(npc1);
        world.delete_npc(npc2);
    }
    result
}

fn show(
    world: &mut World,
    gm: ObjectId,
    npc1: ObjectId,
    npc2: ObjectId,
    ids: Option<&(i32, String, i32, String)>,
) -> Outcome {
    // ATTACK speed in milliseconds, then the number of attacks per 100 seconds.
    let s_atk1 = attacks_per_100s(world, npc1)?;
    let s_atk2 = attacks_per_100s(world, npc2)?;

    let first = roll(world, npc1, npc2, true);
    let second = roll(world, npc2, npc1, false);
    // `npc.abortAttack()` after a landed hit.
    if first.landed {
        world.abort_attack(npc1);
    }
    if second.landed {
        world.abort_attack(npc2);
    }

    let n = f64::from(ROLLS);
    let (miss1, miss2) = (first.miss / 100, second.miss / 100);
    let (shld1, shld2) = (first.shld / 100, second.shld / 100);
    let (crit1, crit2) = (first.crit / 100, second.crit / 100);
    let (patk1, patk2) = (first.patk / n, second.patk / n);
    let (pdef1, pdef2) = (first.pdef / n, second.pdef / n);
    let (dmg1, dmg2) = (first.dmg / n, second.dmg / n);

    // Total damage per 100 seconds.
    let tdmg1 = (f64::from(s_atk1) * dmg1) as i32;
    let tdmg2 = (f64::from(s_atk2) * dmg2) as i32;

    // HP restored per 100 seconds.
    let max_hp = |id: ObjectId| world.stats(id).map_or(0.0, |s| f64::from(s.max_hp()));
    let (max_hp1, max_hp2) = (max_hp(npc1), max_hp(npc2));
    let hp1 = regen_per_100s(world, npc1);
    let hp2 = regen_per_100s(world, npc2);

    let mut html = String::from("<html><title>Selected mobs to fight</title><body><table>");
    match ids {
        None => {
            html.push_str("<tr><td width=140>Parameter</td><td width=70>me</td><td width=70>target</td></tr>")
        }
        Some((_, name1, _, name2)) => html.push_str(&format!(
            "<tr><td width=140>Parameter</td><td width=70>{name1}</td><td width=70>{name2}</td></tr>"
        )),
    }
    html.push_str(&format!(
        "<tr><td>miss</td><td>{miss1}%</td><td>{miss2}%</td></tr><tr><td>shld</td><td>{shld2}%</td><td>{shld1}%</td></tr><tr><td>crit</td><td>{crit1}%</td><td>{crit2}%</td></tr><tr><td>pAtk / pDef</td><td>{} / {}</td><td>{} / {}</td></tr><tr><td>made hits</td><td>{s_atk1}</td><td>{s_atk2}</td></tr><tr><td>dmg per hit</td><td>{}</td><td>{}</td></tr><tr><td>got dmg</td><td>{tdmg2}</td><td>{tdmg1}</td></tr><tr><td>got regen</td><td>{hp1}</td><td>{hp2}</td></tr><tr><td>had HP</td><td>{}</td><td>{}</td></tr><tr><td>die</td>",
        patk1 as i32,
        pdef1 as i32,
        patk2 as i32,
        pdef2 as i32,
        dmg1 as i32,
        dmg2 as i32,
        max_hp1 as i32,
        max_hp2 as i32,
    ));
    // `(tdmg2 - hp1) > 1` with Java's int arithmetic.
    let left1 = tdmg2.wrapping_sub(hp1);
    if left1 > 1 {
        html.push_str(&format!(
            "<td>{} sec</td>",
            ((100.0 * max_hp1) / f64::from(left1)) as i32
        ));
    } else {
        html.push_str("<td>never</td>");
    }
    let left2 = tdmg1.wrapping_sub(hp2);
    if left2 > 1 {
        html.push_str(&format!(
            "<td>{} sec</td>",
            ((100.0 * max_hp2) / f64::from(left2)) as i32
        ));
    } else {
        html.push_str("<td>never</td>");
    }
    html.push_str("</tr></table><center><br>");
    match ids {
        None => html.push_str(
            "<button value=\"Retry\" action=\"bypass -h admin_fight_calculator_show\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
        ),
        Some((id1, _, id2, _)) => html.push_str(&format!(
            "<button value=\"Retry\" action=\"bypass -h admin_fight_calculator_show {id1} {id2}\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">"
        )),
    }
    html.push_str("</center></body></html>");
    world.send_admin_html(gm, &AdminHtml::from_html(&html));
    Ok(())
}
