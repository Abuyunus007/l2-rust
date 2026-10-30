//! Quest 419: Get a Pet (Pet Manager Martin, the racial animal lists, the wolf collar quiz).
// Java: dist/game/data/scripts/quests/Q00419_GetAPet/Q00419_GetAPet.java
//
// Pattern: a kill quest where every race hunts its own spider/animal drop (table in `hunt`),
// followed by a quiz whose progress is a bit field in the memo state.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GUARD_METTY: i32 = 30072;
const ACCESSORY_MERCHANT_ELICE: i32 = 30091;
const GATEKEEPER_BELLA: i32 = 30256;
const PET_MENAGER_MARTIN: i32 = 30731;

// Items
const ANIMAL_LOVERS_LIST: i32 = 3417;
const ANIMAL_SLAYERS_1ST_LIST: i32 = 3418;
const ANIMAL_SLAYERS_2ND_LIST: i32 = 3419;
const ANIMAL_SLAYERS_3RD_LIST: i32 = 3420;
const ANIMAL_SLAYERS_4TH_LIST: i32 = 3421;
const ANIMAL_SLAYERS_5TH_LIST: i32 = 3422;
const BLOODY_FANG: i32 = 3423;
const BLOODY_CLAW: i32 = 3424;
const BLOODY_NAIL: i32 = 3425;
const BLOODY_KASHA_FANG: i32 = 3426;
const BLOODY_TARANTULA_NAIL: i32 = 3427;
const ANIMAL_SLAYERS_LIST: i32 = 10164;
const BLOODY_RED_CLAW: i32 = 10165;

// Reward
const WOLF_COLLAR: i32 = 2375;

// Monster
const LESSER_DARK_HORROR: i32 = 20025;
const PROWLER: i32 = 20034;
const GIANT_SPIDER: i32 = 20103;
const DARK_HORROR: i32 = 20105;
const TALON_SPIDER: i32 = 20106;
const BLADE_SPIDER: i32 = 20108;
const HOOK_SPIDER: i32 = 20308;
const HUNTER_TARANTULA: i32 = 20403;
const CRIMSON_SPIDER: i32 = 20460;
const PINCER_SPIDER: i32 = 20466;
const KASHA_SPIDER: i32 = 20474;
const KASHA_FANG_SPIDER: i32 = 20476;
const KASHA_BLADE_SPIDER: i32 = 20478;
const PLUNDER_TARANTULA: i32 = 20508;
const CRIMSON_SPIDER2: i32 = 22244;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `LINKS` (keys 1110001..1110070, in that order).
const LINKS: [&str; 70] = [
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Can be used for item transportation.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Can help during hunting by assisting in attacks.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">Can be sent to the village to buy items.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Can be traded or sold to a new owner for adena.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">When taking down a monster, always have a pet's company.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Tell your pet to pick up items.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Tell your pet to attack monsters first.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Let your pet do what it wants.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">10 hours</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">15 hours</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">It won't disappear.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">25 hours</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">Dire Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Air Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Turek Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Kasha Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">It's tail is always pointing straight down.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">It's tail is always curled up.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">It's tail is always wagging back and forth.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">What are you talking about?! A wolf doesn't have a tail.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Raccoon</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Jackal</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Fox</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Shepherd Dog</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">1.4 km</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">2.4 km</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">3.4 km</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">4.4 km</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">Male</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Female</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">A baby that was born last year</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">A baby that was born two years ago</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Goat</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Meat of a dead animal</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Berries</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Wild Bird</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Breeding season is January-February.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">Pregnancy is nine months.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Babies are born in April-June.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Has up to ten offspring at one time.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">3-6 years</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">6-9 years</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">9-12 years</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">12-15 years</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Wolves gather and move in groups of 7-13 animals.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Wolves can eat a whole calf in one sitting.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">If they have water, wolves can live for 5-6 days without eating anything.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">A pregnant wolf makes its home in a wide open place to have its babies.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">A grown wolf is still not as heavy as a fully-grown male adult human.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">A wolf changes into a werewolf during a full-moon.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">The color of a wolf's fur is the same as the place where it lives.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">A wolf enjoys eating Dwarves.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Talking Island - Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Dark Forest - Ashen Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">Elven Forest - Gray Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet QUESTIONS\">Orc - Black Wolf</a><br>",
    "<a action=\"bypass -h Script Q00419_GetAPet 30731-14.html\">None of the above.</a><br>",
];

/// The list, the drop and the chance (percent) of a monster (Java's `onKill` switch).
fn hunt(npc_id: i32) -> Option<(i32, i32, i32)> {
    Some(match npc_id {
        LESSER_DARK_HORROR => (ANIMAL_SLAYERS_3RD_LIST, BLOODY_NAIL, 60),
        PROWLER => (ANIMAL_SLAYERS_3RD_LIST, BLOODY_NAIL, 100),
        GIANT_SPIDER => (ANIMAL_SLAYERS_1ST_LIST, BLOODY_FANG, 60),
        DARK_HORROR => (ANIMAL_SLAYERS_3RD_LIST, BLOODY_NAIL, 75),
        TALON_SPIDER => (ANIMAL_SLAYERS_1ST_LIST, BLOODY_FANG, 75),
        BLADE_SPIDER => (ANIMAL_SLAYERS_1ST_LIST, BLOODY_FANG, 100),
        HOOK_SPIDER => (ANIMAL_SLAYERS_2ND_LIST, BLOODY_CLAW, 75),
        HUNTER_TARANTULA => (ANIMAL_SLAYERS_5TH_LIST, BLOODY_TARANTULA_NAIL, 75),
        CRIMSON_SPIDER => (ANIMAL_SLAYERS_2ND_LIST, BLOODY_CLAW, 60),
        PINCER_SPIDER => (ANIMAL_SLAYERS_2ND_LIST, BLOODY_CLAW, 100),
        KASHA_SPIDER => (ANIMAL_SLAYERS_4TH_LIST, BLOODY_KASHA_FANG, 60),
        KASHA_FANG_SPIDER => (ANIMAL_SLAYERS_4TH_LIST, BLOODY_KASHA_FANG, 75),
        KASHA_BLADE_SPIDER => (ANIMAL_SLAYERS_4TH_LIST, BLOODY_KASHA_FANG, 100),
        PLUNDER_TARANTULA => (ANIMAL_SLAYERS_5TH_LIST, BLOODY_TARANTULA_NAIL, 100),
        CRIMSON_SPIDER2 => (ANIMAL_SLAYERS_LIST, BLOODY_RED_CLAW, 75),
        _ => return None,
    })
}

/// The list and the drop of a race, in the order Java's `onTalk` checks them.
const LISTS: [(i32, i32); 6] = [
    (ANIMAL_SLAYERS_LIST, BLOODY_RED_CLAW),
    (ANIMAL_SLAYERS_1ST_LIST, BLOODY_FANG),
    (ANIMAL_SLAYERS_2ND_LIST, BLOODY_CLAW),
    (ANIMAL_SLAYERS_3RD_LIST, BLOODY_NAIL),
    (ANIMAL_SLAYERS_4TH_LIST, BLOODY_KASHA_FANG),
    (ANIMAL_SLAYERS_5TH_LIST, BLOODY_TARANTULA_NAIL),
];

/// Java `Q00419_GetAPet`.
pub struct Q00419GetAPet;

impl Script for Q00419GetAPet {
    fn name(&self) -> &'static str {
        "Q00419_GetAPet"
    }

    fn path(&self) -> &'static str {
        "quests/Q00419_GetAPet"
    }

    fn id(&self) -> i32 {
        419
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PET_MENAGER_MARTIN]);
        s.add_talk_id(&[
            PET_MENAGER_MARTIN,
            GUARD_METTY,
            ACCESSORY_MERCHANT_ELICE,
            GATEKEEPER_BELLA,
        ]);
        s.add_kill_id(&[
            LESSER_DARK_HORROR,
            PROWLER,
            GIANT_SPIDER,
            DARK_HORROR,
            TALON_SPIDER,
            BLADE_SPIDER,
            HOOK_SPIDER,
            HUNTER_TARANTULA,
            CRIMSON_SPIDER,
            PINCER_SPIDER,
            KASHA_SPIDER,
            KASHA_FANG_SPIDER,
            KASHA_BLADE_SPIDER,
            PLUNDER_TARANTULA,
            CRIMSON_SPIDER2,
        ]);
        s.register_quest_items(&[
            ANIMAL_LOVERS_LIST,
            ANIMAL_SLAYERS_1ST_LIST,
            ANIMAL_SLAYERS_2ND_LIST,
            ANIMAL_SLAYERS_3RD_LIST,
            ANIMAL_SLAYERS_4TH_LIST,
            ANIMAL_SLAYERS_5TH_LIST,
            BLOODY_FANG,
            BLOODY_CLAW,
            BLOODY_NAIL,
            BLOODY_KASHA_FANG,
            BLOODY_TARANTULA_NAIL,
            ANIMAL_SLAYERS_LIST,
            BLOODY_RED_CLAW,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    let r = ctx.player_race(player);
                    if r == race::HUMAN {
                        ctx.give_items(player, ANIMAL_SLAYERS_1ST_LIST, 1);
                        htmltext = Some("30731-04.htm".to_owned());
                    } else if r == race::ELF {
                        ctx.give_items(player, ANIMAL_SLAYERS_2ND_LIST, 1);
                        htmltext = Some("30731-05.htm".to_owned());
                    } else if r == race::DARK_ELF {
                        ctx.give_items(player, ANIMAL_SLAYERS_3RD_LIST, 1);
                        htmltext = Some("30731-06.htm".to_owned());
                    } else if r == race::ORC {
                        ctx.give_items(player, ANIMAL_SLAYERS_4TH_LIST, 1);
                        htmltext = Some("30731-07.htm".to_owned());
                    } else if r == race::DWARF {
                        ctx.give_items(player, ANIMAL_SLAYERS_5TH_LIST, 1);
                        htmltext = Some("30731-08.htm".to_owned());
                    } else if r == race::KAMAEL {
                        ctx.give_items(player, ANIMAL_SLAYERS_LIST, 1);
                        htmltext = Some("30731-08a.htm".to_owned());
                    }
                }
            }
            "30731-03.htm" | "30072-02.html" | "30091-02.html" | "30256-02.html" | "30256-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30731-12.html" => {
                // Each race hands in its own list and 50 drops for the lover's list.
                let r = ctx.player_race(player);
                let (list, drop) = if r == race::HUMAN {
                    (ANIMAL_SLAYERS_1ST_LIST, BLOODY_FANG)
                } else if r == race::ELF {
                    (ANIMAL_SLAYERS_2ND_LIST, BLOODY_CLAW)
                } else if r == race::DARK_ELF {
                    (ANIMAL_SLAYERS_3RD_LIST, BLOODY_NAIL)
                } else if r == race::ORC {
                    (ANIMAL_SLAYERS_4TH_LIST, BLOODY_KASHA_FANG)
                } else if r == race::DWARF {
                    (ANIMAL_SLAYERS_5TH_LIST, BLOODY_TARANTULA_NAIL)
                } else {
                    (ANIMAL_SLAYERS_LIST, BLOODY_RED_CLAW)
                };
                if ctx.has_quest_items(player, list) && ctx.get_quest_items_count(player, drop) >= 50 {
                    ctx.take_items(player, list, -1);
                    ctx.take_items(player, drop, -1);
                    ctx.give_items(player, ANIMAL_LOVERS_LIST, 1);
                }

                qs.set_memo_state(ctx, 0);
                htmltext = Some(event.to_owned());
            }
            "QUESTIONS" => {
                if (qs.get_memo_state(ctx) & 15) == 10 && ctx.has_quest_items(player, ANIMAL_LOVERS_LIST) {
                    ctx.take_items(player, ANIMAL_LOVERS_LIST, -1);
                    ctx.give_items(player, WOLF_COLLAR, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("30731-15.html".to_owned());
                } else {
                    let mut link_base = 0;
                    let mut page = String::new();
                    let mut find_response = false;
                    while !find_response {
                        let random_link_offset = ctx.get_random(14) + 4;
                        let mut i7: i32 = 1;
                        for _ in 1..=random_link_offset {
                            i7 *= 2;
                        }

                        let memo = qs.get_memo_state(ctx);
                        if (i7 & memo) == 0 && random_link_offset < 18 {
                            find_response = true;
                            qs.set_memo_state(ctx, (memo + 1) | i7);
                            // Index of Java's `LINKS` key `1110000 + 5 * (offset - 4)` + 1.
                            link_base = (5 * (random_link_offset - 4)) as usize;
                            page = format!("30731-{}.htm", 20 + (random_link_offset - 4));
                        }
                    }

                    let mut link_count = 1;
                    let mut reply_offset = [0usize; 4];
                    let mut i8: i32 = 0;
                    while link_count < 5 {
                        let random_reply_offset = ctx.get_random(4) + 1;
                        let mut i7: i32 = 1;
                        for _ in 1..=random_reply_offset {
                            i7 *= 2;
                        }

                        if (i7 & i8) == 0 && random_reply_offset < 5 {
                            reply_offset[link_count - 1] = random_reply_offset as usize;
                            link_count += 1;
                            i8 |= i7;
                        }
                    }

                    // Java's `LINKS.get(linkId + n)` is `LINKS[link_base + n - 1]`.
                    let link = |n: usize| LINKS.get(link_base + n - 1).copied().unwrap_or("");
                    let text = ctx.get_htm(player, &page)?;
                    htmltext = Some(
                        text.replace("<?reply1?>", link(reply_offset[0]))
                            .replace("<?reply2?>", link(reply_offset[1]))
                            .replace("<?reply3?>", link(reply_offset[2]))
                            .replace("<?reply4?>", link(reply_offset[3]))
                            .replace("<?reply5?>", link(5)),
                    );
                }
            }
            "30731-14.html" => {
                qs.set_memo_state(ctx, 0);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && let Some((list, drop, chance)) = hunt(npc.id)
            && ctx.has_quest_items(killer, list)
            && ctx.get_quest_items_count(killer, drop) < 50
            && ctx.get_random(100) < chance
        {
            ctx.give_items(killer, drop, 1);
            if ctx.get_quest_items_count(killer, drop) >= 50 {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            if npc.id == PET_MENAGER_MARTIN {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30731-01.htm"
                } else {
                    "30731-02.htm"
                }
                .to_owned();
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                PET_MENAGER_MARTIN => {
                    if let Some((_, drop)) = LISTS.iter().find(|(list, _)| ctx.has_quest_items(player, *list))
                    {
                        htmltext = if ctx.get_quest_items_count(player, *drop) < 50 {
                            if !ctx.has_quest_items(player, *drop) {
                                "30731-09.html"
                            } else {
                                "30731-10.html"
                            }
                        } else {
                            "30731-11.html"
                        }
                        .to_owned();
                    } else if ctx.has_quest_items(player, ANIMAL_LOVERS_LIST) {
                        let memo = qs.get_memo_state(ctx);
                        if memo != 14 && memo != 1879048192 {
                            htmltext = "30731-16.html".to_owned();
                        } else {
                            qs.set_memo_state(ctx, 1879048192);
                            htmltext = "30731-13.html".to_owned();
                        }
                    }
                }
                GUARD_METTY => {
                    if ctx.has_quest_items(player, ANIMAL_LOVERS_LIST) {
                        let memo = qs.get_memo_state(ctx);
                        qs.set_memo_state(ctx, memo | 4);
                        htmltext = "30072-01.html".to_owned();
                    }
                }
                ACCESSORY_MERCHANT_ELICE => {
                    if ctx.has_quest_items(player, ANIMAL_LOVERS_LIST) {
                        let memo = qs.get_memo_state(ctx);
                        qs.set_memo_state(ctx, memo | 8);
                        htmltext = "30091-01.html".to_owned();
                    }
                }
                GATEKEEPER_BELLA => {
                    if ctx.has_quest_items(player, ANIMAL_LOVERS_LIST) {
                        let memo = qs.get_memo_state(ctx);
                        qs.set_memo_state(ctx, memo | 2);
                        htmltext = "30256-01.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00419_GetAPet";

    const NPCS: &str = r#"
        <npc id="30731" type="Folk" level="30" name="Pet Manager Martin"/>
        <npc id="30072" type="Folk" level="30" name="Guard Metty"/>
        <npc id="30091" type="Folk" level="30" name="Elice"/>
        <npc id="30256" type="Folk" level="30" name="Bella"/>
        <npc id="20108" type="Monster" level="10" name="Blade Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="2375" type="EtcItem" name="Wolf Collar"/>
        <item id="3417" type="EtcItem" name="Animal Lover's List"><set name="is_questitem" val="true"/></item>
        <item id="3418" type="EtcItem" name="Animal Slayer's 1st List"><set name="is_questitem" val="true"/></item>
        <item id="3423" type="EtcItem" name="Bloody Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00419GetAPet);
        let mut pages = Pages::of(&Q00419GetAPet)
            .page("30731-02.htm", &["30731-03.htm"])
            .page("30731-03.htm", &["ACCEPT"])
            .page("30731-11.html", &["30731-12.html"])
            .page("30731-13.html", &["QUESTIONS"])
            .plain(&[
                "30731-01.htm",
                "30731-04.htm",
                "30731-09.html",
                "30731-10.html",
                "30731-12.html",
                "30731-15.html",
                "30731-16.html",
                "30072-01.html",
                "30091-01.html",
                "30256-01.html",
            ]);
        for n in 20..=33 {
            pages = pages.file(
                &format!("data/scripts/quests/{NAME}/30731-{n}.htm"),
                "<html><body>question<?reply1?><?reply2?><?reply3?><?reply4?><?reply5?></body></html>",
            );
        }
        let pages = pages.npcs(&[30731, 30072, 30091, 30256]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let martin = t.spawn(30731, 20);
        let metty = t.spawn(30072, 25);
        let elice = t.spawn(30091, 30);
        let bella = t.spawn(30256, 35);
        (t, martin, metty, elice, bella)
    }

    fn start(t: &mut ScriptTest, martin: Oid) {
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, martin).contains("30731-02.htm"));
        assert!(t.event(1, NAME, "30731-03.htm").contains("30731-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30731-04.htm"));
    }

    #[test]
    fn a_level_14_character_is_sent_away() {
        let (mut t, martin, ..) = setup();
        t.enter_as(1, 14, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, martin).contains("30731-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_human_gets_the_first_list_and_hunts_fangs() {
        let (mut t, martin, ..) = setup();
        start(&mut t, martin);
        assert_eq!(t.count(1, ANIMAL_SLAYERS_1ST_LIST), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, martin).contains("30731-09.html"));
        // The Blade Spider always drops (100%).
        let spider = t.spawn(20108, 60);
        t.kill(1, spider);
        assert_eq!(t.count(1, BLOODY_FANG), 1);
        assert!(t.quest_page(1, martin).contains("30731-10.html"));
    }

    #[test]
    fn fifty_fangs_then_the_visits_then_the_quiz_ends_with_the_collar() {
        let (mut t, martin, metty, elice, bella) = setup();
        start(&mut t, martin);
        for _ in 0..50 {
            let spider = t.spawn(20108, 60);
            t.kill(1, spider);
        }
        assert_eq!(t.count(1, BLOODY_FANG), 50);
        assert!(t.quest_page(1, martin).contains("30731-11.html"));
        t.take_writes();
        assert!(t.event(1, NAME, "30731-12.html").contains("30731-12.html"));
        assert_eq!(
            (
                t.count(1, ANIMAL_LOVERS_LIST),
                t.count(1, ANIMAL_SLAYERS_1ST_LIST),
                t.count(1, BLOODY_FANG)
            ),
            (1, 0, 0)
        );

        // The three visits set the bits 4, 8 and 2 of the memo state: 14.
        t.quest_page(1, metty);
        t.quest_page(1, elice);
        t.quest_page(1, bella);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            assert_eq!(qs.get_memo_state(ctx), 14);
        });
        assert!(t.quest_page(1, martin).contains("30731-13.html"));

        // A question: one of the quiz pages with the five answers filled in.
        let page = t.event(1, NAME, "QUESTIONS");
        assert!(page.contains("question"));
        assert!(page.contains("Script Q00419_GetAPet"));
        assert!(!page.contains("<?reply"));
        assert_eq!(t.count(1, 2375), 0);

        // All the answers right (memo & 15 == 10): the collar.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 10);
        });
        t.take_writes();
        assert!(t.event(1, NAME, "QUESTIONS").contains("30731-15.html"));
        assert_eq!((t.count(1, 2375), t.count(1, ANIMAL_LOVERS_LIST)), (1, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
