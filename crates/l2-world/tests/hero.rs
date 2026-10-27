#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Heroes (M8-HERO-01): the status at login, the hero skills, the hero voice, the diary and the
//! choice at the period end with `CycleDatabaseWrites` on and off. Expected behaviour is read off
//! `mechanics/olympiad/Hero.java`, `Player.setHero` and `ChatHeroVoice.java`.

mod party_scene;

use std::sync::Arc;

use l2_data::{ExperienceLoss, ExperienceTable, KarmaLoss, SkillTrees};
use l2_protocol_game::server::ChatType;
use l2_world::*;
use party_scene::*;

const SKILLS: &str = r#"<list>
<skill id="1374" levels="1" name="Heroic Valor"><operateType>P</operateType></skill>
<skill id="1375" levels="1" name="Heroic Grandeur"><operateType>P</operateType></skill>
</list>"#;
const TREE: &str = r#"<list><skillTree type="heroSkillTree">
<skill skillName="Heroic Valor" skillId="1374" skillLevel="1" getLevel="1" />
<skill skillName="Heroic Grandeur" skillId="1375" skillLevel="1" getLevel="1" />
</skillTree></list>"#;
const DUELIST: i32 = 88;

fn scene(cycle: bool) -> Scene {
    let mut s = Scene::with(Config {
        skills_xml: SKILLS.into(),
        ..Config::default()
    });
    let rows: String = EXP
        .iter()
        .enumerate()
        .map(|(i, exp)| format!(r#"<experience level="{}" tolevel="{exp}" />"#, i + 1))
        .collect();
    s.world.set_exp_data(
        Arc::new(
            ExperienceTable::from_xml(&format!(
                r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
            ))
            .unwrap(),
        ),
        Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
        Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
        Arc::new(SkillTrees::from_xml(TREE).unwrap()),
    );
    s.world.set_olympiad_settings(OlympiadSettings {
        cycle_database_writes: cycle,
        clock_override_unix_ms: Some(1_793_212_200_000),
        zone_offset_override_ms: Some(0),
        ..OlympiadSettings::default()
    });
    s
}

fn row(char_id: i32, name: &str, played: i32, claimed: bool) -> HeroRow {
    HeroRow {
        char_id,
        char_name: name.into(),
        class_id: DUELIST,
        count: 2,
        played,
        claimed,
        ..HeroRow::default()
    }
}

fn hero_writes(s: &Scene) -> Vec<HeroWrite> {
    s.persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::Olympiad(OlympiadWrite::Hero(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn skill_ids(s: &Scene, who: i32) -> Vec<i32> {
    let mut ids: Vec<i32> = s.player(who).core.skills.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    ids
}

#[test]
fn a_claimed_hero_logs_in_as_a_hero_with_the_skills() {
    let mut s = scene(false);
    s.world.load_heroes(HeroLoad {
        rows: vec![
            row(P1, "Alpha", 1, true),
            row(P2, "Beta", 1, false),
            row(P3, "Old", 0, true),
            row(P4, "Sub", 1, true),
        ],
        details: vec![],
    });
    assert!(s.world.is_hero_char(P1));
    assert!(s.world.is_unclaimed_hero(P2));
    assert!(!s.world.is_hero_char(P3) && !s.world.is_unclaimed_hero(P3));
    assert_ne!(s.world.hero_by_class(DUELIST), 0);
    assert_eq!(s.world.hero_list_packet().heroes.len(), 3);
    s.enter_data(player_data(P1, "Alpha", 40, (X, Y, Z)));
    s.enter_data(player_data(P2, "Beta", 40, (X, Y, Z)));
    assert!(s.world.is_hero(id(P1)));
    assert!(!s.world.is_hero(id(P2)));
    assert_eq!(skill_ids(&s, P1), [1374, 1375]);
    assert!(skill_ids(&s, P2).is_empty());
    assert!(l2_world::enter::build_user_info(&s.world, s.player(P1)).hero);
    assert!(!l2_world::enter::build_user_info(&s.world, s.player(P2)).hero);
    // A sub class gets the status but not the skills (`_baseClass == _activeClass`).
    let mut sub = player_data(P4, "Sub", 40, (X, Y, Z));
    sub.base_class = 1;
    s.enter_data(sub);
    assert!(s.world.is_hero(id(P4)));
    assert!(skill_ids(&s, P4).is_empty());
}

#[test]
fn set_hero_gives_and_takes_the_skills() {
    let mut s = scene(false);
    s.enter(P1, "Alpha");
    s.world.set_hero(id(P1), true);
    assert_eq!(skill_ids(&s, P1), [1374, 1375]);
    s.world.set_hero(id(P1), false);
    assert!(skill_ids(&s, P1).is_empty());
    assert!(!s.world.is_hero(id(P1)));
}

#[test]
fn the_hero_voice_is_for_heroes_and_gms() {
    let mut s = scene(false);
    s.world.load_heroes(HeroLoad {
        rows: vec![row(P1, "Alpha", 1, true)],
        details: vec![],
    });
    s.enter(P1, "Alpha");
    s.enter(P2, "Beta");
    let say = |s: &mut Scene, who: i32, text: &str| {
        s.input(
            who,
            ClientInput::Say {
                chat_type: ChatType::HeroVoice.client_id(),
                text: text.into(),
                target: None,
            },
        );
    };
    say(&mut s, P2, "hello");
    let sent = s.log.take();
    assert!(names(&sent, P1).is_empty());
    assert_eq!(messages(&sent, P2).len(), 1);
    say(&mut s, P1, "hello");
    let sent = s.log.take();
    for who in [P1, P2] {
        let says = of(&sent, who, "SAY2");
        assert_eq!(says.len(), 1);
        // opcode, sender id, then the chat type.
        assert_eq!(i32::from_le_bytes(says[0][5..9].try_into().unwrap()), 17);
    }
    // The flood protector: a second line at once is refused.
    say(&mut s, P1, "again");
    let sent = s.log.take();
    assert!(of(&sent, P2, "SAY2").is_empty());
}

#[test]
fn the_diary_rows_are_written_and_the_words_are_kept() {
    let mut s = scene(false);
    s.world.load_heroes(HeroLoad {
        rows: vec![row(P1, "Alpha", 1, true)],
        details: vec![HeroDetails {
            char_id: P1,
            message: Some("hi".into()),
            diary: vec![HeroDiaryRow {
                time: 1_793_212_200_000,
                action: ACTION_HERO_GAINED,
                param: 0,
            }],
            fights: vec![],
        }],
    });
    assert_eq!(s.world.hero_state().diary_len(P1), Some(1));
    assert_eq!(s.world.hero_state().message(P1), Some("hi"));
    s.world.hero_set_rb_killed(P1, 9001);
    s.world.hero_set_hero_gained(P1);
    // The rows are the hero's own deeds: written whatever the switch says (D-53).
    let writes = hero_writes(&s);
    assert!(matches!(writes[0], HeroWrite::Diary { char_id, action: 1, param: 9001, .. } if char_id == P1));
    assert!(matches!(writes[1], HeroWrite::Diary { action: 2, .. }));
    // The line joins the list of the hero (the monster 9001 is known).
    assert_eq!(s.world.hero_state().diary_len(P1), Some(2));
    // The hero words: only a hero, at most 300 characters.
    s.enter(P1, "Alpha");
    s.input(
        P1,
        ClientInput::Olympiad(OlympiadInput::WriteHeroWords("x".repeat(301))),
    );
    assert_eq!(s.world.hero_state().message(P1), Some("hi"));
    s.input(
        P1,
        ClientInput::Olympiad(OlympiadInput::WriteHeroWords("new".into())),
    );
    assert_eq!(s.world.hero_state().message(P1), Some("new"));
    // Only the words this server changed are written at shutdown with the switch off.
    s.persisted.0.lock().unwrap().clear();
    s.world.save_olympiad_status();
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    assert!(hero_writes(&s).contains(&HeroWrite::Message {
        char_id: P1,
        message: "new".into()
    }));
}

fn to_be(char_id: i32, name: &str) -> HeroToBe {
    HeroToBe {
        class_id: DUELIST,
        char_id,
        char_name: name.into(),
    }
}

#[test]
fn the_choice_with_the_switch_on_writes_the_heroes() {
    let mut s = scene(true);
    s.world.load_heroes(HeroLoad {
        rows: vec![row(P1, "Alpha", 1, true)],
        details: vec![],
    });
    s.enter(P1, "Alpha");
    assert!(s.world.is_hero(id(P1)));
    s.persisted.0.lock().unwrap().clear();
    s.world
        .olympiad_compute_new_heroes(&[to_be(P2, "Beta"), to_be(P1, "Alpha")]);
    // The old hero lost the status and the skills; both are unclaimed heroes now.
    assert!(!s.world.is_hero(id(P1)));
    assert!(skill_ids(&s, P1).is_empty());
    assert!(s.world.is_unclaimed_hero(P1) && s.world.is_unclaimed_hero(P2));
    let heroes = s.world.heroes();
    assert_eq!(heroes.len(), 2);
    // A known hero counts one more (2 + 1), a new one starts at 1.
    assert_eq!(heroes.iter().find(|(i, _)| *i == P1).unwrap().1.count, 3);
    assert_eq!(heroes.iter().find(|(i, _)| *i == P2).unwrap().1.count, 1);
    let writes = hero_writes(&s);
    assert_eq!(writes[0], HeroWrite::ResetPlayed);
    assert_eq!(writes[1], HeroWrite::DeleteItems);
    assert!(writes.contains(&HeroWrite::Insert {
        char_id: P2,
        class_id: DUELIST,
        count: 1,
        played: 1,
        claimed: false
    }));
    assert!(writes.contains(&HeroWrite::Update {
        char_id: P1,
        class_id: DUELIST,
        count: 3,
        played: 1,
        claimed: false
    }));
    // The claim: the status, the skills, the diary row and the read of the details.
    s.persisted.0.lock().unwrap().clear();
    s.world.claim_hero(id(P1));
    assert!(s.world.is_hero(id(P1)) && s.world.is_hero_char(P1));
    assert_eq!(skill_ids(&s, P1), [1374, 1375]);
    assert!(
        hero_writes(&s)
            .iter()
            .any(|w| matches!(w, HeroWrite::Diary { action: 2, .. }))
    );
    assert!(s.persisted.0.lock().unwrap().iter().any(|c| matches!(
        c,
        PersistCommand::OlympiadRereadRequest {
            request: OlympiadRereadRequest::Heroes { only: Some(p), .. },
            ..
        } if *p == P1
    )));
}

#[test]
fn the_choice_with_the_switch_off_stays_in_memory_and_a_read_keeps_the_claim() {
    let mut s = scene(false);
    s.enter(P1, "Alpha");
    s.world.olympiad_compute_new_heroes(&[to_be(P1, "Alpha")]);
    assert!(s.world.is_unclaimed_hero(P1));
    assert!(hero_writes(&s).is_empty());
    s.world.claim_hero(id(P1));
    // Only the player's own diary row goes out; no `heroes` row.
    assert!(
        hero_writes(&s)
            .iter()
            .all(|w| matches!(w, HeroWrite::Diary { .. }))
    );
    let then = OlympiadRereadThen {
        player: id(P1),
        npc: id(0),
        action: OlympiadRereadAction::Hero(HeroAction::List),
    };
    // A read of the table the Java game server keeps (unclaimed, same count) does not undo it.
    s.world.handle(WorldCommand::OlympiadReread {
        result: Ok(OlympiadRereadRows::Heroes(Box::new(HeroLoad {
            rows: vec![HeroRow {
                count: 1,
                ..row(P1, "Alpha", 1, false)
            }],
            details: vec![],
        }))),
        then,
    });
    assert!(s.world.is_hero_char(P1));
    // A failed read changes nothing.
    s.world.handle(WorldCommand::OlympiadReread {
        result: Err(()),
        then,
    });
    assert!(s.world.is_hero_char(P1));
}
