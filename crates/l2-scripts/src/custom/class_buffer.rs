//! The class buffer: the buffs, songs and dances that fit a class, in two ways.
//!
//! - **The NPC** next to every town gatekeeper gives them for free.
//! - **The voiced command `.buffme`** opens the same pages anywhere and charges the player's level
//!   times 1,000 Adena each time a group is given.
//!
//! Own feature (not from the Java datapack). The player picks a group on the page: the one that
//! fits the class is offered first, and any other can be looked at and taken. A pet or servitor
//! gets the fighting buffs with it. The groups are in [`class_buffer_data`](super::class_buffer_data).
//!
//! The two ways share the pages and the events. The paid way puts `p-` before the event, and its
//! links are `voice` bypasses (`.buffme show tank`), which, unlike a `Script` link, reach the
//! script with no NPC next to the player.

use l2_util::ObjectId;
use l2_world::script::class_enum_name;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::class_buffer_data::{Buff, GROUPS, Group, PET_BUFFS, PET_SONGS, group_by_key, group_of_class};

/// `data/stats/npcs/50000-50099.xml`; spawned by `data/spawns/Others/ClassBuffer.xml`.
pub const NPC_ID: i32 = 50011;
/// How far from the buffer a player may stand when he asks: the target must be a class buffer in
/// talking range, so a link that is kept open cannot be used from the other end of the world.
const MAX_DISTANCE: f64 = 400.0;
/// What `.buffme` costs for each level of the player, in Adena.
const FEE_PER_LEVEL: i64 = 1_000;

pub(crate) const BUTTON: &str =
    "width=200 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"";

/// Who asks: the NPC's page (free) or `.buffme` (paid).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Npc,
    Command,
}

impl Mode {
    /// The bypass of a click on a page.
    fn action(self, click: &str) -> String {
        match self {
            Mode::Npc => format!("bypass -h Script ClassBuffer {click}"),
            Mode::Command => format!("bypass -h voice .buffme {}", click.replace(':', " ")),
        }
    }

    /// What a group costs a player of `level`.
    fn fee(self, level: i32) -> i64 {
        match self {
            Mode::Npc => 0,
            Mode::Command => fee_for_level(level),
        }
    }
}

/// The price of `.buffme` for a player of `level`: 1,000 Adena for each level.
fn fee_for_level(level: i32) -> i64 {
    i64::from(level.max(0)) * FEE_PER_LEVEL
}

/// `80000` as `80,000`.
pub(crate) fn grouped(value: i64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `PHOENIX_KNIGHT` to `Phoenix Knight`.
fn class_name(class_id: i32) -> String {
    let name = class_enum_name(class_id).unwrap_or("Adventurer");
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn button(mode: Mode, label: &str, click: &str) -> String {
    format!(
        "<button value=\"{label}\" action=\"{}\" {BUTTON}><br>",
        mode.action(click)
    )
}

/// The label of a button that gives a group: with the price when there is one.
fn buff_label(mode: Mode, title: &str, level: i32) -> String {
    match mode.fee(level) {
        0 => format!("Buff me: {title}"),
        fee => format!("Buff me: {title} ({} Adena)", grouped(fee)),
    }
}

fn names(buffs: &[Buff]) -> String {
    buffs.iter().map(|b| b.name).collect::<Vec<_>>().join(", ")
}

/// The first page: the class, the group that fits it, and the other groups.
fn main_page(mode: Mode, class_id: i32, level: i32) -> String {
    let mine = group_of_class(class_id);
    let mut html = String::from("<html><body><center><font color=\"LEVEL\">Class Buffer</font></center><br>");
    html.push_str(&format!(
        "Welcome, %playername%.<br1>Your class is <font color=\"LEVEL\">{}</font>, so the \
         <font color=\"LEVEL\">{}</font> buffs fit you best.<br>",
        class_name(class_id),
        mine.title
    ));
    if mode == Mode::Command {
        html.push_str(&format!(
            "Each time costs <font color=\"LEVEL\">{} Adena</font> ({} for every level).<br>",
            grouped(mode.fee(level)),
            grouped(FEE_PER_LEVEL)
        ));
    }
    html.push_str("<center>");
    html.push_str(&button(
        mode,
        &buff_label(mode, mine.title, level),
        &format!("buff:{}", mine.key),
    ));
    html.push_str(&button(
        mode,
        &format!("What is in {}?", mine.title),
        &format!("show:{}", mine.key),
    ));
    html.push_str("<br><font color=\"LEVEL\">Other groups</font><br>");
    for group in GROUPS.iter().filter(|g| g.key != mine.key) {
        html.push_str(&button(mode, group.title, &format!("show:{}", group.key)));
    }
    html.push_str("</center><br1>Your pet or servitor gets the fighting buffs with them.</body></html>");
    html
}

/// What a group holds, with the button that gives it.
fn group_page(mode: Mode, group: &Group, level: i32) -> String {
    let mut html = format!(
        "<html><body><center><font color=\"LEVEL\">{}</font></center><br>{}<br><br>\
         <font color=\"LEVEL\">Buffs</font><br1>{}<br><br>\
         <font color=\"LEVEL\">Songs and dances</font><br1>{}<br><br><center>",
        group.title,
        group.about,
        names(group.buffs),
        names(group.songs)
    );
    html.push_str(&button(
        mode,
        &buff_label(mode, group.title, level),
        &format!("buff:{}", group.key),
    ));
    html.push_str(&button(mode, "Back", "main"));
    html.push_str("</center></body></html>");
    html
}

/// The class buffer the player stands at: his target, when it is one and close enough.
fn buffer_near(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<ScriptNpc> {
    let npc = ctx.script_npc(ctx.target_of(player)?)?;
    (npc.id == NPC_ID && ctx.distance_3d(player, npc.object_id)? <= MAX_DISTANCE).then_some(npc)
}

/// What `.buffme` does once the fee is paid, for any creature: the buffs, songs and dances of
/// `group` on `target`, cast by `effector`. The hired companions buff themselves with it.
pub(crate) fn buff_target(ctx: &mut ScriptCtx<'_>, effector: ObjectId, target: ObjectId, group: &Group) {
    give(ctx, effector, target, group.all());
}

fn give(
    ctx: &mut ScriptCtx<'_>,
    effector: ObjectId,
    target: ObjectId,
    buffs: impl Iterator<Item = &'static Buff>,
) {
    for buff in buffs {
        ctx.world_mut()
            .apply_effects(effector, target, SkillKey::new(buff.id, buff.level));
    }
}

/// `p-show tank` and `show:tank`: the mode, the command and the key of a click. A click of
/// `.buffme` comes with its words split by a space.
fn parse(event: &str) -> (Mode, &str, &str) {
    let (mode, click) = match event.strip_prefix("p-") {
        Some(click) => (Mode::Command, click),
        None => (Mode::Npc, event),
    };
    let (command, key) = click.split_once([':', ' ']).unwrap_or((click, ""));
    (mode, command, key.trim())
}

/// The class buffer.
pub struct ClassBuffer;

impl Script for ClassBuffer {
    fn name(&self) -> &'static str {
        "ClassBuffer"
    }

    fn path(&self) -> &'static str {
        "custom/ClassBuffer"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NPC_ID]);
        s.add_talk_id(&[NPC_ID]);
        s.add_first_talk_id(&[NPC_ID]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(main_page(
            Mode::Npc,
            ctx.player_class_id(player),
            ctx.player_level(player),
        ))
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let class_id = ctx.player_class_id(player);
        let level = ctx.player_level(player);
        let (mode, command, key) = parse(event);
        match command {
            "main" => Some(main_page(mode, class_id, level)),
            "show" => group_by_key(key).map(|group| group_page(mode, group, level)),
            "buff" => {
                let group = group_by_key(key)?;
                if ctx.is_dead(player) {
                    return None;
                }
                // Who casts: the NPC, or the player himself when he asked by command.
                let effector = match mode {
                    Mode::Npc => {
                        let Some(buffer) = buffer_near(ctx, player) else {
                            ctx.send_message(player, "Come closer to the Class Buffer and select it first.");
                            return None;
                        };
                        buffer.object_id
                    }
                    Mode::Command => {
                        if ctx.world().is_in_olympiad_mode(player) {
                            ctx.send_message(player, "You cannot be buffed during the Olympiad.");
                            return None;
                        }
                        let fee = mode.fee(level);
                        // Too little Adena: the player gets the game's own "not enough adena".
                        if !ctx.world_mut().reduce_adena(player, fee, true) {
                            return None;
                        }
                        player
                    }
                };
                give(ctx, effector, player, group.all());
                if let Some(summon) = ctx.summon_of(player) {
                    give(ctx, effector, summon, PET_BUFFS.iter().chain(PET_SONGS));
                }
                let paid = match mode.fee(level) {
                    0 => String::new(),
                    fee => format!(" for {} Adena", grouped(fee)),
                };
                ctx.send_message(player, &format!("{} buffs given{paid}.", group.title));
                Some(main_page(mode, class_id, level))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::path::Path;

    use l2_data::{ClassId, SkillLoadOptions, SkillTemplates};
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::custom::class_buffer_data::{GROUPS, PET_BUFFS, PET_SONGS};
    use crate::testing::{ScriptTest, datapack, pages_to, skill_xml};

    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const NPCS: &str = r#"<npc id="50011" type="Folk" level="70" name="Class Buffer"/>
        <npc id="50012" type="Folk" level="70" name="Somebody Else"/>
        <npc id="14000" type="Servitor" level="70" name="Servitor"/>"#;

    fn all_ids() -> BTreeSet<i32> {
        GROUPS
            .iter()
            .flat_map(|g| g.all())
            .chain(PET_BUFFS)
            .chain(PET_SONGS)
            .map(|b| b.id)
            .collect()
    }

    #[test]
    fn every_class_is_in_exactly_one_group() {
        let mut seen: HashMap<u16, &str> = HashMap::new();
        for group in &GROUPS {
            for class in group.classes {
                assert!(
                    seen.insert(*class, group.key).is_none(),
                    "class {class} is in two groups"
                );
            }
        }
        for id in 0..=200u16 {
            assert_eq!(
                ClassId(id).is_player_class(),
                seen.contains_key(&id),
                "class {id}: the player classes and the groups differ"
            );
        }
    }

    #[test]
    fn the_group_of_a_class_follows_its_job() {
        for (class, key) in [
            (player_class::FIGHTER, "warrior"),
            (player_class::DUELIST, "warrior"),
            (player_class::PHOENIX_KNIGHT, "tank"),
            (player_class::EVA_TEMPLAR, "tank"),
            (player_class::ADVENTURER, "dagger"),
            (player_class::GHOST_HUNTER, "dagger"),
            (player_class::SAGITTARIUS, "archer"),
            (player_class::ARCHMAGE, "mage"),
            (player_class::ARCANA_LORD, "mage"),
            (player_class::DOMINATOR, "mage"),
            (player_class::CARDINAL, "healer"),
            (player_class::SHILLIEN_SAINT, "healer"),
        ] {
            assert_eq!(group_of_class(class).key, key, "class {class}");
        }
        // A class that is not a player class follows the server's fighter/mage split.
        assert_eq!(group_of_class(-1).key, "warrior");
        assert_eq!(group_of_class(5000).key, "warrior");
    }

    #[test]
    fn class_names_read_like_the_game() {
        assert_eq!(class_name(player_class::PHOENIX_KNIGHT), "Phoenix Knight");
        assert_eq!(class_name(player_class::ARCHMAGE), "Archmage");
    }

    #[test]
    fn every_skill_of_every_group_exists_at_the_level_given() {
        let Some(root) = datapack() else {
            eprintln!("skipped: no datapack");
            return;
        };
        let skills = SkillTemplates::load(root, &SkillLoadOptions::default());
        for group in &GROUPS {
            for buff in group.all().chain(PET_BUFFS).chain(PET_SONGS) {
                let skill = skills
                    .get(buff.id, buff.level)
                    .unwrap_or_else(|| panic!("{}: skill {} level {}", group.key, buff.id, buff.level));
                assert!(
                    skill.name.eq_ignore_ascii_case(buff.name) || !buff.name.is_empty(),
                    "{}",
                    buff.id
                );
                // The highest level there is: a lower one would be a weaker buff for nothing.
                assert_eq!(
                    skills.max_level(buff.id),
                    buff.level,
                    "{}: {}",
                    group.key,
                    buff.name
                );
            }
        }
    }

    #[test]
    fn a_group_fits_the_buff_slots_and_holds_no_two_of_a_kind() {
        let Some(root) = datapack() else {
            eprintln!("skipped: no datapack");
            return;
        };
        let skills = SkillTemplates::load(root, &SkillLoadOptions::default());
        let lists = GROUPS
            .iter()
            .map(|g| (g.key, g.buffs, g.songs))
            .chain([("pet", PET_BUFFS, PET_SONGS)]);
        for (key, buffs, songs) in lists {
            // `Player.ini`: MaxBuffAmount and MaxDanceAmount.
            assert!(buffs.len() <= 20, "{key}: {} buffs", buffs.len());
            assert!(songs.len() <= 12, "{key}: {} songs and dances", songs.len());
            let mut kinds = BTreeSet::new();
            for buff in buffs.iter().chain(songs) {
                let skill = skills.get(buff.id, buff.level).unwrap();
                assert_eq!(skill.is_dance(), songs.contains(buff), "{key}: {}", buff.name);
                let kind = format!("{:?}", skill.abnormal_type);
                assert!(
                    kind.contains("None") || kinds.insert(kind.clone()),
                    "{key}: {} has the same abnormal type ({kind}) as another skill",
                    buff.name
                );
            }
        }
    }

    /// The shipped data: the NPC exists, and one stands next to each town gatekeeper.
    #[test]
    fn a_class_buffer_stands_next_to_every_town_gatekeeper() {
        let Some(root) = datapack() else {
            eprintln!("skipped: no datapack");
            return;
        };
        let npcs = l2_data::NpcTemplates::load(root, &l2_data::NpcLoadOptions::default());
        let template = npcs
            .get(NPC_ID)
            .expect("data/stats/npcs/50000-50099.xml has the class buffer");
        assert_eq!(template.npc_type, "Folk");
        let spawns = l2_data::SpawnData::load(root, &npcs, &l2_data::SpawnLoadOptions::default());

        // The gatekeepers are the NPCs with a file in `data/teleporters/town`.
        let gatekeepers: Vec<i32> = std::fs::read_dir(root.join("data/teleporters/town"))
            .unwrap()
            .flatten()
            .filter_map(|e| e.path().file_stem()?.to_str()?.parse().ok())
            .collect();
        assert_eq!(gatekeepers.len(), 21);
        let buffers: Vec<_> = spawns.spawns_of(NPC_ID).collect();
        assert_eq!(buffers.len(), gatekeepers.len());
        for gatekeeper in gatekeepers {
            let spot = spawns
                .spawns_of(gatekeeper)
                .next()
                .expect("the gatekeeper stands somewhere")
                .location;
            let nearest = buffers
                .iter()
                .map(|b| {
                    let (dx, dy) = (f64::from(b.location.x - spot.x), f64::from(b.location.y - spot.y));
                    (dx * dx + dy * dy).sqrt()
                })
                .fold(f64::MAX, f64::min);
            assert!(
                nearest < 120.0,
                "gatekeeper {gatekeeper}: the nearest buffer is {nearest:.0} away"
            );
        }
    }

    fn scene(root: &Path) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ClassBuffer);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.set_skills(ITEMS, &skill_xml(root, &all_ids()));
        let buffer = t.spawn(NPC_ID, 20);
        (t, buffer)
    }

    fn buffed(t: &ScriptTest, who: i32, ids: impl Iterator<Item = i32>) -> Vec<i32> {
        ids.filter(|id| t.world.script_buff_skill_level(ObjectId(who), *id).is_none())
            .collect()
    }

    #[test]
    fn the_talk_page_names_the_class_and_offers_its_group() {
        let Some(root) = datapack() else { return };
        let (mut t, buffer) = scene(root);
        t.enter_as(
            1,
            80,
            l2_world::script::race::HUMAN,
            player_class::PHOENIX_KNIGHT,
            vec![],
        );
        t.talk(1, buffer);
        let page = pages_to(&t.take(), 1).pop().unwrap();
        assert!(page.contains("Phoenix Knight"), "{page}");
        assert!(page.contains("buff:tank"));
        assert!(page.contains("show:warrior") && page.contains("show:healer"));
        assert_eq!(
            page.matches("show:tank").count(),
            1,
            "the own group is not listed twice"
        );
        assert!(page.len() < 4000);
    }

    #[test]
    fn a_group_page_lists_its_buffs_and_every_page_is_short_enough() {
        let Some(root) = datapack() else { return };
        let (mut t, buffer) = scene(root);
        t.enter_as(
            1,
            80,
            l2_world::script::race::HUMAN,
            player_class::DUELIST,
            vec![],
        );
        t.talk(1, buffer);
        t.take();
        for group in &GROUPS {
            // Every group is linked from the first page; "Back" leads there.
            t.event(1, "ClassBuffer", "main");
            let page = t.event(1, "ClassBuffer", &format!("show:{}", group.key));
            for buff in group.all() {
                assert!(page.contains(buff.name), "{}: {}", group.key, buff.name);
            }
            assert!(page.contains(&format!("buff:{}", group.key)));
            assert!(page.len() < 6000, "{}: {} characters", group.key, page.len());
        }
        assert_eq!(t.event(1, "ClassBuffer", "show:nonsense"), "");
    }

    #[test]
    fn a_group_lands_on_the_player_with_every_skill_and_the_pet_gets_its_own() {
        let Some(root) = datapack() else { return };
        for group in &GROUPS {
            let (mut t, buffer) = scene(root);
            t.enter_as(
                1,
                80,
                l2_world::script::race::HUMAN,
                player_class::DUELIST,
                vec![],
            );
            let pet = t.summon(1, 14000);
            t.talk(1, buffer);
            t.take();
            // As a player does: look at the group, then take it from that page.
            t.event(1, "ClassBuffer", &format!("show:{}", group.key));
            t.event(1, "ClassBuffer", &format!("buff:{}", group.key));
            let missing = buffed(&t, 1, group.all().map(|b| b.id));
            assert!(
                missing.is_empty(),
                "{}: not on the player: {missing:?}",
                group.key
            );
            for buff in group.all() {
                assert_eq!(
                    t.world.script_buff_skill_level(ObjectId(1), buff.id),
                    Some(buff.level),
                    "{}: {}",
                    group.key,
                    buff.name
                );
            }
            for buff in PET_BUFFS.iter().chain(PET_SONGS) {
                assert_eq!(
                    t.world.script_buff_skill_level(pet, buff.id),
                    Some(buff.level),
                    "pet: {}",
                    buff.name
                );
            }
        }
    }

    #[test]
    fn nothing_is_given_away_from_the_buffer() {
        let Some(root) = datapack() else { return };
        let (mut t, buffer) = scene(root);
        let other = t.spawn(50012, 40);
        t.enter_as(
            1,
            80,
            l2_world::script::race::HUMAN,
            player_class::DUELIST,
            vec![],
        );
        t.talk(1, buffer);
        t.event(1, "ClassBuffer", "show:warrior");
        let might = || vec![1068];
        // The page is open, but the player's target is another NPC now.
        t.world.set_target(ObjectId(1), Some(other));
        t.event(1, "ClassBuffer", "buff:warrior");
        assert_eq!(buffed(&t, 1, [1068].into_iter()), might());
        // No target at all.
        t.world.set_target(ObjectId(1), None);
        t.event(1, "ClassBuffer", "buff:warrior");
        assert_eq!(buffed(&t, 1, [1068].into_iter()), might());
        // The buffer is the target, but far away.
        t.world.set_target(ObjectId(1), Some(buffer));
        t.world.set_position(
            ObjectId(1),
            crate::testing::X + 2_000,
            crate::testing::Y,
            crate::testing::Z,
        );
        t.event(1, "ClassBuffer", "buff:warrior");
        assert_eq!(buffed(&t, 1, [1068].into_iter()), might());
        // Back next to it: now it works.
        t.world.set_position(
            ObjectId(1),
            crate::testing::X,
            crate::testing::Y,
            crate::testing::Z,
        );
        t.event(1, "ClassBuffer", "buff:warrior");
        assert!(buffed(&t, 1, [1068].into_iter()).is_empty());
    }

    // ---- `.buffme`: the same pages anywhere, for level x 1,000 Adena ----

    fn say(t: &mut ScriptTest, text: &str) -> String {
        t.input(
            1,
            l2_world::ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        );
        pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    /// A player of `level` with `adena`, alone in the world: no buffer anywhere near.
    fn paid_scene(root: &Path, level: i32, adena: i64) -> ScriptTest {
        let (mut t, _buffer) = scene(root);
        t.enter_as(
            1,
            level,
            l2_world::script::race::HUMAN,
            player_class::DUELIST,
            vec![crate::testing::item(100, 57, adena)],
        );
        t.take();
        t
    }

    #[test]
    fn the_fee_is_the_level_times_one_thousand() {
        assert_eq!(fee_for_level(1), 1_000);
        assert_eq!(fee_for_level(20), 20_000);
        assert_eq!(fee_for_level(80), 80_000);
        assert_eq!(fee_for_level(85), 85_000);
        assert_eq!(fee_for_level(-3), 0);
        assert_eq!(Mode::Npc.fee(80), 0);
        assert_eq!(Mode::Command.fee(80), 80_000);
    }

    #[test]
    fn adena_is_written_with_separators() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(80_000), "80,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }

    #[test]
    fn a_click_is_read_from_the_page_and_from_the_command() {
        assert_eq!(parse("show:tank"), (Mode::Npc, "show", "tank"));
        assert_eq!(parse("main"), (Mode::Npc, "main", ""));
        assert_eq!(parse("p-buff tank"), (Mode::Command, "buff", "tank"));
        assert_eq!(parse("p-show  mage"), (Mode::Command, "show", "mage"));
        assert_eq!(parse("p-main"), (Mode::Command, "main", ""));
    }

    #[test]
    fn buffme_opens_the_page_with_the_price_and_links_that_need_no_npc() {
        let Some(root) = datapack() else { return };
        let mut t = paid_scene(root, 80, 0);
        let page = say(&mut t, ".buffme");
        assert!(page.contains("80,000 Adena"), "{page}");
        assert!(page.contains("Duelist"));
        assert!(page.contains("bypass -h voice .buffme buff warrior"));
        assert!(page.contains("bypass -h voice .buffme show tank"));
        assert!(
            !page.contains("Script ClassBuffer"),
            "a Script link needs an NPC next to the player"
        );
        assert!(page.len() < 4000);
        // The same pages from a click on them.
        let group = say(&mut t, ".buffme show tank");
        assert!(group.contains("bypass -h voice .buffme buff tank"), "{group}");
        assert!(group.contains("(80,000 Adena)"));
    }

    #[test]
    fn buffme_takes_level_times_one_thousand_and_gives_the_group() {
        let Some(root) = datapack() else { return };
        for group in &GROUPS {
            let mut t = paid_scene(root, 80, 200_000);
            let pet = t.summon(1, 14000);
            say(&mut t, ".buffme");
            // As the page does: the click is a `voice` bypass.
            t.bypass(1, &format!("voice .buffme show {}", group.key));
            t.take();
            t.bypass(1, &format!("voice .buffme buff {}", group.key));
            assert_eq!(t.count(1, 57), 120_000, "{}", group.key);
            for buff in group.all() {
                assert_eq!(
                    t.world.script_buff_skill_level(ObjectId(1), buff.id),
                    Some(buff.level),
                    "{}: {}",
                    group.key,
                    buff.name
                );
            }
            for buff in PET_BUFFS.iter().chain(PET_SONGS) {
                assert_eq!(
                    t.world.script_buff_skill_level(pet, buff.id),
                    Some(buff.level),
                    "pet: {}",
                    buff.name
                );
            }
        }
    }

    #[test]
    fn the_typed_command_works_without_a_page_and_the_fee_follows_the_level() {
        let Some(root) = datapack() else { return };
        let mut t = paid_scene(root, 20, 25_000);
        say(&mut t, ".buffme buff warrior");
        assert_eq!(t.count(1, 57), 5_000, "20 x 1,000");
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), Some(3));
        // The rest is not enough for a second time.
        say(&mut t, ".buffme buff mage");
        assert_eq!(t.count(1, 57), 5_000);
        assert_eq!(
            t.world.script_buff_skill_level(ObjectId(1), 1059),
            None,
            "Empower of the mage group"
        );
    }

    #[test]
    fn without_enough_adena_nothing_is_given_or_taken() {
        let Some(root) = datapack() else { return };
        let mut t = paid_scene(root, 80, 79_999);
        say(&mut t, ".buffme buff warrior");
        assert_eq!(t.count(1, 57), 79_999);
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), None);
        // With 80,000 it goes through.
        let mut t = paid_scene(root, 80, 80_000);
        say(&mut t, ".buffme buff warrior");
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), Some(3));
    }

    #[test]
    fn a_dead_player_pays_nothing_and_gets_nothing() {
        let Some(root) = datapack() else { return };
        let mut t = paid_scene(root, 80, 200_000);
        t.world.do_die(ObjectId(1), None);
        t.take();
        say(&mut t, ".buffme buff warrior");
        assert_eq!(t.count(1, 57), 200_000);
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), None);
    }

    #[test]
    fn an_unknown_group_or_click_costs_nothing() {
        let Some(root) = datapack() else { return };
        let mut t = paid_scene(root, 80, 200_000);
        say(&mut t, ".buffme buff nonsense");
        say(&mut t, ".buffme nonsense");
        say(&mut t, ".buffme buff");
        assert_eq!(t.count(1, 57), 200_000);
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), None);
    }

    #[test]
    fn the_npc_stays_free() {
        let Some(root) = datapack() else { return };
        let (mut t, buffer) = scene(root);
        t.enter_as(
            1,
            80,
            l2_world::script::race::HUMAN,
            player_class::DUELIST,
            vec![crate::testing::item(100, 57, 500)],
        );
        t.talk(1, buffer);
        t.event(1, "ClassBuffer", "show:warrior");
        t.event(1, "ClassBuffer", "buff:warrior");
        assert_eq!(t.count(1, 57), 500);
        assert_eq!(t.world.script_buff_skill_level(ObjectId(1), 1068), Some(3));
    }
}
