//! Tests of the XML forms against the real `dist/game/config` files (skipped when `dist/` is not
//! there) and against the loaders of the servers: what the forms write must still be read.

use std::path::{Path, PathBuf};

use super::model::Row;
use super::*;

fn dist_config() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/game/config");
    if dir.join("AccessLevels.xml").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no dist/game/config");
        None
    }
}

/// A run folder in a temp dir with a copy of the dist XML configs.
fn run_folder(dist: &Path) -> tempfile::TempDir {
    let run = tempfile::tempdir().unwrap();
    let config = run.path().join(CONFIG_SUBDIR);
    std::fs::create_dir_all(&config).unwrap();
    for topic in TopicId::ALL {
        let from = dist.join(topic.file());
        if from.is_file() {
            std::fs::copy(&from, config.join(topic.file())).unwrap();
        }
    }
    run
}

fn load(dist: &Path, topic: TopicId) -> XmlFile {
    XmlFile::load(topic, &dist.join(topic.file())).unwrap()
}

/// (lines removed, lines added) between two texts, comparing whole lines and ignoring the
/// common start and end.
fn changed_lines(before: &str, after: &str) -> (usize, usize) {
    let a: Vec<&str> = before.split('\n').collect();
    let b: Vec<&str> = after.split('\n').collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (a.len() - prefix - suffix, b.len() - prefix - suffix)
}

#[test]
fn changed_lines_helper() {
    assert_eq!(changed_lines("a\nb\nc", "a\nb\nc"), (0, 0));
    assert_eq!(changed_lines("a\nb\nc", "a\nX\nc"), (1, 1));
    assert_eq!(changed_lines("a\nc", "a\nb\nc"), (0, 1));
    assert_eq!(changed_lines("a\nb\nc", "a\nc"), (1, 0));
}

#[test]
fn every_dist_xml_config_round_trips_byte_identical() {
    let Some(dist) = dist_config() else { return };
    let mut count = 0;
    for topic in TopicId::ALL {
        if !dist.join(topic.file()).is_file() {
            assert!(topic.optional(), "{} is missing from dist", topic.file());
            continue;
        }
        let file = load(&dist, topic);
        let again = render(file.form.as_form(), &file.original).unwrap();
        assert_eq!(again, file.original, "{}", topic.file());
        assert!(!file.is_dirty(), "{}", topic.file());
        assert!(
            !has_errors(&file.issues()),
            "{}: {:?}",
            topic.file(),
            file.issues()
        );
        count += 1;
    }
    assert_eq!(
        count, 9,
        "AccessLevels, AdminCommands, ClassMaster, DynamicExpRates, SiegeSchedule, BotReportPunishments, SecondaryAuth, Scripts, default-ipconfig"
    );
}

#[test]
fn dist_files_keep_their_line_endings_and_comments() {
    let Some(dist) = dist_config() else { return };
    for topic in [TopicId::AccessLevels, TopicId::BotReport, TopicId::SecondaryAuth] {
        let mut file = load(&dist, topic);
        // One edit in each, then the other lines are the same bytes (CRLF included).
        match &mut file.form {
            Form::AccessLevels(f) => f.rows_mut()[1].set("name", "Player"),
            Form::BotReport(f) => f.rows_mut()[0].set("skillLevel", "2"),
            Form::SecondaryAuth(f) => f.ban_time.value = "60".into(),
            _ => unreachable!(),
        }
        let out = render(file.form.as_form(), &file.original).unwrap();
        assert_eq!(changed_lines(&file.original, &out), (1, 1), "{}", topic.file());
        assert_eq!(out.matches("\r\n").count(), file.original.matches("\r\n").count());
        assert_eq!(out.matches("<!--").count(), file.original.matches("<!--").count());
    }
}

/// One change in each form, and the number of lines it touches.
#[test]
fn one_change_per_form_touches_one_line() {
    let Some(dist) = dist_config() else { return };
    for topic in TopicId::ALL {
        if topic == TopicId::IpConfig || !dist.join(topic.file()).is_file() {
            continue;
        }
        let mut file = load(&dist, topic);
        match &mut file.form {
            Form::AccessLevels(f) => f.rows_mut()[2].set_flag("allowAltg", true),
            Form::AdminCommands(f) => f.rows_mut()[0].set("accessLevel", "100"),
            Form::ClassMaster(f) => f.settings_mut().set_flag("classChangeEnabled", true),
            Form::DynamicExp(f) => f.rows_mut()[84].set("exp", "2"),
            Form::Siege(f) => f.set_day(3, 5),
            Form::BotReport(f) => f.rows_mut()[2].set("skillId", "6099"),
            Form::SecondaryAuth(f) => f.max_attempts.value = "9".into(),
            Form::Scripts(_) => {
                assert!(!file.is_dirty());
                continue;
            }
            Form::IpConfig(f) => f.defines_mut()[1].set("address", "10.0.0.1"),
        }
        assert!(file.is_dirty(), "{}", topic.file());
        let out = render(file.form.as_form(), &file.original).unwrap();
        assert_eq!(changed_lines(&file.original, &out), (1, 1), "{}", topic.file());
        XmlDoc::parse(&out).unwrap();
    }
}

#[test]
fn deleting_a_row_takes_only_its_line() {
    let Some(dist) = dist_config() else { return };
    let mut file = load(&dist, TopicId::DynamicExp);
    let Form::DynamicExp(form) = &mut file.form else {
        unreachable!()
    };
    form.rows_mut().remove(10);
    let out = render(file.form.as_form(), &file.original).unwrap();
    assert_eq!(changed_lines(&file.original, &out), (1, 0));
    assert!(!out.contains("level=\"11\""));
}

// ---- the servers' loaders read what the forms write ------------------------------------------

#[test]
fn the_access_levels_loader_reads_the_written_files() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let dir = run.path().join(CONFIG_SUBDIR);
    let before = game_server::world_data::AccessLevels::load(&dir);
    assert!(!before.allows_alt_g(0));
    assert!(before.rights().has_access("admin_admin", 30));

    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    assert!(view.errors().is_empty(), "{:?}", view.errors());
    for file in view.files_mut() {
        match &mut file.form {
            Form::AccessLevels(f) => {
                f.rows_mut()[1].set_flag("allowAltg", true); // level 0
                f.rows_mut()[1].set("nameColor", "0000FF");
                f.add_level();
            }
            Form::AdminCommands(f) => {
                // "admin" needed level 30; now only the highest level may use it.
                let row = f
                    .rows()
                    .iter()
                    .position(|r| r.get("command") == Some("admin"))
                    .unwrap();
                f.rows_mut()[row].set("accessLevel", "100");
                f.rows_mut()[row].set_flag("confirmDlg", true);
            }
            _ => {}
        }
    }
    view.save_all();
    assert_eq!(
        view.notes(),
        ["Restart the game server to apply: AccessLevels.xml, AdminCommands.xml"]
    );
    assert!(view.files().iter().all(|f| !f.is_dirty()), "reloaded clean");

    let after = game_server::world_data::AccessLevels::load(&dir);
    assert!(after.allows_alt_g(0));
    assert_eq!(
        after.get(0).name_color,
        0x00FF_0000 >> 16 | 0xFF,
        "0000FF read as the int 0xFF"
    );
    assert!(!after.rights().has_access("admin_admin", 30));
    assert!(after.rights().has_access("admin_admin", 100));
    assert!(after.rights().requires_confirm("admin_admin", 100));
    assert_eq!(after.rights().level_count(), before.rights().level_count() + 1);
    assert_eq!(after.rights().command_count(), before.rights().command_count());
}

#[test]
fn the_class_master_and_siege_loaders_read_the_written_files() {
    let Some(dist) = dist_config() else { return };
    let mut class_master = load(&dist, TopicId::ClassMaster);
    let Form::ClassMaster(form) = &mut class_master.form else {
        unreachable!()
    };
    form.settings_mut().set_flag("classChangeEnabled", true);
    form.options[1]
        .rewards
        .list
        .rows
        .push(Row::new(&[("id", "6622"), ("count", "2")]));
    let text = render(class_master.form.as_form(), &class_master.original).unwrap();
    let config = l2_data::ClassMasterConfig::from_xml(&text).unwrap();
    assert!(config.is_enabled);
    assert_eq!(config.class_change_data.len(), 2);
    assert_eq!(config.class_change_data[1].items_rewarded[0].id, 6622);
    assert_eq!(config.class_change_data[1].items_rewarded[0].count, 2);
    assert_eq!(config.class_change_data[1].items_required[0].count, 1_000_000);

    let mut siege = load(&dist, TopicId::Siege);
    let Form::Siege(form) = &mut siege.form else {
        unreachable!()
    };
    form.set_day(0, 5); // Gludio: FRIDAY
    form.rows_mut()[0].set("hour", "21");
    form.rows_mut()[0].set_flag("siegeEnabled", false);
    let text = render(siege.form.as_form(), &siege.original).unwrap();
    let schedule = l2_data::SiegeSchedule::from_xml(&text).unwrap();
    let gludio = schedule.for_castle(1).unwrap();
    assert_eq!((gludio.day, gludio.hour, gludio.siege_enabled), (6, 21, false));
    assert_eq!(schedule.len(), 9);
    // The other castles are as before.
    assert_eq!(schedule.for_castle(2).unwrap().hour, 20);
}

#[test]
fn the_secondary_auth_and_bot_report_loaders_read_the_written_files() {
    let Some(dist) = dist_config() else { return };
    let mut auth = load(&dist, TopicId::SecondaryAuth);
    let Form::SecondaryAuth(form) = &mut auth.form else {
        unreachable!()
    };
    form.enabled.value = "true".into();
    form.max_attempts.value = "3".into();
    form.ban_time.value = "10".into();
    // The `&` is written as `&amp;`; the game server's reader resolves it (G-09).
    form.recovery_link.value = "https://example.test/recover?a=1&b=2".into();
    form.add_password("135790");
    let text = render(auth.form.as_form(), &auth.original).unwrap();
    let data = game_server::client::secondary::SecondaryAuthData::parse(&text);
    assert!(data.enabled);
    assert_eq!((data.max_attempts, data.ban_time), (3, 10));
    assert_eq!(data.recovery_link, "https://example.test/recover?a=1&b=2");
    assert!(data.is_forbidden("135790") && data.is_forbidden("000000"));
    assert_eq!(
        data.forbidden.len(),
        game_server::client::secondary::SecondaryAuthData::parse(&auth.original)
            .forbidden
            .len()
            + 1
    );

    let mut report = load(&dist, TopicId::BotReport);
    let Form::BotReport(form) = &mut report.form else {
        unreachable!()
    };
    form.rows_mut()[0].set("neededReportCount", "30");
    form.add_punishment();
    let text = render(report.form.as_form(), &report.original).unwrap();
    let punishments = game_server::world_data::bot_report_punishments_from_xml(&text);
    assert!(punishments.contains_key(&30) && !punishments.contains_key(&25));
    assert!(punishments.contains_key(&-125), "the range key");
    assert_eq!(punishments.len(), 8);
}

#[test]
fn the_ipconfig_loader_reads_the_written_template() {
    let Some(dist) = dist_config() else { return };
    let mut ip = load(&dist, TopicId::IpTemplate);
    let Form::IpConfig(form) = &mut ip.form else {
        unreachable!()
    };
    form.set_default_address("203.0.113.5");
    form.add_define();
    form.defines_mut()[0].set("address", "127.0.0.2");
    let text = render(ip.form.as_form(), &ip.original).unwrap();
    let config = game_server::ls_link::ipconfig::IpConfig::parse(&text);
    assert_eq!(config.hosts.first().map(String::as_str), Some("127.0.0.2"));
    assert!(config.hosts.contains(&"203.0.113.5".to_owned()));
    assert!(config.subnets.contains(&"192.168.0.0/24".to_owned()));
    assert_eq!(config.subnets.last().map(String::as_str), Some("0.0.0.0/0"));
}

#[test]
fn dynamic_exp_rates_read_back_as_floats() {
    let Some(dist) = dist_config() else { return };
    let mut file = load(&dist, TopicId::DynamicExp);
    let Form::DynamicExp(form) = &mut file.form else {
        unreachable!()
    };
    assert_eq!(form.set_range(1, 40, 2.5, 1.5), 40);
    let text = render(file.form.as_form(), &file.original).unwrap();
    // Java: `parseInteger(attrs, "level")`, `parseFloat(attrs, "exp")`, `parseFloat(attrs, "sp")`.
    let doc = XmlDoc::parse(&text).unwrap();
    let rows = doc.children_named(0, "dynamic");
    assert_eq!(rows.len(), 87);
    for (n, &row) in rows.iter().enumerate() {
        let level: i32 = doc.attr(row, "level").unwrap().parse().unwrap();
        let exp: f32 = doc.attr(row, "exp").unwrap().parse().unwrap();
        let sp: f32 = doc.attr(row, "sp").unwrap().parse().unwrap();
        assert_eq!(level as usize, n + 1);
        let (want_exp, want_sp) = if level <= 40 { (2.5, 1.5) } else { (1.0, 1.0) };
        assert_eq!((exp, sp), (want_exp, want_sp));
    }
}

// ---- the view ---------------------------------------------------------------------------------

#[test]
fn the_view_lists_the_nine_files_and_saves_nothing_when_unchanged() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    assert_eq!(view.files().len(), 9);
    assert!(view.errors().is_empty(), "{:?}", view.errors());
    let titles: Vec<&str> = view.files().iter().map(|f| f.topic.title()).collect();
    assert!(titles.contains(&"Access levels") && titles.contains(&"Siege schedule"));
    let before: Vec<Vec<u8>> = view
        .files()
        .iter()
        .map(|f| std::fs::read(&f.path).unwrap())
        .collect();
    view.save_all();
    assert!(view.notes().is_empty());
    let after: Vec<Vec<u8>> = view
        .files()
        .iter()
        .map(|f| std::fs::read(&f.path).unwrap())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn errors_block_the_save_and_a_changed_file_is_refused() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    let file = view
        .files_mut()
        .iter_mut()
        .find(|f| f.topic == TopicId::Siege)
        .unwrap();
    let Form::Siege(form) = &mut file.form else {
        unreachable!()
    };
    form.rows_mut()[0].set("hour", "25");
    let path = file.path.clone();
    let bytes = std::fs::read(&path).unwrap();
    view.save_all();
    assert_eq!(view.notes().len(), 1);
    assert!(
        view.notes()[0].contains("SiegeSchedule.xml") && view.notes()[0].contains("0 to 23"),
        "{:?}",
        view.notes()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "nothing was written");

    // Fix the value, but let someone else change the file first.
    let file = view
        .files_mut()
        .iter_mut()
        .find(|f| f.topic == TopicId::Siege)
        .unwrap();
    let Form::Siege(form) = &mut file.form else {
        unreachable!()
    };
    form.rows_mut()[0].set("hour", "17");
    std::fs::write(&path, [bytes.as_slice(), b"<!-- by hand -->\r\n"].concat()).unwrap();
    view.save_all();
    assert!(view.notes()[0].contains("changed on disk"), "{:?}", view.notes());
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .ends_with("<!-- by hand -->\r\n")
    );
}

#[test]
fn ipconfig_xml_is_created_from_the_template_and_then_listed() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    assert!(view.files().iter().all(|f| f.topic != TopicId::IpConfig));
    view.create_ipconfig();
    assert!(view.files().iter().any(|f| f.topic == TopicId::IpConfig));
    assert_eq!(view.files().len(), 10);
    view.create_ipconfig();
    assert!(view.notes()[0].contains("exists already"));
}

#[test]
fn discard_puts_the_form_back() {
    let Some(dist) = dist_config() else { return };
    let mut file = load(&dist, TopicId::ClassMaster);
    let Form::ClassMaster(form) = &mut file.form else {
        unreachable!()
    };
    form.options.remove(0);
    assert!(file.is_dirty());
    file.discard();
    assert!(!file.is_dirty());
    let Form::ClassMaster(form) = &file.form else {
        unreachable!()
    };
    assert_eq!(form.options.len(), 2);
}

#[test]
fn the_admin_commands_form_gets_the_levels_of_the_access_levels_form() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    view.sync_levels();
    let file = view
        .files()
        .iter()
        .find(|f| f.topic == TopicId::AdminCommands)
        .unwrap();
    // All dist commands use a level that AccessLevels.xml lists: no warning.
    assert!(file.issues().is_empty(), "{:?}", file.issues());
    // A level removed in the access levels form shows up as a warning on the commands.
    let file = view
        .files_mut()
        .iter_mut()
        .find(|f| f.topic == TopicId::AccessLevels)
        .unwrap();
    let Form::AccessLevels(form) = &mut file.form else {
        unreachable!()
    };
    let at = form
        .rows()
        .iter()
        .position(|r| r.get("level") == Some("30"))
        .unwrap();
    form.delete_level(at);
    view.sync_levels();
    let file = view
        .files()
        .iter()
        .find(|f| f.topic == TopicId::AdminCommands)
        .unwrap();
    assert!(
        file.issues()
            .iter()
            .any(|i| i.warning && i.message.contains("level 30"))
    );
}

// ---- drawing ----------------------------------------------------------------------------------

fn draw(view: &mut XmlConfigView, frames: usize) {
    let ctx = egui::Context::default();
    for _ in 0..frames {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| view.show(ui));
        output.textures_delta.clear();
    }
}

#[test]
fn every_topic_draws_without_panicking() {
    let Some(dist) = dist_config() else { return };
    let run = run_folder(&dist);
    let mut view = XmlConfigView::with_run_dir(run.path().to_str().unwrap());
    view.reload();
    for i in 0..view.files().len() {
        view.selected = i;
        draw(&mut view, 3);
    }
    // With a change in each form (red fields, marks and the changed lists).
    for file in view.files_mut() {
        match &mut file.form {
            Form::AccessLevels(f) => {
                f.rows_mut()[1].set("level", "x");
                f.copy_level(0);
            }
            Form::AdminCommands(f) => {
                f.search = "ban".into();
                f.level_filter = Some(100);
            }
            Form::Siege(f) => f.rows_mut()[0].set("hour", "99"),
            Form::BotReport(f) => f.rows_mut()[0].set("skillId", "0"),
            Form::SecondaryAuth(f) => {
                f.add_password("abc");
                f.unlocked = true;
            }
            Form::DynamicExp(f) => f.rows_mut()[0].set("exp", "-4"),
            Form::ClassMaster(f) => {
                f.options.push(class_master::OptionModel::new("x", "BAD"));
            }
            Form::IpConfig(f) => f.set_default_address("bad"),
            Form::Scripts(_) => {}
        }
    }
    for i in 0..view.files().len() {
        view.selected = i;
        draw(&mut view, 3);
    }
}

#[test]
fn an_empty_run_folder_draws_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut view = XmlConfigView::with_run_dir(dir.path().to_str().unwrap());
    draw(&mut view, 2);
    assert!(view.files().is_empty());
    assert_eq!(view.errors().len(), 9, "every required file is reported");
}
