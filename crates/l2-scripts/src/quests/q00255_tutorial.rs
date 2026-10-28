//! Quest 255: Tutorial (the newbie helpers of every village, the tutorial window, the question
//! marks and the client events of a new character).
// Java: dist/game/data/scripts/quests/Q00255_Tutorial/Q00255_Tutorial.java
//
// Pattern: the tutorial (`docs/design/scripts.md`, "Quest patterns"): no journal entry, the
// state is started at login (`on_enter_world`) and lives in `memoState` (a bit field of the
// shown steps) and `memoStateEx1` (the newbie helper step). The world sends the tutorial
// packets here as events: `TL:<link>` (RequestTutorialLinkHtml), `TC:<bypass>`
// (RequestTutorialPassCmdToServer), `QM:<number>` (RequestTutorialQuestionMark), `CE:<id>`
// (RequestTutorialClientEvent); numeric events are the helper NPCs' links and the quest timers.
//
// The player listeners Java adds (`ON_PLAYER_LEVEL_CHANGED`, `ON_PLAYER_ITEM_PICKUP`,
// `ON_PLAYER_SIT`, `ON_CREATURE_ATTACKED`) are not engine hooks yet (follow-up): the script
// keeps them in its state as Java registers them, and their bodies are the `pub` methods
// `on_player_level_changed`, `on_player_item_pickup`, `on_player_sit` and
// `on_creature_attacked` for the engine to call.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::server::{PlaySound, TutorialEnableClientEvent, TutorialShowQuestionMark};
use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED, player_class as pc, race};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROIEN: i32 = 30008;
const NEWBIE_HELPER_HUMAN_FIGHTER: i32 = 30009;
const GALLINT: i32 = 30017;
const NEWBIE_HELPER_HUMAN_MAGE: i32 = 30019;
const MITRAELL: i32 = 30129;
const NEWBIE_HELPER_DARK_ELF: i32 = 30131;
const NERUPA: i32 = 30370;
const NEWBIE_HELPER_ELF: i32 = 30400;
const LAFERON: i32 = 30528;
const NEWBIE_HELPER_DWARF: i32 = 30530;
const VULKUS: i32 = 30573;
const NEWBIE_HELPER_ORC: i32 = 30575;
const PERWAN: i32 = 32133;
const NEWBIE_HELPER_KAMAEL: i32 = 32134;

// Monster
const TUTORIAL_GREMLIN: i32 = 18342;

// Items
const SOULSHOT_NO_GRADE_FOR_BEGINNERS: i32 = 5789;
const SPIRITSHOT_NO_GRADE_FOR_BEGINNERS: i32 = 5790;
const BLUE_GEMSTONE: i32 = 6353;
const TUTORIAL_GUIDE: i32 = 5588;

// Quest items
const RECOMMENDATION_1: i32 = 1067;
const RECOMMENDATION_2: i32 = 1068;
const LEAF_OF_THE_MOTHER_TREE: i32 = 1069;
const BLOOD_OF_MITRAELL: i32 = 1070;
const LICENSE_OF_MINER: i32 = 1498;
const VOUCHER_OF_FLAME: i32 = 1496;
const DIPLOMA: i32 = 9881;

// Territory wars
const TW_GLUDIO: i32 = 81;
const TW_DION: i32 = 82;
const TW_GIRAN: i32 = 83;
const TW_OREN: i32 = 84;
const TW_ADEN: i32 = 85;
const TW_HEINE: i32 = 86;
const TW_GODDARD: i32 = 87;
const TW_RUNE: i32 = 88;
const TW_SCHUTTGART: i32 = 89;

// Connected quests
const Q10276_MUTATED_KANEUS_GLUDIO: i32 = 10276;
const Q10277_MUTATED_KANEUS_DION: i32 = 10277;
const Q10278_MUTATED_KANEUS_HEINE: i32 = 10278;
const Q10279_MUTATED_KANEUS_OREN: i32 = 10279;
const Q10280_MUTATED_KANEUS_SCHUTTGART: i32 = 10280;
const Q10281_MUTATED_KANEUS_RUNE: i32 = 10281;
const Q192_SEVEN_SIGNS_SERIES_OF_DOUBT: i32 = 192;
const Q10292_SEVEN_SIGNS_GIRL_OF_DOUBT: i32 = 10292;
const Q234_FATES_WHISPER: i32 = 234;
const Q128_PAILAKA_SONG_OF_ICE_AND_FIRE: i32 = 128;
const Q129_PAILAKA_DEVILS_LEGACY: i32 = 129;
const Q144_PAIRAKA_WOUNDED_DRAGON: i32 = 144;

const Q729_PROTECT_THE_TERRITORY_CATAPULT: i32 = 729;
const Q730_PROTECT_THE_SUPPLIES_SAFE: i32 = 730;
const Q731_PROTECT_THE_MILITARY_ASSOCIATION_LEADER: i32 = 731;
const Q732_PROTECT_THE_RELIGIOUS_ASSOCIATION_LEADER: i32 = 732;
const Q733_PROTECT_THE_ECONOMIC_ASSOCIATION_LEADER: i32 = 733;

const Q201_TUTORIAL_HUMAN_FIGHTER: i32 = 201;
const Q202_TUTORIAL_HUMAN_MAGE: i32 = 202;
const Q203_TUTORIAL_ELF: i32 = 203;
const Q204_TUTORIAL_DARK_ELF: i32 = 204;
const Q205_TUTORIAL_ORC: i32 = 205;
const Q206_TUTORIAL_DWARF: i32 = 206;

const Q717_FOR_THE_SAKE_OF_THE_TERRITORY_GLUDIO: i32 = 717;
const Q718_FOR_THE_SAKE_OF_THE_TERRITORY_DION: i32 = 718;
const Q719_FOR_THE_SAKE_OF_THE_TERRITORY_GIRAN: i32 = 719;
const Q720_FOR_THE_SAKE_OF_THE_TERRITORY_OREN: i32 = 720;
const Q721_FOR_THE_SAKE_OF_THE_TERRITORY_ADEN: i32 = 721;
const Q722_FOR_THE_SAKE_OF_THE_TERRITORY_INNADRIL: i32 = 722;
const Q723_FOR_THE_SAKE_OF_THE_TERRITORY_GODDARD: i32 = 723;
const Q724_FOR_THE_SAKE_OF_THE_TERRITORY_RUNE: i32 = 724;
const Q725_FOR_THE_SAKE_OF_THE_TERRITORY_SCHUTTGART: i32 = 725;
const Q728_TERRITORY_WAR: i32 = 728;

/// `2147483632`: the memo state without its low 4 bits.
const MEMO_FLAG_MASK: i32 = 2147483632;
/// `2147483392`: the memo state without its low 8 bits.
const MEMO_MASK: i32 = 2147483392;

/// The NPCs of `addStartNpc`, `addFirstTalkId` and `addTalkId`.
const NPCS: [i32; 14] = [
    ROIEN,
    NEWBIE_HELPER_HUMAN_FIGHTER,
    GALLINT,
    NEWBIE_HELPER_HUMAN_MAGE,
    MITRAELL,
    NEWBIE_HELPER_DARK_ELF,
    NERUPA,
    NEWBIE_HELPER_ELF,
    LAFERON,
    NEWBIE_HELPER_DWARF,
    VULKUS,
    NEWBIE_HELPER_ORC,
    PERWAN,
    NEWBIE_HELPER_KAMAEL,
];

/// The listeners Java adds to the player (`player.addListener(...)`), per player. They go with
/// the `Player` object in Java, so a login starts without them.
#[derive(Default)]
struct TutorialListeners {
    /// `ON_PLAYER_LEVEL_CHANGED` (added by `onEnterWorld`).
    level_changed: HashSet<ObjectId>,
    /// `ON_PLAYER_ITEM_PICKUP`: how many; Java adds one more every time without a check.
    item_pickup: HashMap<ObjectId, u32>,
    /// `ON_PLAYER_SIT` (added only when there is none).
    sit: HashSet<ObjectId>,
    /// `ON_CREATURE_ATTACKED`: how many, like `item_pickup`.
    attacked: HashMap<ObjectId, u32>,
}

/// Java `Q00255_Tutorial`.
pub struct Q00255Tutorial;

impl Script for Q00255Tutorial {
    fn name(&self) -> &'static str {
        "Q00255_Tutorial"
    }

    fn path(&self) -> &'static str {
        "quests/Q00255_Tutorial"
    }

    fn id(&self) -> i32 {
        255
    }

    fn setup(&self, s: &mut ScriptSetup) {
        // NOTE: Java registers nothing with `DisableTutorial = True`; the setting is not known
        // when the scripts are set up here, so every hook checks it instead.
        s.set_on_enter_world();
        s.add_start_npc(&NPCS);
        s.add_first_talk_id(&NPCS);
        s.add_talk_id(&NPCS);
        s.add_kill_id(&[TUTORIAL_GREMLIN]);
    }

    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if disabled(ctx) {
            return;
        }
        // A new `Player` object: the listeners of the last login are gone.
        forget_listeners(ctx, player);
        user_connected(ctx, player);
        ctx.state::<TutorialListeners>().level_changed.insert(player);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if disabled(ctx) {
            // Without the listener the NPC shows its own page.
            ctx.show_chat_window(npc, talker);
            return Some(String::new());
        }
        let qs = ctx.get_quest_state(talker, true)?;
        match npc.id {
            ROIEN => talk_roien(ctx, npc, talker, qs),
            NEWBIE_HELPER_HUMAN_FIGHTER => talk_carl(ctx, npc, talker, qs),
            GALLINT => talk_gallin(ctx, npc, talker, qs),
            NEWBIE_HELPER_HUMAN_MAGE => talk_doff(ctx, npc, talker, qs),
            MITRAELL => talk_jundin(ctx, npc, talker, qs),
            NEWBIE_HELPER_DARK_ELF => talk_poeny(ctx, npc, talker, qs),
            NERUPA => talk_nerupa(ctx, npc, talker, qs),
            NEWBIE_HELPER_ELF => talk_mother_temp(ctx, npc, talker, qs),
            LAFERON => talk_foreman_laferon(ctx, npc, talker, qs),
            NEWBIE_HELPER_DWARF => talk_miner_mai(ctx, npc, talker, qs),
            VULKUS => talk_guardian_vullkus(ctx, npc, talker, qs),
            NEWBIE_HELPER_ORC => talk_shela_priestess(ctx, npc, talker, qs),
            PERWAN => talk_subelder_perwan(ctx, npc, talker, qs),
            NEWBIE_HELPER_KAMAEL => talk_helper_krenisk(ctx, npc, talker, qs),
            _ => {}
        }
        Some(String::new())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        talker: Option<ObjectId>,
    ) -> Option<String> {
        let talker = talker?;
        if !is_numeric(event) {
            if disabled(ctx) {
                return None;
            }

            // A number that does not parse throws in Java: the event does nothing.
            if let Some(rest) = event.strip_prefix("CE:") {
                if let Ok(event_id) = rest.parse::<i32>() {
                    tutorial_event(ctx, talker, event_id);
                }
            } else if event.starts_with("TL:") {
                let start = if event.contains("_close_") { 18 } else { 5 };
                if let Some(Ok(mut pass)) = event.get(start..).map(str::parse::<i32>) {
                    if pass < 302 {
                        pass = -pass;
                    }
                    tutorial_event(ctx, talker, pass);
                }
            } else if let Some(rest) = event.strip_prefix("TC:") {
                if let Ok(reply) = rest.parse::<i32>() {
                    select_from_menu(ctx, talker, reply);
                }
            } else if let Some(rest) = event.strip_prefix("QM:") {
                if let Ok(question_id) = rest.parse::<i32>() {
                    question_mark_clicked(ctx, talker, question_id);
                }
            }
            return None;
        }

        let event_id = event.parse::<i32>().ok()?;
        if event_id > 1000000 {
            fire_event(ctx, event_id, talker);
            return None;
        }

        if ctx.is_dead(talker) {
            return None;
        }

        let qs = ctx.get_quest_state(talker, true)?;
        // Java reads `npc.getId()` here: the helpers' links and timers always have the NPC.
        let npc = npc?;
        match npc.id {
            NEWBIE_HELPER_HUMAN_FIGHTER => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010a"),
                _ => {}
            },
            NEWBIE_HELPER_HUMAN_MAGE => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_009b");
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010b"),
                _ => {}
            },
            NEWBIE_HELPER_DARK_ELF => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    if !ctx.is_mage_class(talker) {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    } else {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009b");
                    }
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010d"),
                _ => {}
            },
            NEWBIE_HELPER_ELF => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    if !ctx.is_mage_class(talker) {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    } else {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009b");
                    }
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010c"),
                _ => {}
            },
            NEWBIE_HELPER_DWARF => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010f"),
                _ => {}
            },
            NEWBIE_HELPER_ORC => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    if !ctx.is_mage_class(talker) {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    } else {
                        play_tutorial_voice(ctx, talker, "tutorial_voice_009c");
                    }
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010e"),
                _ => {}
            },
            NEWBIE_HELPER_KAMAEL => match qs.get_memo_state_ex(ctx, 1) {
                0 => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_009a");
                    qs.set_memo_state_ex(ctx, 1, 1);
                }
                3 => play_tutorial_voice(ctx, talker, "tutorial_voice_010g"),
                _ => {}
            },
            ROIEN => {
                if event_id == ROIEN {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_roien(ctx, event_id, talker, npc, qs);
                }
            }
            GALLINT => event_gallin(ctx, event_id, talker, npc, qs),
            MITRAELL => {
                if event_id == MITRAELL {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_jundin(ctx, event_id, talker, npc, qs);
                }
            }
            NERUPA => {
                if event_id == NERUPA {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_nerupa(ctx, event_id, talker, npc, qs);
                }
            }
            LAFERON => {
                if event_id == LAFERON {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_foreman_laferon(ctx, event_id, talker, npc, qs);
                }
            }
            VULKUS => {
                if event_id == VULKUS {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_guardian_vullkus(ctx, event_id, talker, npc, qs);
                }
            }
            PERWAN => {
                if event_id == PERWAN {
                    teacher_timer(ctx, talker, qs);
                } else {
                    event_subelder_perwan(ctx, event_id, talker, npc, qs);
                }
            }
            _ => {}
        }
        Some(String::new())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if disabled(ctx) {
            return;
        }
        // Java would throw for a killer without the state.
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if npc.id == TUTORIAL_GREMLIN {
            let ex = qs.get_memo_state_ex(ctx, 1);
            if ex == 1 || ex == 0 {
                ctx.play_sound_named(killer, "tutorial_voice_011");
                show_question_mark(ctx, killer, 3);
                qs.set_memo_state_ex(ctx, 1, 2);
            }

            let ex = qs.get_memo_state_ex(ctx, 1);
            // NOTE(java-quirk): `getRandom(2) <= 1` is always true (the roll is still made).
            if (ex == 1 || ex == 2 || ex == 0)
                && !ctx.has_quest_items(killer, BLUE_GEMSTONE)
                && ctx.get_random(2) <= 1
            {
                ctx.drop_item(npc, Some(killer), BLUE_GEMSTONE, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestTutorial);
            }
        }
    }
}

impl Q00255Tutorial {
    /// The `ON_PLAYER_LEVEL_CHANGED` listener of `onEnterWorld`: `levelUp(player, newLevel)`.
    pub fn on_player_level_changed(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, new_level: i32) {
        if ctx.state::<TutorialListeners>().level_changed.contains(&player) {
            level_up(ctx, player, new_level);
        }
    }

    /// The `ON_PLAYER_ITEM_PICKUP` listeners of `enableTutorialEvent`: the blue gemstone and
    /// the first adena, once per listener.
    pub fn on_player_item_pickup(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, item_id: i32) {
        let count = ctx
            .state::<TutorialListeners>()
            .item_pickup
            .get(&player)
            .copied()
            .unwrap_or(0);
        for _ in 0..count {
            let Some(qs) = ctx.get_quest_state(player, false) else {
                return;
            };
            if item_id == BLUE_GEMSTONE && (qs.get_memo_state(ctx) & 1048576) != 0 {
                tutorial_event(ctx, player, 1048576);
            } else if item_id == 57 && (qs.get_memo_state(ctx) & 2097152) != 0 {
                tutorial_event(ctx, player, 2097152);
            }
        }
    }

    /// The `ON_PLAYER_SIT` listener of `enableTutorialEvent`.
    pub fn on_player_sit(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if ctx.state::<TutorialListeners>().sit.contains(&player) {
            tutorial_event(ctx, player, 8388608);
        }
    }

    /// The `ON_CREATURE_ATTACKED` listeners of `enableTutorialEvent`: at 30% HP or less.
    pub fn on_creature_attacked(&self, ctx: &mut ScriptCtx<'_>, target: ObjectId) {
        let count = ctx
            .state::<TutorialListeners>()
            .attacked
            .get(&target)
            .copied()
            .unwrap_or(0);
        for _ in 0..count {
            let Some(p) = ctx.world().player(target) else {
                return;
            };
            let max_hp = ctx.world().stats(target).map_or(0, |s| s.max_hp());
            if p.core.cur_hp <= f64::from(max_hp) * 0.3 {
                tutorial_event(ctx, target, 256);
            }
        }
    }
}

fn disabled(ctx: &ScriptCtx<'_>) -> bool {
    ctx.script_settings().disable_tutorial
}

/// Java `StringUtil.isNumeric(text)`: only digits, not empty.
fn is_numeric(text: &str) -> bool {
    // NOTE: Java's `Character.isDigit` also takes the digits of other scripts; the client
    // sends ASCII.
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

fn forget_listeners(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let listeners = ctx.state::<TutorialListeners>();
    listeners.level_changed.remove(&player);
    listeners.item_pickup.remove(&player);
    listeners.sit.remove(&player);
    listeners.attacked.remove(&player);
}

/// The timer of the player's tutorial steps: `String.valueOf(talker.getObjectId() + 1000000)`.
fn player_timer(player: ObjectId) -> String {
    // NOTE(java-quirk): an int sum; above 2146483647 it wraps to a negative, non-numeric event.
    player.0.wrapping_add(1000000).to_string()
}

/// `startQuestTimer(String.valueOf(talker.getObjectId() + 1000000), time, null, player)`.
fn start_player_timer(ctx: &mut ScriptCtx<'_>, player: ObjectId, time_ms: i64) {
    ctx.start_quest_timer(&player_timer(player), time_ms, None, Some(player), false);
}

/// `startQuestTimer(String.valueOf(npc.getId()), time, npc, talker)`.
fn start_npc_timer(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, time_ms: i64) {
    ctx.start_quest_timer(&npc.id.to_string(), time_ms, Some(npc), Some(talker), false);
}

/// The teacher's own timer (`eventId == npc id`): the question mark of the first class change.
fn teacher_timer(ctx: &mut ScriptCtx<'_>, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) >= 4 {
        show_question_mark(ctx, talker, 7);
        ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
        play_tutorial_voice(ctx, talker, "tutorial_voice_025");
    }
}

/// Java `enableTutorialEvent(qs, eventStatus)`: the player listeners of the bits, then
/// `TutorialEnableClientEvent`.
fn enable_tutorial_event(ctx: &mut ScriptCtx<'_>, qs: Qs, event_status: i32) {
    let player = qs.player();
    let level = ctx.player_level(player);
    let listeners = ctx.state::<TutorialListeners>();
    if (event_status & (1048576 | 2097152)) != 0 {
        if level < 6 {
            *listeners.item_pickup.entry(player).or_default() += 1;
        }
    } else if listeners.item_pickup.contains_key(&player) {
        listeners.item_pickup.remove(&player);
    }

    if (event_status & 8388608) != 0 {
        if !listeners.sit.contains(&player) {
            listeners.sit.insert(player);
        }
    } else if listeners.sit.contains(&player) {
        listeners.sit.remove(&player);
    }

    if (event_status & 256) != 0 {
        if level < 6 {
            *listeners.attacked.entry(player).or_default() += 1;
        }
    } else {
        listeners.attacked.remove(&player);
    }

    ctx.world_mut().send(
        player,
        &TutorialEnableClientEvent {
            event_id: event_status,
        },
    );
}

/// Java `fireEvent(timer_id, talker)`: the player's step timer.
fn fire_event(ctx: &mut ScriptCtx<'_>, timer_id: i32, talker: ObjectId) {
    if ctx.world().player(talker).is_none() || ctx.is_dead(talker) || timer_id <= 1000000 {
        return;
    }

    let Some(qs) = ctx.get_quest_state(talker, true) else {
        return;
    };
    match qs.get_memo_state_ex(ctx, 1) {
        -2 => {
            match ctx.player_class_id(talker) {
                pc::FIGHTER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001a");
                    show_tutorial_html(ctx, talker, "tutorial-human-fighter-001.html");
                }
                pc::MAGE => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001b");
                    show_tutorial_html(ctx, talker, "tutorial-human-mage-001.html");
                }
                pc::ELVEN_FIGHTER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001c");
                    show_tutorial_html(ctx, talker, "tutorial-elven-fighter-001.html");
                }
                pc::ELVEN_MAGE => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001d");
                    show_tutorial_html(ctx, talker, "tutorial-elven-mage-001.html");
                }
                pc::DARK_FIGHTER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001e");
                    show_tutorial_html(ctx, talker, "tutorial-delf-fighter-001.html");
                }
                pc::DARK_MAGE => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001f");
                    show_tutorial_html(ctx, talker, "tutorial-delf-mage-001.html");
                }
                pc::ORC_FIGHTER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001g");
                    show_tutorial_html(ctx, talker, "tutorial-orc-fighter-001.html");
                }
                pc::ORC_MAGE => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001h");
                    show_tutorial_html(ctx, talker, "tutorial-orc-mage-001.html");
                }
                pc::DWARVEN_FIGHTER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001i");
                    show_tutorial_html(ctx, talker, "tutorial-dwarven-fighter-001.html");
                }
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_001k");
                    show_tutorial_html(ctx, talker, "tutorial-kamael-001.html");
                }
                _ => {}
            }

            if !ctx.has_quest_items(talker, TUTORIAL_GUIDE) {
                ctx.give_items(talker, TUTORIAL_GUIDE, 1);
            }

            start_player_timer(ctx, qs.player(), 30000);
            qs.set_memo_state_ex(ctx, 1, -3);
        }
        -3 => play_tutorial_voice(ctx, talker, "tutorial_voice_002"),
        -4 => {
            play_tutorial_voice(ctx, talker, "tutorial_voice_008");
            qs.set_memo_state_ex(ctx, 1, -5);
        }
        _ => {}
    }
}

/// `showQuestionMark`, the tutorial sound, and a memo bit cleared: the common end of most
/// client events.
fn mark(ctx: &mut ScriptCtx<'_>, talker: ObjectId, number: i32) {
    show_question_mark(ctx, talker, number);
    ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
}

/// Java `tutorialEvent(talker, event_id)`: the client events, the tutorial links (negative)
/// and the level steps.
fn tutorial_event(ctx: &mut ScriptCtx<'_>, talker: ObjectId, event_id: i32) {
    let Some(qs) = ctx.get_quest_state(talker, false) else {
        return;
    };

    if event_id == 0 {
        close_tutorial_html(ctx, talker);
        return;
    }

    let memo_state = qs.get_memo_state(ctx);
    let memo_flag = memo_state & MEMO_FLAG_MASK;

    if event_id < 0 {
        match event_id.wrapping_abs() {
            1 => {
                close_tutorial_html(ctx, talker);
                play_tutorial_voice(ctx, talker, "tutorial_voice_006");
                mark(ctx, talker, 1);
                start_player_timer(ctx, qs.player(), 30000);
                if qs.get_memo_state_ex(ctx, 1) <= 0 {
                    qs.set_memo_state_ex(ctx, 1, -4);
                }
            }
            2 => {
                play_tutorial_voice(ctx, talker, "tutorial_voice_003");
                show_tutorial_html(ctx, talker, "tutorial-02.html");
                enable_tutorial_event(ctx, qs, memo_flag | 1);
                if qs.get_memo_state_ex(ctx, 1) <= 0 {
                    qs.set_memo_state_ex(ctx, 1, -5);
                }
            }
            3 => {
                show_tutorial_html(ctx, talker, "tutorial-03.html");
                enable_tutorial_event(ctx, qs, memo_flag | 2);
            }
            4 => {
                show_tutorial_html(ctx, talker, "tutorial-04.html");
                enable_tutorial_event(ctx, qs, memo_flag | 4);
            }
            5 => {
                show_tutorial_html(ctx, talker, "tutorial-05.html");
                enable_tutorial_event(ctx, qs, memo_flag | 8);
            }
            6 => {
                show_tutorial_html(ctx, talker, "tutorial-06.html");
                enable_tutorial_event(ctx, qs, memo_flag | 16);
            }
            7 => {
                show_tutorial_html(ctx, talker, "tutorial-100.html");
                enable_tutorial_event(ctx, qs, memo_flag);
            }
            8 => {
                show_tutorial_html(ctx, talker, "tutorial-101.html");
                enable_tutorial_event(ctx, qs, memo_flag);
            }
            9 => {
                show_tutorial_html(ctx, talker, "tutorial-102.html");
                enable_tutorial_event(ctx, qs, memo_flag);
            }
            10 => {
                show_tutorial_html(ctx, talker, "tutorial-103.html");
                enable_tutorial_event(ctx, qs, memo_flag);
            }
            11 => {
                show_tutorial_html(ctx, talker, "tutorial-104.html");
                enable_tutorial_event(ctx, qs, memo_flag);
            }
            12 => close_tutorial_html(ctx, talker),
            _ => {}
        }
        return;
    }

    let level = ctx.player_level(talker);
    let class = ctx.player_class_id(talker);
    match event_id {
        1 => {
            if level < 6 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_004");
                show_tutorial_html(ctx, talker, "tutorial-03.html");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
                enable_tutorial_event(ctx, qs, memo_flag | 2);
            }
        }
        2 => {
            if level < 6 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_005");
                show_tutorial_html(ctx, talker, "tutorial-05.html");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
                enable_tutorial_event(ctx, qs, memo_flag | 8);
            }
        }
        8 => {
            if level < 6 {
                show_tutorial_html(ctx, talker, "tutorial-human-fighter-007.html");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
                match class {
                    pc::FIGHTER => ctx.add_radar(talker, -71424, 258336, -3109),
                    pc::MAGE => ctx.add_radar(talker, -91036, 248044, -3568),
                    pc::ELVEN_FIGHTER | pc::ELVEN_MAGE => ctx.add_radar(talker, 46112, 41200, -3504),
                    pc::DARK_FIGHTER | pc::DARK_MAGE => ctx.add_radar(talker, 28384, 11056, -4233),
                    pc::ORC_FIGHTER | pc::ORC_MAGE => ctx.add_radar(talker, -56736, -113680, -672),
                    pc::DWARVEN_FIGHTER => ctx.add_radar(talker, 108567, -173994, -406),
                    pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => ctx.add_radar(talker, -125872, 38016, 1251),
                    _ => {}
                }

                play_tutorial_voice(ctx, talker, "tutorial_voice_007");
                qs.set_memo_state(ctx, memo_flag | 2);
                if qs.get_memo_state_ex(ctx, 1) <= 0 {
                    qs.set_memo_state_ex(ctx, 1, -5);
                }
            }
        }
        256 => {
            if level < 6 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_017");
                mark(ctx, talker, 10);
                qs.set_memo_state(ctx, memo_state & !256);
                enable_tutorial_event(ctx, qs, (memo_flag & !256) | 8388608);
            }
        }
        512 => {
            show_question_mark(ctx, talker, 8);
            play_tutorial_voice(ctx, talker, "tutorial_voice_016");
            ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
            qs.set_memo_state(ctx, memo_state & !512);
        }
        1024 => {
            qs.set_memo_state(ctx, memo_state & !1024);
            match class {
                pc::FIGHTER => ctx.add_radar(talker, -83020, 242553, -3718),
                pc::ELVEN_FIGHTER => ctx.add_radar(talker, 45061, 52468, -2796),
                pc::DARK_FIGHTER => ctx.add_radar(talker, 10447, 14620, -4242),
                pc::ORC_FIGHTER => ctx.add_radar(talker, -46389, -113905, -21),
                pc::DWARVEN_FIGHTER => ctx.add_radar(talker, 115271, -182692, -1445),
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => ctx.add_radar(talker, -118132, 42788, 723),
                _ => {}
            }

            if !ctx.is_mage_class(talker) {
                play_tutorial_voice(ctx, talker, "tutorial_voice_014");
                mark(ctx, talker, 9);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 134217728);
            qs.set_memo_state(ctx, memo_state & !1024);
        }
        134217728 => {
            show_question_mark(ctx, talker, 24);
            play_tutorial_voice(ctx, talker, "tutorial_voice_020");
            ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
            enable_tutorial_event(ctx, qs, memo_flag & !134217728);
            qs.set_memo_state(ctx, memo_state & !134217728);
            enable_tutorial_event(ctx, qs, memo_flag | 2048);
        }
        2048 => {
            if ctx.is_mage_class(talker) {
                play_tutorial_voice(ctx, talker, "tutorial_voice_019");
                mark(ctx, talker, 11);
                match class {
                    pc::MAGE => ctx.add_radar(talker, -84981, 244764, -3726),
                    pc::ELVEN_MAGE => ctx.add_radar(talker, 45701, 52459, -2796),
                    pc::DARK_MAGE => ctx.add_radar(talker, 45701, 52459, -2796),
                    pc::ORC_MAGE => ctx.add_radar(talker, -46225, -113312, -21),
                    _ => {}
                }

                qs.set_memo_state(ctx, memo_state & !2048);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 268435456);
        }
        268435456 => {
            if class == pc::FIGHTER {
                play_tutorial_voice(ctx, talker, "tutorial_voice_021");
                mark(ctx, talker, 25);
                qs.set_memo_state(ctx, memo_state & !268435456);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 536870912);
        }
        536870912 => {
            match class {
                pc::DWARVEN_FIGHTER
                | pc::MAGE
                | pc::ELVEN_FIGHTER
                | pc::ELVEN_MAGE
                | pc::DARK_MAGE
                | pc::DARK_FIGHTER
                | pc::MALE_SOLDIER
                | pc::FEMALE_SOLDIER => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_021");
                    mark(ctx, talker, 25);
                    qs.set_memo_state(ctx, memo_state & !536870912);
                }
                _ => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_030");
                    mark(ctx, talker, 27);
                    qs.set_memo_state(ctx, memo_state & !536870912);
                }
            }

            enable_tutorial_event(ctx, qs, memo_flag | 1073741824);
        }
        1073741824 => {
            match class {
                pc::ORC_FIGHTER | pc::ORC_MAGE => {
                    play_tutorial_voice(ctx, talker, "tutorial_voice_021");
                    mark(ctx, talker, 25);
                    qs.set_memo_state(ctx, memo_state & !1073741824);
                }
                _ => {}
            }

            enable_tutorial_event(ctx, qs, memo_flag | 67108864);
        }
        67108864 => {
            mark(ctx, talker, 17);
            qs.set_memo_state(ctx, memo_state & !67108864);
            enable_tutorial_event(ctx, qs, memo_flag | 4096);
        }
        4096 => {
            mark(ctx, talker, 13);
            qs.set_memo_state(ctx, memo_state & !4096);
            enable_tutorial_event(ctx, qs, memo_flag | 16777216);
        }
        16777216 => {
            if ctx.player_class_race(talker) != race::KAMAEL {
                play_tutorial_voice(ctx, talker, "tutorial_voice_023");
                mark(ctx, talker, 15);
                qs.set_memo_state(ctx, memo_state & !16777216);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 32);
        }
        16384 => {
            if ctx.player_class_race(talker) == race::KAMAEL && ctx.player_class_level(talker) == 1 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_028");
                mark(ctx, talker, 15);
                qs.set_memo_state(ctx, memo_state & !16384);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 64);
        }
        33554432 => {
            if ctx.get_one_time_quest_flag(talker, Q234_FATES_WHISPER) == 0 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_024");
                mark(ctx, talker, 16);
                qs.set_memo_state(ctx, memo_state & !33554432);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 32768);
        }
        32768 => {
            if ctx.get_one_time_quest_flag(talker, Q234_FATES_WHISPER) == 1 {
                mark(ctx, talker, 29);
                qs.set_memo_state(ctx, memo_state & !32768);
            }
        }
        32 => {
            if ctx.get_one_time_quest_flag(talker, Q128_PAILAKA_SONG_OF_ICE_AND_FIRE) == 0 {
                mark(ctx, talker, 30);
                qs.set_memo_state(ctx, memo_state & !32);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 16384);
        }
        64 => {
            if ctx.get_one_time_quest_flag(talker, Q129_PAILAKA_DEVILS_LEGACY) == 0 {
                mark(ctx, talker, 31);
                qs.set_memo_state(ctx, memo_state & !64);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 128);
        }
        128 => {
            if ctx.get_one_time_quest_flag(talker, Q144_PAIRAKA_WOUNDED_DRAGON) == 0 {
                mark(ctx, talker, 32);
                qs.set_memo_state(ctx, memo_state & !128);
            }

            enable_tutorial_event(ctx, qs, memo_flag | 33554432);
        }
        2097152 => {
            if level < 6 {
                show_question_mark(ctx, talker, 23);
                play_tutorial_voice(ctx, talker, "tutorial_voice_012");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
                qs.set_memo_state(ctx, memo_state & !2097152);
            }
        }
        1048576 => {
            if level < 6 {
                show_question_mark(ctx, talker, 5);
                play_tutorial_voice(ctx, talker, "tutorial_voice_013");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
                qs.set_memo_state(ctx, memo_state & !1048576);
            }
        }
        8388608 => {
            if level < 6 {
                play_tutorial_voice(ctx, talker, "tutorial_voice_018");
                show_tutorial_html(ctx, talker, "tutorial-21z.html");
                qs.set_memo_state(ctx, memo_state & !8388608);
                enable_tutorial_event(ctx, qs, memo_flag & !8388608);
            }
        }
        _ => {}
    }
}

/// The level pages of a quest that is not done yet: `!haveMemo || oneTimeFlag == 0`.
fn level_page(ctx: &mut ScriptCtx<'_>, player: ObjectId, quest_id: i32, page: &str, radar: (i32, i32, i32)) {
    if !ctx.have_memo(player, quest_id) || ctx.get_one_time_quest_flag(player, quest_id) == 0 {
        show_tutorial_html(ctx, player, page);
        ctx.play_sound(player, QuestSound::ItemsoundQuestTutorial);
        ctx.add_radar(player, radar.0, radar.1, radar.2);
    }
}

/// Java `levelUp(player, level)`.
fn level_up(ctx: &mut ScriptCtx<'_>, player: ObjectId, level: i32) {
    match level {
        5 => tutorial_event(ctx, player, 1024),
        6 => tutorial_event(ctx, player, 134217728),
        7 => tutorial_event(ctx, player, 2048),
        9 => tutorial_event(ctx, player, 268435456),
        10 => tutorial_event(ctx, player, 536870912),
        12 => tutorial_event(ctx, player, 1073741824),
        15 => tutorial_event(ctx, player, 67108864),
        18 => {
            tutorial_event(ctx, player, 4096);
            level_page(
                ctx,
                player,
                Q10276_MUTATED_KANEUS_GLUDIO,
                "tw-gludio.html",
                (-13900, 123822, -3112),
            );
        }
        28 => level_page(
            ctx,
            player,
            Q10277_MUTATED_KANEUS_DION,
            "tw-dion.html",
            (18199, 146081, -3080),
        ),
        35 => tutorial_event(ctx, player, 16777216),
        36 => tutorial_event(ctx, player, 32),
        38 => level_page(
            ctx,
            player,
            Q10278_MUTATED_KANEUS_HEINE,
            "tw-heine.html",
            (108384, 221563, -3592),
        ),
        39 => {
            if ctx.player_race(player) == race::KAMAEL {
                tutorial_event(ctx, player, 16384);
            }
        }
        48 => level_page(
            ctx,
            player,
            Q10279_MUTATED_KANEUS_OREN,
            "tw-oren.html",
            (81023, 56456, -1552),
        ),
        58 => level_page(
            ctx,
            player,
            Q10280_MUTATED_KANEUS_SCHUTTGART,
            "tw-schuttgart.html",
            (85868, -142164, -1342),
        ),
        61 => tutorial_event(ctx, player, 64),
        68 => level_page(
            ctx,
            player,
            Q10281_MUTATED_KANEUS_RUNE,
            "tw-rune.html",
            (42596, -47988, -800),
        ),
        73 => tutorial_event(ctx, player, 128),
        79 => level_page(
            ctx,
            player,
            Q192_SEVEN_SIGNS_SERIES_OF_DOUBT,
            "tutorial-ss-79.html",
            (81655, 54736, -1509),
        ),
        81 => level_page(
            ctx,
            player,
            Q10292_SEVEN_SIGNS_GIRL_OF_DOUBT,
            "tutorial-ss-81.html",
            (146995, 23755, -1984),
        ),
        _ => {}
    }
}

/// Java `selectFromMenu(talker, reply)`: the `bypass` links of the tutorial window.
fn select_from_menu(ctx: &mut ScriptCtx<'_>, talker: ObjectId, reply: i32) {
    let page = match reply {
        1 => "tutorial-22g.html",
        2 => "tutorial-22w.html",
        3 => "tutorial-22ap.html",
        4 => "tutorial-22ad.html",
        5 => "tutorial-22bt.html",
        6 => "tutorial-22bh.html",
        7 => "tutorial-22cs.html",
        8 => "tutorial-22cn.html",
        9 => "tutorial-22cw.html",
        10 => "tutorial-22db.html",
        11 => "tutorial-22dp.html",
        12 => "tutorial-22et.html",
        13 => "tutorial-22es.html",
        14 => "tutorial-22fp.html",
        15 => "tutorial-22fs.html",
        16 => "tutorial-22gs.html",
        17 => "tutorial-22ge.html",
        18 => "tutorial-22ko.html",
        19 => "tutorial-22kw.html",
        20 => "tutorial-22ns.html",
        21 => "tutorial-22nb.html",
        22 => "tutorial-22oa.html",
        23 => "tutorial-22op.html",
        24 => "tutorial-22ps.html",
        25 => "tutorial-22pp.html",
        26 => match ctx.player_class_id(talker) {
            pc::WARRIOR => "tutorial-22.html",
            pc::KNIGHT => "tutorial-22a.html",
            pc::ROGUE => "tutorial-22b.html",
            pc::WIZARD => "tutorial-22c.html",
            pc::CLERIC => "tutorial-22d.html",
            pc::ELVEN_KNIGHT => "tutorial-22e.html",
            pc::ELVEN_SCOUT => "tutorial-22f.html",
            pc::ELVEN_WIZARD => "tutorial-22g.html",
            pc::ORACLE => "tutorial-22h.html",
            pc::ORC_RAIDER => "tutorial-22i.html",
            pc::ORC_MONK => "tutorial-22j.html",
            pc::ORC_SHAMAN => "tutorial-22k.html",
            pc::SCAVENGER => "tutorial-22l.html",
            pc::ARTISAN => "tutorial-22m.html",
            pc::PALUS_KNIGHT => "tutorial-22n.html",
            pc::ASSASSIN => "tutorial-22o.html",
            pc::DARK_WIZARD => "tutorial-22p.html",
            pc::SHILLIEN_ORACLE => "tutorial-22q.html",
            _ => "tutorial-22qe.html",
        },
        27 => "tutorial-29.html",
        28 => "tutorial-28.html",
        29 => "tutorial-07a.html",
        30 => "tutorial-07b.html",
        31 => match ctx.player_class_id(talker) {
            pc::TROOPER => "tutorial-28a.html",
            pc::WARDER => "tutorial-28b.html",
            _ => return,
        },
        32 => "tutorial-22qa.html",
        33 => match ctx.player_class_id(talker) {
            pc::TROOPER => "tutorial-22qb.html",
            pc::WARDER => "tutorial-22qc.html",
            _ => return,
        },
        34 => "tutorial-22qd.html",
        _ => return,
    };
    show_tutorial_html(ctx, talker, page);
}

/// Java `questionMarkClicked(talker, question_id)`.
fn question_mark_clicked(ctx: &mut ScriptCtx<'_>, talker: ObjectId, question_id: i32) {
    // `talker.getQuestState(...)`: the world sends `QM:` only to a player with the state.
    let Some(qs) = ctx.get_quest_state(talker, false) else {
        return;
    };

    let memo_flag = qs.get_memo_state(ctx) & MEMO_MASK;
    let class = ctx.player_class_id(talker);
    let player_race = ctx.player_race(talker);
    match question_id {
        1 => {
            play_tutorial_voice(ctx, talker, "tutorial_voice_007");
            if qs.get_memo_state_ex(ctx, 1) <= 0 {
                qs.set_memo_state_ex(ctx, 1, -5);
            }

            let radar = match class {
                pc::FIGHTER => Some((-71424, 258336, -3109)),
                pc::MAGE => Some((-91036, 248044, -3568)),
                // NOTE(java-quirk): the elves get the human mage's place.
                pc::ELVEN_FIGHTER | pc::ELVEN_MAGE => Some((-91036, 248044, -3568)),
                pc::DARK_FIGHTER | pc::DARK_MAGE => Some((28384, 11056, -4233)),
                pc::ORC_FIGHTER | pc::ORC_MAGE => Some((-56736, -113680, -672)),
                pc::DWARVEN_FIGHTER => Some((108567, -173994, -406)),
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => Some((-125872, 38016, 1251)),
                _ => None,
            };
            if let Some((x, y, z)) = radar {
                show_tutorial_html(ctx, talker, "tutorial-human-fighter-007.html");
                ctx.add_radar(talker, x, y, z);
            }

            qs.set_memo_state(ctx, memo_flag | 2);
        }
        2 => {
            let page = match class {
                pc::FIGHTER => Some("tutorial-human-fighter-008.html"),
                pc::MAGE => Some("tutorial-human-mage-008.html"),
                pc::ELVEN_FIGHTER | pc::ELVEN_MAGE => Some("tutorial-elf-008.html"),
                pc::DARK_FIGHTER | pc::DARK_MAGE => Some("tutorial-delf-008.html"),
                pc::ORC_FIGHTER | pc::ORC_MAGE => Some("tutorial-orc-008.html"),
                pc::DWARVEN_FIGHTER => Some("tutorial-dwarven-fighter-008.html"),
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => Some("tutorial-kamael-008.html"),
                _ => None,
            };
            if let Some(page) = page {
                show_tutorial_html(ctx, talker, page);
            }

            qs.set_memo_state(ctx, memo_flag | 2);
        }
        3 => {
            show_tutorial_html(ctx, talker, "tutorial-09.html");
            enable_tutorial_event(ctx, qs, memo_flag | 1048576);
            // TODO find better way!
            let memo = qs.get_memo_state(ctx);
            qs.set_memo_state(ctx, memo | 1048576);
        }
        4 => show_tutorial_html(ctx, talker, "tutorial-10.html"),
        5 => {
            match class {
                pc::FIGHTER => ctx.add_radar(talker, -71424, 258336, -3109),
                pc::MAGE => ctx.add_radar(talker, -91036, 248044, -3568),
                pc::ELVEN_FIGHTER | pc::ELVEN_MAGE => ctx.add_radar(talker, 46112, 41200, -3504),
                pc::DARK_FIGHTER | pc::DARK_MAGE => ctx.add_radar(talker, 28384, 11056, -4233),
                pc::ORC_FIGHTER | pc::ORC_MAGE => ctx.add_radar(talker, -56736, -113680, -672),
                pc::DWARVEN_FIGHTER => ctx.add_radar(talker, 108567, -173994, -406),
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => ctx.add_radar(talker, -125872, 38016, 1251),
                _ => {}
            }

            show_tutorial_html(ctx, talker, "tutorial-11.html");
        }
        7 => {
            show_tutorial_html(ctx, talker, "tutorial-15.html");
            qs.set_memo_state(ctx, memo_flag | 5);
        }
        8 => show_tutorial_html(ctx, talker, "tutorial-18.html"),
        9 => {
            if !ctx.is_mage_class(talker) {
                match player_race {
                    race::HUMAN | race::ELF | race::DARK_ELF => {
                        show_tutorial_html(ctx, talker, "tutorial-fighter-017.html");
                    }
                    race::DWARF => show_tutorial_html(ctx, talker, "tutorial-fighter-dwarf-017.html"),
                    race::ORC => show_tutorial_html(ctx, talker, "tutorial-fighter-orc-017.html"),
                    race::KAMAEL => show_tutorial_html(ctx, talker, "tutorial-kamael-017.html"),
                    _ => {}
                }
            }
        }
        10 => show_tutorial_html(ctx, talker, "tutorial-19.html"),
        11 => match player_race {
            race::HUMAN => show_tutorial_html(ctx, talker, "tutorial-mage-020.html"),
            race::ELF | race::DARK_ELF => show_tutorial_html(ctx, talker, "tutorial-mage-elf-020.html"),
            race::ORC => show_tutorial_html(ctx, talker, "tutorial-mage-orc-020.html"),
            _ => {}
        },
        12 => show_tutorial_html(ctx, talker, "tutorial-15.html"),
        13 => {
            let page = match class {
                pc::FIGHTER => Some("tutorial-21.html"),
                pc::MAGE => Some("tutorial-21a.html"),
                pc::ELVEN_FIGHTER => Some("tutorial-21b.html"),
                pc::ELVEN_MAGE => Some("tutorial-21c.html"),
                pc::ORC_FIGHTER => Some("tutorial-21d.html"),
                pc::ORC_MAGE => Some("tutorial-21e.html"),
                pc::DWARVEN_FIGHTER => Some("tutorial-21f.html"),
                pc::DARK_FIGHTER => Some("tutorial-21g.html"),
                pc::DARK_MAGE => Some("tutorial-21h.html"),
                pc::MALE_SOLDIER => Some("tutorial-21i.html"),
                pc::FEMALE_SOLDIER => Some("tutorial-21j.html"),
                _ => None,
            };
            if let Some(page) = page {
                show_tutorial_html(ctx, talker, page);
            }
        }
        15 => {
            if player_race != race::KAMAEL {
                show_tutorial_html(ctx, talker, "tutorial-28.html");
            } else if class == pc::TROOPER {
                show_tutorial_html(ctx, talker, "tutorial-28a.html");
            } else if class == pc::WARDER {
                show_tutorial_html(ctx, talker, "tutorial-28b.html");
            }
        }
        16 => show_tutorial_html(ctx, talker, "tutorial-30.html"),
        17 => show_tutorial_html(ctx, talker, "tutorial-27.html"),
        19 => show_tutorial_html(ctx, talker, "tutorial-07.html"),
        20 => show_tutorial_html(ctx, talker, "tutorial-14.html"),
        21 => show_tutorial_html(ctx, talker, "tutorial-newbie-001.html"),
        22 => show_tutorial_html(ctx, talker, "tutorial-14.html"),
        23 => show_tutorial_html(ctx, talker, "tutorial-24.html"),
        24 => {
            let page = match player_race {
                race::HUMAN => Some("tutorial-newbie-003a.html"),
                race::ELF => Some("tutorial-newbie-003b.html"),
                race::DARK_ELF => Some("tutorial-newbie-003c.html"),
                race::ORC => Some("tutorial-newbie-003d.html"),
                race::DWARF => Some("tutorial-newbie-003e.html"),
                race::KAMAEL => Some("tutorial-newbie-003f.html"),
                _ => None,
            };
            if let Some(page) = page {
                show_tutorial_html(ctx, talker, page);
            }
        }
        25 => {
            let page = match class {
                pc::FIGHTER => Some("tutorial-newbie-002a.html"),
                pc::MAGE => Some("tutorial-newbie-002b.html"),
                pc::ELVEN_FIGHTER | pc::ELVEN_MAGE => Some("tutorial-newbie-002c.html"),
                pc::DARK_MAGE => Some("tutorial-newbie-002d.html"),
                pc::DARK_FIGHTER => Some("tutorial-newbie-002e.html"),
                pc::DWARVEN_FIGHTER => Some("tutorial-newbie-002g.html"),
                pc::ORC_FIGHTER | pc::ORC_MAGE => Some("tutorial-newbie-002f.html"),
                pc::MALE_SOLDIER | pc::FEMALE_SOLDIER => Some("tutorial-newbie-002i.html"),
                _ => None,
            };
            if let Some(page) = page {
                show_tutorial_html(ctx, talker, page);
            }
        }
        26 => {
            if !ctx.is_mage_class(talker) || class == pc::ORC_MAGE {
                show_tutorial_html(ctx, talker, "tutorial-newbie-004a.html");
            } else {
                show_tutorial_html(ctx, talker, "tutorial-newbie-004b.html");
            }
        }
        27 => {
            if matches!(class, pc::FIGHTER | pc::ORC_MAGE | pc::ORC_FIGHTER) {
                show_tutorial_html(ctx, talker, "tutorial-newbie-002h.html");
            }
        }
        28 => show_tutorial_html(ctx, talker, "tutorial-31.html"),
        29 => show_tutorial_html(ctx, talker, "tutorial-32.html"),
        30 => show_tutorial_html(ctx, talker, "tutorial-33.html"),
        31 => show_tutorial_html(ctx, talker, "tutorial-34.html"),
        32 => show_tutorial_html(ctx, talker, "tutorial-35.html"),
        33 => {
            let page = match ctx.player_level(talker) {
                18 => Some("tw-gludio.html"),
                28 => Some("tw-dion.html"),
                38 => Some("tw-heine.html"),
                48 => Some("tw-oren.html"),
                // NOTE(java-quirk): "shuttgart", while `levelUp` shows "tw-schuttgart.html".
                58 => Some("tw-shuttgart.html"),
                68 => Some("tw-rune.html"),
                _ => None,
            };
            if let Some(page) = page {
                show_tutorial_html(ctx, talker, page);
            }
        }
        34 => {
            if ctx.player_level(talker) == 79 {
                show_tutorial_html(ctx, talker, "tutorial-ss-79.html");
            }
        }
        _ => {}
    }
}

/// Java `userConnected(talker)`: the state is started, and below level 6 the tutorial goes on
/// where it was; from level 6 on, the level marks and the territory war memos.
fn user_connected(ctx: &mut ScriptCtx<'_>, talker: ObjectId) {
    let Some(qs) = ctx.get_quest_state(talker, true) else {
        return;
    };

    if !qs.is_started(ctx) {
        qs.set_state(ctx, STARTED);
    }

    if ctx.player_level(talker) < 6 {
        if ctx.get_one_time_quest_flag(talker, 255) != 0 {
            return;
        }

        let mut memo_state = qs.get_memo_state(ctx);
        let memo_flag;
        if memo_state == -1 {
            memo_state = 0;
            memo_flag = 0;
        } else {
            memo_flag = memo_state & 255;
            memo_state &= MEMO_MASK;
        }

        match memo_flag {
            0 => {
                start_player_timer(ctx, qs.player(), 5000);
                memo_state = MEMO_MASK & !(8388608 | 1048576);
                qs.set_memo_state(ctx, 1 | memo_state);
                if qs.get_memo_state_ex(ctx, 1) <= 0 {
                    qs.set_memo_state_ex(ctx, 1, -2);
                }
            }
            1 => {
                show_question_mark(ctx, talker, 1);
                play_tutorial_voice(ctx, talker, "tutorial_voice_006");
                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
            }
            2 => {
                let race_tutorial = [
                    Q201_TUTORIAL_HUMAN_FIGHTER,
                    Q202_TUTORIAL_HUMAN_MAGE,
                    Q203_TUTORIAL_ELF,
                    Q204_TUTORIAL_DARK_ELF,
                    Q205_TUTORIAL_ORC,
                    Q206_TUTORIAL_DWARF,
                ]
                .iter()
                .any(|&quest| ctx.have_memo(talker, quest));
                if race_tutorial {
                    mark(ctx, talker, 6);
                } else {
                    mark(ctx, talker, 2);
                }
            }
            3 => {
                let mut state_mark = 1;
                if ctx.get_quest_items_count(talker, BLUE_GEMSTONE) == 1 {
                    state_mark = 3;
                } else if qs.get_memo_state_ex(ctx, 1) == 2 {
                    state_mark = 2;
                }

                match state_mark {
                    1 => show_question_mark(ctx, talker, 3),
                    2 => show_question_mark(ctx, talker, 4),
                    3 => show_question_mark(ctx, talker, 5),
                    _ => {}
                }

                ctx.play_sound(talker, QuestSound::ItemsoundQuestTutorial);
            }
            4 => mark(ctx, talker, 12),
            _ => {}
        }

        enable_tutorial_event(ctx, qs, memo_state);
    } else {
        let level_quest = match ctx.player_level(talker) {
            18 => Some((10276, 33)),
            28 => Some((10277, 33)),
            38 => Some((10278, 33)),
            48 => Some((10279, 33)),
            58 => Some((10280, 33)),
            68 => Some((10281, 33)),
            79 => Some((192, 34)),
            _ => None,
        };
        if let Some((quest, question_mark)) = level_quest
            && ctx.have_memo(talker, quest)
            && ctx.get_one_time_quest_flag(talker, quest) == 0
        {
            mark(ctx, talker, question_mark);
        }

        let territory_war_id = get_dominion_siege_id(ctx, talker);
        let territory_war_state = ctx.get_nr_memo_state_ex(qs, 728, 1);

        if territory_war_id > 0 && get_dominion_war_state(ctx, territory_war_id) == 5 {
            if !ctx.have_nr_memo(qs, 728) {
                ctx.set_nr_memo(qs, 728);
                ctx.set_nr_memo_state(qs, 728, 0);
                ctx.set_nr_memo_state_ex(qs, 728, 1, territory_war_id);
            } else if territory_war_id != territory_war_state {
                ctx.set_nr_memo_state(qs, 728, 0);
                ctx.set_nr_memo_state_ex(qs, 728, 1, territory_war_id);
            }

            let territory_quest = match territory_war_id {
                81 => Some((TW_GLUDIO, Q717_FOR_THE_SAKE_OF_THE_TERRITORY_GLUDIO)),
                82 => Some((TW_DION, Q718_FOR_THE_SAKE_OF_THE_TERRITORY_DION)),
                83 => Some((TW_GIRAN, Q719_FOR_THE_SAKE_OF_THE_TERRITORY_GIRAN)),
                84 => Some((TW_OREN, Q720_FOR_THE_SAKE_OF_THE_TERRITORY_OREN)),
                85 => Some((TW_ADEN, Q721_FOR_THE_SAKE_OF_THE_TERRITORY_ADEN)),
                86 => Some((TW_HEINE, Q722_FOR_THE_SAKE_OF_THE_TERRITORY_INNADRIL)),
                87 => Some((TW_GODDARD, Q723_FOR_THE_SAKE_OF_THE_TERRITORY_GODDARD)),
                88 => Some((TW_RUNE, Q724_FOR_THE_SAKE_OF_THE_TERRITORY_RUNE)),
                89 => Some((TW_SCHUTTGART, Q725_FOR_THE_SAKE_OF_THE_TERRITORY_SCHUTTGART)),
                _ => None,
            };
            if let Some((territory, quest)) = territory_quest
                && get_dominion_war_state(ctx, territory) == 5
            {
                if !ctx.have_nr_memo(qs, quest) {
                    ctx.set_nr_memo(qs, quest);
                    ctx.set_nr_memo_state(qs, quest, 0);
                    ctx.set_nr_flag_journal(qs, quest, 1);
                }
                show_question_mark(ctx, talker, quest);
                ctx.play_sound(talker, QuestSound::ItemsoundQuestMiddle);
            }
        } else {
            if ctx.have_nr_memo(qs, Q728_TERRITORY_WAR) && (81..=89).contains(&territory_war_state) {
                let tw_nr_state = ctx.get_nr_memo_state(qs, Q728_TERRITORY_WAR);
                let tw_nr_state_for_current_war = ctx.get_nr_memo_state(qs, 636 + territory_war_state);
                if tw_nr_state_for_current_war >= 0 {
                    ctx.set_nr_memo_state(qs, Q728_TERRITORY_WAR, tw_nr_state_for_current_war + tw_nr_state);
                    ctx.remove_nr_memo(qs, 636 + territory_war_state);
                }
            }

            if ctx.have_nr_memo(qs, 739) && ctx.get_nr_memo_state(qs, 739) > 0 {
                ctx.set_nr_memo_state(qs, 739, 0);
            }

            for quest in [
                Q729_PROTECT_THE_TERRITORY_CATAPULT,
                Q730_PROTECT_THE_SUPPLIES_SAFE,
                Q731_PROTECT_THE_MILITARY_ASSOCIATION_LEADER,
                Q732_PROTECT_THE_RELIGIOUS_ASSOCIATION_LEADER,
                Q733_PROTECT_THE_ECONOMIC_ASSOCIATION_LEADER,
            ] {
                if ctx.have_nr_memo(qs, quest) {
                    ctx.remove_nr_memo(qs, quest);
                }
            }
        }
    }
}

// ---------------------------------- Event

/// The beginner shots of the village teachers' event 31 (`eventGallin`, `eventJundin`, ...):
/// soulshots for a fighter, spiritshots (or the orc mage's soulshots) for a mage, while the
/// player has few of them.
fn teacher_shots(ctx: &mut ScriptCtx<'_>, talker: ObjectId, orc_mage_soulshots: bool, spiritshots: i64) {
    let soulshots = ctx.get_quest_items_count(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS);
    if !ctx.is_mage_class(talker) && soulshots <= 200 {
        play_tutorial_voice(ctx, talker, "tutorial_voice_026");
        ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
        ctx.add_exp_and_sp(talker, 0, 50);
    }

    if ctx.is_mage_class(talker)
        && ctx.get_quest_items_count(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS) <= 200
        && ctx.get_quest_items_count(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS) <= 100
    {
        if orc_mage_soulshots && ctx.player_class_id(talker) == pc::ORC_MAGE {
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
        } else {
            play_tutorial_voice(ctx, talker, "tutorial_voice_027");
            ctx.give_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS, spiritshots);
        }
        ctx.add_exp_and_sp(talker, 0, 50);
    }
}

/// `if (qs.getMemoStateEx(1) <= 3) qs.setMemoStateEx(1, 4);`
fn teacher_done(ctx: &mut ScriptCtx<'_>, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 3 {
        qs.set_memo_state_ex(ctx, 1, 4);
    }
}

/// Event 41 of the teachers but Roien: to the Kamael village, with a radar to the teacher.
fn teacher_teleport(ctx: &mut ScriptCtx<'_>, talker: ObjectId, page: &str) {
    ctx.teleport_player(talker, -120050, 44500, 360, 0, true);
    ctx.add_radar(talker, -119692, 44504, 380);
    ctx.show_html_file(talker, page, None);
}

fn event_roien(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, RECOMMENDATION_1) {
                if !ctx.is_mage_class(talker) && qs.get_memo_state_ex(ctx, 1) <= 3 {
                    ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
                    play_tutorial_voice(ctx, talker, "tutorial_voice_026");
                    ctx.add_exp_and_sp(talker, 0, 50);
                    qs.set_memo_state_ex(ctx, 1, 4);
                }

                if ctx.is_mage_class(talker) && qs.get_memo_state_ex(ctx, 1) <= 3 {
                    if ctx.player_class_id(talker) == pc::ORC_MAGE {
                        ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
                        play_tutorial_voice(ctx, talker, "tutorial_voice_026");
                    } else {
                        ctx.give_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS, 100);
                        play_tutorial_voice(ctx, talker, "tutorial_voice_027");
                    }

                    ctx.add_exp_and_sp(talker, 0, 50);
                    qs.set_memo_state_ex(ctx, 1, 4);
                }

                start_npc_timer(ctx, npc, talker, 60000);
                ctx.take_items(talker, RECOMMENDATION_1, 1);
                ctx.show_html_file(talker, "30008-002.html", None);
            }
        }
        41 => {
            // NOTE(java-quirk): Roien's teleport has no radar, the others' have one.
            ctx.teleport_player(talker, -120050, 44500, 360, 0, true);
            ctx.show_html_file(talker, "30008-005.html", None);
        }
        42 => {
            ctx.add_radar(talker, -84081, 243277, -3723);
            ctx.show_html_file(talker, "30008-006.html", None);
        }
        _ => {}
    }
}

fn event_gallin(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, RECOMMENDATION_2) {
                teacher_shots(ctx, talker, true, 200);
                ctx.take_items(talker, RECOMMENDATION_2, 1);
                start_npc_timer(ctx, npc, talker, 60000);
                teacher_done(ctx, qs);
                ctx.show_html_file(talker, "30017-002.html", None);
            }
        }
        41 => teacher_teleport(ctx, talker, "30017-005.html"),
        42 => {
            ctx.add_radar(talker, -84081, 243277, -3723);
            ctx.show_html_file(talker, "30017-006.html", None);
        }
        _ => {}
    }
}

fn event_jundin(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, BLOOD_OF_MITRAELL) {
                teacher_shots(ctx, talker, true, 200);
                ctx.take_items(talker, BLOOD_OF_MITRAELL, 1);
                start_npc_timer(ctx, npc, talker, 60000);
                teacher_done(ctx, qs);
                ctx.show_html_file(talker, "30129-002.html", None);
            }
        }
        41 => teacher_teleport(ctx, talker, "30129-005.html"),
        42 => {
            ctx.add_radar(talker, 17024, 13296, -3744);
            ctx.show_html_file(talker, "30129-006.html", None);
        }
        _ => {}
    }
}

fn event_nerupa(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, LEAF_OF_THE_MOTHER_TREE) {
                // No orc mages among the elves: always spiritshots.
                teacher_shots(ctx, talker, false, 200);
                ctx.take_items(talker, LEAF_OF_THE_MOTHER_TREE, 1);
                start_npc_timer(ctx, npc, talker, 60000);
                teacher_done(ctx, qs);
                ctx.show_html_file(talker, "30370-002.html", None);
            }
        }
        41 => teacher_teleport(ctx, talker, "30370-005.html"),
        42 => {
            ctx.add_radar(talker, 45475, 48359, -3060);
            ctx.show_html_file(talker, "30370-006.html", None);
        }
        _ => {}
    }
}

fn event_foreman_laferon(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, LICENSE_OF_MINER) {
                teacher_shots(ctx, talker, false, 100);
                ctx.take_items(talker, LICENSE_OF_MINER, 1);
                start_npc_timer(ctx, npc, talker, 60000);
                teacher_done(ctx, qs);
                ctx.show_html_file(talker, "30528-002.html", None);
            }
        }
        41 => teacher_teleport(ctx, talker, "30528-005.html"),
        42 => {
            ctx.add_radar(talker, 115632, -177996, -905);
            ctx.show_html_file(talker, "30528-006.html", None);
        }
        _ => {}
    }
}

fn event_guardian_vullkus(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    match event {
        31 => {
            if ctx.has_quest_items(talker, VOUCHER_OF_FLAME) {
                ctx.take_items(talker, VOUCHER_OF_FLAME, 1);
                start_npc_timer(ctx, npc, talker, 60000);
                teacher_done(ctx, qs);

                if ctx.get_quest_items_count(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS) <= 200 {
                    ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
                    ctx.add_exp_and_sp(talker, 0, 50);
                }

                play_tutorial_voice(ctx, talker, "tutorial_voice_026");
                ctx.show_html_file(talker, "30573-002.html", None);
            }
        }
        41 => teacher_teleport(ctx, talker, "30573-005.html"),
        42 => {
            ctx.add_radar(talker, -45032, -113598, -192);
            ctx.show_html_file(talker, "30573-006.html", None);
        }
        _ => {}
    }
}

fn event_subelder_perwan(ctx: &mut ScriptCtx<'_>, event: i32, talker: ObjectId, npc: ScriptNpc, qs: Qs) {
    if event == 31 && ctx.has_quest_items(talker, DIPLOMA) {
        if ctx.player_race(talker) == race::KAMAEL
            && ctx.player_class_level(talker) == 0
            && qs.get_memo_state_ex(ctx, 1) <= 3
        {
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
            ctx.add_exp_and_sp(talker, 0, 50);
            qs.set_memo_state_ex(ctx, 1, 4);
        }

        ctx.take_items(talker, DIPLOMA, -1);
        start_npc_timer(ctx, npc, talker, 60000);
        ctx.add_radar(talker, -119692, 44504, 380);
        ctx.show_html_file(talker, "32133-002.html", None);
    }
}

// ---------------------------------- Talks

/// The teachers' first talk (Roien, Gallin, Jundin, Nerupa, Laferon, Perwan): the letter
/// page, else after or before the helper step.
fn talk_teacher(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs, letter: i32, prefix: i32) {
    let page = |n: &str| format!("{prefix}-{n}.html");
    if ctx.has_quest_items(talker, letter) {
        ctx.show_html_file(talker, &page("001"), Some(npc));
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, &page("004"), Some(npc));
    } else if qs.get_memo_state_ex(ctx, 1) <= 3 {
        ctx.show_html_file(talker, &page("003"), Some(npc));
    }
}

fn talk_roien(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, RECOMMENDATION_1, ROIEN);
}

/// `memoStateEx(1)` is 0, 1 or 2: the helper step before the gemstone is brought.
fn helper_step(ctx: &ScriptCtx<'_>, qs: Qs) -> bool {
    matches!(qs.get_memo_state_ex(ctx, 1), 0..=2)
}

/// The start of a newbie helper's step for the right race (and class): the radar to the helper
/// goes and the helper's timer starts.
fn helper_start(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, radar: (i32, i32, i32)) {
    ctx.remove_radar(talker, radar.0, radar.1, radar.2);
    start_npc_timer(ctx, npc, talker, 30000);
}

/// `enableTutorialEvent(qs, (qs.getMemoState() & 2147483392) | 1048576)`.
fn enable_gemstone_event(ctx: &mut ScriptCtx<'_>, qs: Qs) {
    let memo = qs.get_memo_state(ctx);
    enable_tutorial_event(ctx, qs, (memo & MEMO_MASK) | 1048576);
}

/// The gemstone is brought: it goes, `memoStateEx(1)` 3, the letter, the helper's timer and
/// memo bit 4.
fn helper_gemstone(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs, letter: i32) {
    ctx.take_items(talker, BLUE_GEMSTONE, -1);
    qs.set_memo_state_ex(ctx, 1, 3);
    ctx.give_items(talker, letter, 1);
    start_npc_timer(ctx, npc, talker, 30000);
    let memo = qs.get_memo_state(ctx);
    qs.set_memo_state(ctx, (memo & MEMO_MASK) | 4);
}

/// The helpers' beginner shots when the player has none yet. Carl and Doff give the fighter's
/// soulshots before the voice, the others after it (`voice_first`).
fn helper_shots(ctx: &mut ScriptCtx<'_>, talker: ObjectId, voice_first: bool, orc_mage_soulshots: i64) {
    if !ctx.is_mage_class(talker) && !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS) {
        if voice_first {
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
        } else {
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
        }
    }

    if ctx.is_mage_class(talker)
        && !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS)
        && !ctx.has_quest_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS)
    {
        if ctx.player_class_id(talker) == pc::ORC_MAGE {
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, orc_mage_soulshots);
        } else {
            play_tutorial_voice(ctx, talker, "tutorial_voice_027");
            ctx.give_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS, 100);
        }
    }
}

fn talk_carl(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_class_id(talker) == pc::FIGHTER && ctx.player_race(talker) == race::HUMAN {
            helper_start(ctx, npc, talker, (-71424, 258336, -3109));
            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
            ctx.show_html_file(talker, "30009-001.html", None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        ctx.show_html_file(talker, "30009-002.html", None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        helper_gemstone(ctx, npc, talker, qs, RECOMMENDATION_1);
        helper_shots(ctx, talker, false, 200);
        ctx.show_html_file(talker, "30009-003.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30009-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_gallin(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, RECOMMENDATION_2, GALLINT);
}

fn talk_doff(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_class_id(talker) == pc::MAGE && ctx.player_race(talker) == race::HUMAN {
            helper_start(ctx, npc, talker, (-91036, 248044, -3568));
            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
            ctx.show_html_file(talker, "30019-001.html", None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        ctx.show_html_file(talker, "30019-002.html", None);
    }

    // NOTE(java-quirk): an `if`, not an `else if`: the first talk of the step above goes on
    // here (`memoStateEx(1)` is 0 now) and takes a gemstone the player already has.
    if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        helper_gemstone(ctx, npc, talker, qs, RECOMMENDATION_2);
        helper_shots(ctx, talker, false, 200);
        ctx.show_html_file(talker, "30019-003.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30019-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_jundin(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, BLOOD_OF_MITRAELL, MITRAELL);
}

/// The fighter or mage page of a helper.
fn fighter_or_mage<'a>(ctx: &ScriptCtx<'_>, talker: ObjectId, fighter: &'a str, mage: &'a str) -> &'a str {
    if !ctx.is_mage_class(talker) { fighter } else { mage }
}

fn talk_poeny(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_race(talker) == race::DARK_ELF {
            helper_start(ctx, npc, talker, (28384, 11056, -4233));

            let page = fighter_or_mage(ctx, talker, "30009-001.html", "30019-001.html");
            ctx.show_html_file(talker, page, None);

            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        let page = fighter_or_mage(ctx, talker, "30009-002.html", "30019-002.html");
        ctx.show_html_file(talker, page, None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        let page = fighter_or_mage(ctx, talker, "30131-003f.html", "30131-003m.html");
        ctx.show_html_file(talker, page, None);

        helper_gemstone(ctx, npc, talker, qs, BLOOD_OF_MITRAELL);
        // NOTE(java-quirk): the dark elf helper gives an orc mage 100 soulshots (the others
        // give 200); no dark elf is an orc mage.
        helper_shots(ctx, talker, true, 100);
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30131-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_nerupa(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, LEAF_OF_THE_MOTHER_TREE, NERUPA);
}

fn talk_mother_temp(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_race(talker) == race::ELF {
            helper_start(ctx, npc, talker, (46112, 41200, -3504));
            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
            let page = fighter_or_mage(ctx, talker, "30009-001.html", "30019-001.html");
            ctx.show_html_file(talker, page, None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        let page = fighter_or_mage(ctx, talker, "30009-002.html", "30019-002.html");
        ctx.show_html_file(talker, page, None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        helper_gemstone(ctx, npc, talker, qs, LEAF_OF_THE_MOTHER_TREE);
        helper_shots(ctx, talker, true, 200);
        let page = fighter_or_mage(ctx, talker, "30400-003f.html", "30400-003m.html");
        ctx.show_html_file(talker, page, None);
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30400-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_foreman_laferon(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, LICENSE_OF_MINER, LAFERON);
}

fn talk_miner_mai(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_race(talker) == race::DWARF {
            helper_start(ctx, npc, talker, (108567, -173994, -406));
            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
            let page = fighter_or_mage(ctx, talker, "30009-001.html", "30019-001.html");
            ctx.show_html_file(talker, page, None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        ctx.show_html_file(talker, "30009-002.html", None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        helper_gemstone(ctx, npc, talker, qs, LICENSE_OF_MINER);
        if !ctx.is_mage_class(talker) && !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS) {
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
        }

        if ctx.is_mage_class(talker)
            && !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS)
            && !ctx.has_quest_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS)
        {
            if ctx.player_class_id(talker) == pc::ORC_MAGE {
                play_tutorial_voice(ctx, talker, "tutorial_voice_026");
                ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            } else {
                play_tutorial_voice(ctx, talker, "tutorial_voice_027");
                ctx.give_items(talker, SPIRITSHOT_NO_GRADE_FOR_BEGINNERS, 100);
            }
        } else {
            // NOTE(java-quirk): the page is the `else` of the mage shots, so a mage who gets
            // spiritshots sees no page.
            ctx.show_html_file(talker, "30530-003.html", None);
        }
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30530-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_guardian_vullkus(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if ctx.has_quest_items(talker, VOUCHER_OF_FLAME) {
        ctx.show_html_file(talker, "30573-001.html", Some(npc));
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30573-004.html", Some(npc));
    } else {
        ctx.show_html_file(talker, "30573-003.html", Some(npc));
    }
}

fn talk_shela_priestess(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_race(talker) == race::ORC {
            helper_start(ctx, npc, talker, (-56736, -113680, -672));
            qs.set_memo_state_ex(ctx, 1, 0);
            enable_gemstone_event(ctx, qs);
            let page = fighter_or_mage(ctx, talker, "30009-001.html", "30575-001.html");
            ctx.show_html_file(talker, page, None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        let page = fighter_or_mage(ctx, talker, "30009-002.html", "30575-002.html");
        ctx.show_html_file(talker, page, None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        let page = fighter_or_mage(ctx, talker, "30575-003f.html", "30575-003m.html");
        ctx.show_html_file(talker, page, None);

        helper_gemstone(ctx, npc, talker, qs, VOUCHER_OF_FLAME);
        if !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS) {
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
        }

        play_tutorial_voice(ctx, talker, "tutorial_voice_026");
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "30575-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "30009-005.html", None);
    }
}

fn talk_subelder_perwan(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    talk_teacher(ctx, npc, talker, qs, DIPLOMA, PERWAN);
}

fn talk_helper_krenisk(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId, qs: Qs) {
    if qs.get_memo_state_ex(ctx, 1) <= 0 {
        if ctx.player_race(talker) == race::KAMAEL {
            ctx.remove_radar(talker, -125872, 38016, 1251);
            qs.set_memo_state_ex(ctx, 1, 0);
            start_npc_timer(ctx, npc, talker, 30000);
            enable_gemstone_event(ctx, qs);
            ctx.show_html_file(talker, "32134-001.html", None);
        } else {
            ctx.show_html_file(talker, "30009-006.html", None);
        }
    } else if helper_step(ctx, qs) && !ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        ctx.show_html_file(talker, "32134-002.html", None);
    } else if helper_step(ctx, qs) && ctx.has_quest_items(talker, BLUE_GEMSTONE) {
        ctx.show_html_file(talker, "32134-003.html", None);
        helper_gemstone(ctx, npc, talker, qs, DIPLOMA);
        if ctx.player_race(talker) == race::KAMAEL
            && ctx.player_class_level(talker) == 0
            && !ctx.has_quest_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS)
        {
            ctx.give_items(talker, SOULSHOT_NO_GRADE_FOR_BEGINNERS, 200);
            play_tutorial_voice(ctx, talker, "tutorial_voice_026");
        }
    } else if qs.get_memo_state_ex(ctx, 1) == 3 {
        ctx.show_html_file(talker, "32134-004.html", None);
    } else if qs.get_memo_state_ex(ctx, 1) > 3 {
        ctx.show_html_file(talker, "32134-005.html", None);
    }
}

/// Java `showQuestionMark(talker, number)`.
fn show_question_mark(ctx: &mut ScriptCtx<'_>, talker: ObjectId, number: i32) {
    ctx.world_mut()
        .send(talker, &TutorialShowQuestionMark { mark_id: number });
}

/// Java `playTutorialVoice(player, voice)`: `PlaySound(2, voice, 0, 0, x, y, z)`.
fn play_tutorial_voice(ctx: &mut ScriptCtx<'_>, player: ObjectId, voice: &str) {
    let (x, y, z) = ctx.world().position_of(player).unwrap_or((0, 0, 0));
    let packet = PlaySound {
        unknown1: 2,
        sound_file: voice.to_owned(),
        unknown3: 0,
        unknown4: 0,
        unknown5: x,
        unknown6: y,
        unknown7: z,
        unknown8: 0,
    };
    ctx.world_mut().send(player, &packet);
}

/// Java `showTutorialHTML(player, fileName)`: a page of this script in the tutorial window.
fn show_tutorial_html(ctx: &mut ScriptCtx<'_>, player: ObjectId, file_name: &str) {
    if let Some(content) = ctx.get_htm(player, file_name) {
        ctx.world_mut().send_tutorial_html(player, &content);
    }
}

/// Java `closeTutorialHtml(player)`.
fn close_tutorial_html(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().send_tutorial_close_html(player);
}

/// Java `getDominionSiegeID(talker)`: the territory the player is registered for.
fn get_dominion_siege_id(ctx: &ScriptCtx<'_>, talker: ObjectId) -> i32 {
    ctx.world().registered_territory_id(talker)
}

/// Java `getDominionWarState(castleId)`: 5 while the territory war runs.
fn get_dominion_war_state(ctx: &ScriptCtx<'_>, _castle_id: i32) -> i32 {
    if ctx.world().is_tw_in_progress() { 5 } else { 0 }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::QuestInput;
    use l2_world::{ClientInput, ScriptRegistry, ScriptSettings};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00255_Tutorial";

    const NPCS_XML: &str = r#"
        <npc id="30008" type="Folk" level="70" name="Roien"/>
        <npc id="30009" type="Folk" level="70" name="Newbie Helper"/>
        <npc id="18342" type="Monster" level="1" name="Tutorial Gremlin"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade for Beginners"><set name="is_stackable" val="true"/></item>
        <item id="5790" type="EtcItem" name="Spiritshot: No Grade for Beginners"><set name="is_stackable" val="true"/></item>
        <item id="6353" type="EtcItem" name="Blue Gemstone"/>
        <item id="5588" type="EtcItem" name="Tutorial Guide"/>
        <item id="1067" type="EtcItem" name="Recommendation (1)"><set name="is_questitem" val="true"/></item>"#;

    /// `<html>` pages: the tutorial window's (`tutorial-*.html`) and the helpers'.
    fn pages() -> Pages {
        Pages::of(&Q00255Tutorial)
            .plain(&[
                "tutorial-human-fighter-001.html",
                "tutorial-human-fighter-007.html",
                "tutorial-03.html",
                "30009-001.html",
                "30009-002.html",
                "30009-003.html",
                "30009-004.html",
                "30009-006.html",
                "30008-002.html",
            ])
            .page("30008-001.html", &["31"])
            .npcs(&[ROIEN, NEWBIE_HELPER_HUMAN_FIGHTER])
    }

    fn setup(settings: ScriptSettings) -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00255Tutorial);
        let mut t = ScriptTest::with_pages(registry, NPCS_XML, ITEMS, pages());
        t.world.set_script_settings(settings);
        let roien = t.spawn(ROIEN, 20);
        let carl = t.spawn(NEWBIE_HELPER_HUMAN_FIGHTER, 40);
        (t, roien, carl)
    }

    fn quest_input(t: &mut ScriptTest, input: QuestInput) {
        t.input(1, ClientInput::Quest(input));
    }

    fn memo(t: &mut ScriptTest, slot: Option<i32>) -> i32 {
        let mut value = 0;
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            value = match slot {
                None => qs.get_memo_state(ctx),
                Some(slot) => qs.get_memo_state_ex(ctx, slot),
            };
        });
        value
    }

    /// The `TutorialShowHtml` pages sent to player 1.
    fn tutorial_pages(out: &crate::testing::Sent) -> Vec<String> {
        out.iter()
            .filter(|(to, b)| *to == 1 && crate::testing::name_of(b) == "TUTORIAL_SHOW_HTML")
            .map(|(_, b)| crate::testing::decode::<l2_protocol_game::server::TutorialShowHtml>(b).html)
            .collect()
    }

    /// The question marks sent to player 1.
    fn marks(out: &crate::testing::Sent) -> Vec<i32> {
        out.iter()
            .filter(|(to, b)| *to == 1 && crate::testing::name_of(b) == "SHOW_TUTORIAL_MARK")
            .map(|(_, b)| crate::testing::decode::<TutorialShowQuestionMark>(b).mark_id)
            .collect()
    }

    /// The tutorial voices sent to player 1.
    fn voices(out: &crate::testing::Sent) -> Vec<String> {
        out.iter()
            .filter(|(to, b)| *to == 1 && crate::testing::name_of(b) == "PLAY_SOUND")
            .map(|(_, b)| crate::testing::decode::<PlaySound>(b))
            .filter(|s| s.unknown1 == 2)
            .map(|s| s.sound_file)
            .collect()
    }

    #[test]
    fn a_human_fighter_from_login_to_the_first_teacher() {
        let (mut t, roien, carl) = setup(ScriptSettings::default());
        t.enter_as(1, 1, race::HUMAN, pc::FIGHTER, vec![]);

        // Login: the state is started, memo bit 1 and every step bit but the gemstone and sit
        // ones, memoStateEx1 -2, and the client events of those bits.
        let first_memo = 1 | (MEMO_MASK & !(8388608 | 1048576));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", &first_memo.to_string()),
                rows::insert(NAME, "memoStateEx1", "-2"),
            ]
        );
        let out = t.take();
        let event = out
            .iter()
            .find(|(_, b)| crate::testing::name_of(b) == "TUTORIAL_ENABLE_CLIENT_EVENT")
            .map(|(_, b)| crate::testing::decode::<TutorialEnableClientEvent>(b).event_id);
        assert_eq!(event, Some(first_memo & !1));
        // Java's `QuestList` has it too (ids 1..19999); the client has no journal entry for it.
        assert_eq!(t.world.quest_list(Oid(1)).quests[0].quest_id, 255);

        // 5 s later: the first tutorial page, the voice and the guide.
        t.wait_ms(5_000);
        let out = t.take();
        assert_eq!(voices(&out), ["tutorial_voice_001a"]);
        assert!(tutorial_pages(&out)[0].contains("tutorial-human-fighter-001.html"));
        assert_eq!(t.count(1, TUTORIAL_GUIDE), 1);
        assert_eq!(memo(&mut t, Some(1)), -3);

        // The page's "close" link (`tutorial_close_1`): question mark 1.
        quest_input(
            &mut t,
            QuestInput::TutorialLinkHtml {
                bypass: "tutorial_close_1".to_owned(),
            },
        );
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"TUTORIAL_CLOSE_HTML".to_owned()));
        assert_eq!(marks(&out), [1]);
        assert_eq!(memo(&mut t, Some(1)), -4);

        // The question mark: the way to the helper, with a radar.
        quest_input(&mut t, QuestInput::TutorialQuestionMark { number: 1 });
        let out = t.take();
        assert!(tutorial_pages(&out)[0].contains("tutorial-human-fighter-007.html"));
        assert!(names_to(&out, 1).contains(&"RADAR_CONTROL".to_owned()));
        assert_eq!(memo(&mut t, Some(1)), -5);
        assert_eq!(memo(&mut t, None), (first_memo & MEMO_MASK) | 2);

        // The helper: his first page (on the first talk, no quest link needed).
        t.talk(1, carl);
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30009-001.html"));
        assert_eq!(memo(&mut t, Some(1)), 0);

        // A gremlin: voice 011 and question mark 3, then the gemstone on the ground.
        let gremlin = t.spawn(TUTORIAL_GREMLIN, 60);
        t.take();
        t.kill(1, gremlin);
        let out = t.take();
        assert_eq!(marks(&out), [3]);
        assert_eq!(memo(&mut t, Some(1)), 2);
        assert!(t.world.ground_item_ids().iter().any(|id| {
            t.world
                .ground_item(*id)
                .is_some_and(|g| g.data.item_id == BLUE_GEMSTONE)
        }));

        // Back to the helper without it, then with it: the recommendation and 200 soulshots.
        t.talk(1, carl);
        assert!(pages_to(&t.take(), 1).last().unwrap().contains("30009-002.html"));
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), BLUE_GEMSTONE, 1));
        t.take_writes();
        t.talk(1, carl);
        let out = t.take();
        assert!(
            pages_to(&out, 1).last().unwrap().contains("30009-003.html"),
            "{:?}",
            pages_to(&out, 1)
        );
        assert_eq!((t.count(1, BLUE_GEMSTONE), t.count(1, RECOMMENDATION_1)), (0, 1));
        assert_eq!(t.count(1, SOULSHOT_NO_GRADE_FOR_BEGINNERS), 200);
        assert_eq!(memo(&mut t, Some(1)), 3);
        t.talk(1, carl);
        assert!(pages_to(&t.take(), 1).last().unwrap().contains("30009-004.html"));

        // Roien takes the recommendation (link 31 of his page): 200 more soulshots and 50 sp.
        t.talk(1, roien);
        assert!(pages_to(&t.take(), 1).last().unwrap().contains("30008-001.html"));
        let sp = t.world.player(Oid(1)).unwrap().data.sp;
        t.bypass(1, &format!("Script {NAME} 31"));
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30008-002.html"));
        assert_eq!(voices(&out), ["tutorial_voice_026"]);
        assert_eq!(t.count(1, SOULSHOT_NO_GRADE_FOR_BEGINNERS), 400);
        assert_eq!(t.count(1, RECOMMENDATION_1), 0);
        assert_eq!(t.world.player(Oid(1)).unwrap().data.sp, sp + 50);
        assert_eq!(memo(&mut t, Some(1)), 4);

        // A minute later his timer: question mark 7 (the first class change).
        t.take();
        t.wait_ms(60_000);
        let out = t.take();
        assert!(marks(&out).contains(&7));
        assert!(voices(&out).contains(&"tutorial_voice_025".to_owned()));
    }

    #[test]
    fn the_listeners_are_kept_like_java_adds_them() {
        let (mut t, _roien, carl) = setup(ScriptSettings::default());
        t.enter_as(1, 1, race::HUMAN, pc::FIGHTER, vec![]);
        // Login added a pickup listener (adena bit 2097152); the helper's first talk adds a
        // second (gemstone bit): one adena pickup runs the adena step twice, the second time
        // with its bit already cleared.
        t.talk(1, carl);
        t.take();
        t.world.run_as_script(NAME, |ctx| {
            Q00255Tutorial.on_player_item_pickup(ctx, Oid(1), 57);
        });
        let out = t.take();
        assert_eq!(marks(&out), [23]);
        assert_eq!(memo(&mut t, None) & 2097152, 0);

        // Level 5: the 1024 step (fighter: question mark 9).
        t.world.run_as_script(NAME, |ctx| {
            Q00255Tutorial.on_player_level_changed(ctx, Oid(1), 5);
        });
        assert_eq!(marks(&t.take()), [9]);

        // At 30% HP or less the attacked listener (bit 256 at login) shows question mark 10 and
        // turns the sit listener on (the test world's max HP is 0, so 0 HP is "30%").
        t.world.player_mut(Oid(1)).unwrap().core.cur_hp = 0.0;
        t.world.run_as_script(NAME, |ctx| {
            Q00255Tutorial.on_creature_attacked(ctx, Oid(1));
            Q00255Tutorial.on_player_sit(ctx, Oid(1));
        });
        let out = t.take();
        // NOTE(java-quirk) of the port: three attacked listeners (login, the helper's talk, the
        // level step all had bit 256) and the step checks the level, not the bit: three marks.
        assert_eq!(marks(&out), [10, 10, 10]);
        // Then the sit step.
        assert!(voices(&out).contains(&"tutorial_voice_018".to_owned()));
    }

    #[test]
    fn a_character_of_level_six_only_gets_the_level_marks() {
        let (mut t, _, _) = setup(ScriptSettings::default());
        t.enter_as(1, 6, race::HUMAN, pc::WARRIOR, vec![item(0x6000_0100, 57, 1)]);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "<state>", "Started")]);
        assert!(marks(&t.take()).is_empty());
    }

    #[test]
    fn disabled_the_tutorial_does_nothing() {
        let settings = ScriptSettings {
            disable_tutorial: true,
            ..ScriptSettings::default()
        };
        let (mut t, _roien, carl) = setup(settings);
        t.enter_as(1, 1, race::HUMAN, pc::FIGHTER, vec![]);
        assert!(t.take_writes().is_empty());
        t.take();
        // The helper shows his own page.
        t.talk(1, carl);
        assert!(pages_to(&t.take(), 1).last().unwrap().contains("npc 30009"));
    }
}
