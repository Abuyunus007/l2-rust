//! Quest 416: Path of the Orc Shaman (Tataru Zu Hestui, Umos, Moira and the totem spirits).
// Java: dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/Q00416_PathOfTheOrcShaman.java
//
// Pattern: the longest kill chain of the path quests. The memo state is the step (1 for the
// first phase, 100 to 110 for the leopard phase), the kill hooks look at the cond of a random
// party member (`getRandomPartyMemberState`), and the monsters drop the item the cond asks for.
// Two rewards: Umos' (the totem spirit's blood) and Moira's (after the leopards).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const UMOS: i32 = 30502;
const TATARU_ZU_HESTUI: i32 = 30585;
const HESTUI_TOTEM_SPIRIT: i32 = 30592;
const DUDA_MARA_TOTEM_SPIRIT: i32 = 30593;
const MOIRA: i32 = 31979;
const TOTEM_SPIRIT_OF_GANDI: i32 = 32057;
const DEAD_LEOPARDS_CARCASS: i32 = 32090;

// Items
const FIRE_CHARM: i32 = 1616;
const KASHA_BEAR_PELT: i32 = 1617;
const KASHA_BLADE_SPIDER_HUSK: i32 = 1618;
const FIRST_FIERY_EGG: i32 = 1619;
const HESTUI_MASK: i32 = 1620;
const SECOND_FIERY_EGG: i32 = 1621;
const TOTEM_SPIRIT_CLAW: i32 = 1622;
const TATARUS_LETTER: i32 = 1623;
const FLAME_CHARM: i32 = 1624;
const GRIZZLY_BLOOD: i32 = 1625;
const BLOOD_CAULDRON: i32 = 1626;
const SPIRIT_NET: i32 = 1627;
const BOUND_DURKA_SPIRIT: i32 = 1628;
const DURKA_PARASITE: i32 = 1629;
const TOTEM_SPIRIT_BLOOD: i32 = 1630;
const MASK_OF_MEDIUM: i32 = 1631;

// Quest monsters
const DURKA_SPIRIT: i32 = 27056;
const BLACK_LEOPARD: i32 = 27319;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `MOBS`: (monster, the item it drops, the cond that makes it count, the item's chance).
/// The chance is `1` (100%) for every monster.
const MOBS: [(i32, i32, i32); 7] = [
    (20415, FIRST_FIERY_EGG, 1),
    (20478, KASHA_BLADE_SPIDER_HUSK, 1),
    (20479, KASHA_BEAR_PELT, 1),
    (20335, GRIZZLY_BLOOD, 6),
    (20038, DURKA_PARASITE, 9),
    (20043, DURKA_PARASITE, 9),
    (DURKA_SPIRIT, DURKA_PARASITE, 9),
];

/// Java `Q00416_PathOfTheOrcShaman`.
pub struct Q00416PathOfTheOrcShaman;

impl Script for Q00416PathOfTheOrcShaman {
    fn name(&self) -> &'static str {
        "Q00416_PathOfTheOrcShaman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00416_PathOfTheOrcShaman"
    }

    fn id(&self) -> i32 {
        416
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TATARU_ZU_HESTUI]);
        s.add_talk_id(&[
            TATARU_ZU_HESTUI,
            UMOS,
            MOIRA,
            DEAD_LEOPARDS_CARCASS,
            DUDA_MARA_TOTEM_SPIRIT,
            HESTUI_TOTEM_SPIRIT,
            TOTEM_SPIRIT_OF_GANDI,
        ]);
        let mobs: Vec<i32> = MOBS.iter().map(|m| m.0).collect();
        s.add_kill_id(&mobs);
        s.add_kill_id(&[BLACK_LEOPARD]);
        s.register_quest_items(&[
            FIRE_CHARM,
            KASHA_BEAR_PELT,
            KASHA_BLADE_SPIDER_HUSK,
            FIRST_FIERY_EGG,
            HESTUI_MASK,
            SECOND_FIERY_EGG,
            TOTEM_SPIRIT_CLAW,
            TATARUS_LETTER,
            FLAME_CHARM,
            GRIZZLY_BLOOD,
            BLOOD_CAULDRON,
            SPIRIT_NET,
            BOUND_DURKA_SPIRIT,
            DURKA_PARASITE,
            TOTEM_SPIRIT_BLOOD,
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
            "START" => {
                htmltext = Some(
                    if ctx.player_class_id(player) != player_class::ORC_MAGE {
                        if ctx.player_class_id(player) == player_class::ORC_SHAMAN {
                            "30585-02.htm"
                        } else {
                            "30585-03.htm"
                        }
                    } else if ctx.player_level(player) < MIN_LEVEL {
                        "30585-04.htm"
                    } else if ctx.has_quest_items(player, MASK_OF_MEDIUM) {
                        "30585-05.htm"
                    } else {
                        "30585-06.htm"
                    }
                    .to_owned(),
                );
            }
            "30585-07.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                ctx.give_items(player, FIRE_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30585-12.html" => {
                if ctx.has_quest_items(player, TOTEM_SPIRIT_CLAW) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30585-13.html" => {
                if ctx.has_quest_items(player, TOTEM_SPIRIT_CLAW) {
                    ctx.take_items(player, TOTEM_SPIRIT_CLAW, -1);
                    ctx.give_items(player, TATARUS_LETTER, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30585-14.html" => {
                if ctx.has_quest_items(player, TOTEM_SPIRIT_CLAW) {
                    ctx.take_items(player, TOTEM_SPIRIT_CLAW, -1);
                    qs.set_cond(ctx, 12, true);
                    qs.set_memo_state(ctx, 100);
                    htmltext = Some(event.to_owned());
                }
            }
            "30502-07.html" => {
                if ctx.has_quest_items(player, TOTEM_SPIRIT_BLOOD) {
                    ctx.take_items(player, TOTEM_SPIRIT_BLOOD, -1);
                    ctx.give_items(player, MASK_OF_MEDIUM, 1);
                    let level = ctx.player_level(player);
                    if level >= 20 {
                        ctx.add_exp_and_sp(player, 320_534, 22_992);
                    } else if level >= 19 {
                        ctx.add_exp_and_sp(player, 456_128, 29_690);
                    } else {
                        ctx.add_exp_and_sp(player, 591_724, 36_388);
                    }
                    ctx.give_adena(player, 163_800, true);
                    qs.exit_quest(ctx, false, true);
                    ctx.world_mut().send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "32090-05.html" => {
                if qs.is_memo_state(ctx, 106) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32090-06.html" => {
                if qs.is_memo_state(ctx, 106) {
                    qs.set_memo_state(ctx, 107);
                    qs.set_cond(ctx, 18, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30593-02.html" => {
                if ctx.has_quest_items(player, BLOOD_CAULDRON) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30593-03.html" => {
                if ctx.has_quest_items(player, BLOOD_CAULDRON) {
                    ctx.take_items(player, BLOOD_CAULDRON, -1);
                    ctx.give_items(player, SPIRIT_NET, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30592-02.html" => {
                if ctx.has_all_quest_items(player, &[HESTUI_MASK, SECOND_FIERY_EGG]) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30592-03.html" => {
                if ctx.has_all_quest_items(player, &[HESTUI_MASK, SECOND_FIERY_EGG]) {
                    ctx.take_items_many(player, -1, &[HESTUI_MASK, SECOND_FIERY_EGG]);
                    ctx.give_items(player, TOTEM_SPIRIT_CLAW, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32057-02.html" => {
                if qs.is_memo_state(ctx, 101) {
                    qs.set_memo_state(ctx, 102);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32057-05.html" => {
                if qs.is_memo_state(ctx, 109) {
                    qs.set_memo_state(ctx, 110);
                    qs.set_cond(ctx, 21, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java: `getRandomPartyMemberState(player, -1, 3, npc)`: the member whose cond counts.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let member = qs.player();

        if npc.id == BLACK_LEOPARD {
            match qs.get_memo_state(ctx) {
                102 => {
                    qs.set_memo_state(ctx, 103);
                }
                103 => {
                    qs.set_memo_state(ctx, 104);
                    qs.set_cond(ctx, 15, true);
                    if ctx.get_random(100) < 66 {
                        let name = ctx.player_name(member);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::MyDearFriendOfS1WhoHasGoneOnAheadOfMe,
                            &[&name],
                        );
                    }
                }
                105 => {
                    qs.set_memo_state(ctx, 106);
                    qs.set_cond(ctx, 17, true);
                    if ctx.get_random(100) < 66 {
                        let name = ctx.player_name(member);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::ListenToTejakarGandiYoungOrokaTheSpiritOfTheSlainLeopardIsCallingYouS1,
                            &[&name],
                        );
                    }
                }
                107 => {
                    qs.set_memo_state(ctx, 108);
                    qs.set_cond(ctx, 19, true);
                }
                _ => {}
            }
            return;
        }

        let Some(&(_, item, required_cond)) = MOBS.iter().find(|m| m.0 == npc.id) else {
            return;
        };
        if required_cond != qs.get_cond(ctx) {
            return;
        }
        if qs.is_cond(ctx, 1) && ctx.has_quest_items(member, FIRE_CHARM) {
            if ctx.give_item_randomly(member, Some(npc), item, 1, 1, 1, 1.0, true)
                && ctx.has_all_quest_items(
                    member,
                    &[FIRST_FIERY_EGG, KASHA_BLADE_SPIDER_HUSK, KASHA_BEAR_PELT],
                )
            {
                qs.set_cond(ctx, 2, true);
            }
        } else if qs.is_cond(ctx, 6) && ctx.has_quest_items(member, FLAME_CHARM) {
            if ctx.give_item_randomly(member, Some(npc), item, 1, 1, 3, 1.0, true) {
                qs.set_cond(ctx, 7, false);
            }
        } else if qs.is_cond(ctx, 9)
            && ctx.has_quest_items(member, SPIRIT_NET)
            && !ctx.has_quest_items(member, BOUND_DURKA_SPIRIT)
            && ctx.get_quest_items_count(member, DURKA_PARASITE) <= 8
        {
            if npc.id == 20038 || npc.id == 20043 {
                let random = ctx.get_random(10);
                let item_count = ctx.get_quest_items_count(member, DURKA_PARASITE);
                if (item_count == 5 && random < 1)
                    || (item_count == 6 && random < 2)
                    || (item_count == 7 && random < 2)
                    || item_count >= 8
                {
                    ctx.take_items(killer, DURKA_PARASITE, -1);
                    if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                        ctx.add_spawn(DURKA_SPIRIT, x, y, z, 0, true, 0, 0);
                    }
                    ctx.play_sound(member, QuestSound::ItemsoundQuestBeforeBattle);
                } else {
                    ctx.give_items(member, DURKA_PARASITE, 1);
                    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                }
            } else {
                ctx.give_items(member, BOUND_DURKA_SPIRIT, 1);
                ctx.take_items_many(member, -1, &[DURKA_PARASITE, SPIRIT_NET]);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == TATARU_ZU_HESTUI {
                htmltext = "30585-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                TATARU_ZU_HESTUI => {
                    if qs.is_memo_state(ctx, 1) {
                        if ctx.has_quest_items(player, FIRE_CHARM) {
                            if ctx.get_quest_items_count_of(
                                player,
                                &[KASHA_BEAR_PELT, KASHA_BLADE_SPIDER_HUSK, FIRST_FIERY_EGG],
                            ) < 3
                            {
                                htmltext = "30585-08.html".to_owned();
                            } else {
                                ctx.take_items_many(
                                    player,
                                    -1,
                                    &[
                                        FIRE_CHARM,
                                        KASHA_BEAR_PELT,
                                        KASHA_BLADE_SPIDER_HUSK,
                                        FIRST_FIERY_EGG,
                                    ],
                                );
                                ctx.give_items(player, HESTUI_MASK, 1);
                                ctx.give_items(player, SECOND_FIERY_EGG, 1);
                                qs.set_cond(ctx, 3, true);
                                htmltext = "30585-09.html".to_owned();
                            }
                        } else if ctx.has_all_quest_items(player, &[HESTUI_MASK, SECOND_FIERY_EGG]) {
                            htmltext = "30585-10.html".to_owned();
                        } else if ctx.has_quest_items(player, TOTEM_SPIRIT_CLAW) {
                            htmltext = "30585-11.html".to_owned();
                        } else if ctx.has_quest_items(player, TATARUS_LETTER) {
                            htmltext = "30585-15.html".to_owned();
                        } else if ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                GRIZZLY_BLOOD,
                                FLAME_CHARM,
                                BLOOD_CAULDRON,
                                SPIRIT_NET,
                                BOUND_DURKA_SPIRIT,
                                TOTEM_SPIRIT_BLOOD,
                            ],
                        ) {
                            htmltext = "30585-16.html".to_owned();
                        }
                    } else if qs.is_memo_state(ctx, 100) {
                        htmltext = "30585-14.html".to_owned();
                    }
                }
                UMOS => {
                    if qs.is_memo_state(ctx, 1) {
                        if ctx.has_quest_items(player, TATARUS_LETTER) {
                            ctx.give_items(player, FLAME_CHARM, 1);
                            ctx.take_items(player, TATARUS_LETTER, -1);
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30502-01.html".to_owned();
                        } else if ctx.has_quest_items(player, FLAME_CHARM) {
                            if ctx.get_quest_items_count(player, GRIZZLY_BLOOD) < 3 {
                                htmltext = "30502-02.html".to_owned();
                            } else {
                                ctx.take_items_many(player, -1, &[FLAME_CHARM, GRIZZLY_BLOOD]);
                                ctx.give_items(player, BLOOD_CAULDRON, 1);
                                qs.set_cond(ctx, 8, true);
                                htmltext = "30502-03.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, BLOOD_CAULDRON) {
                            htmltext = "30502-04.html".to_owned();
                        } else if ctx.has_at_least_one_quest_item(player, &[BOUND_DURKA_SPIRIT, SPIRIT_NET]) {
                            htmltext = "30502-05.html".to_owned();
                        } else if ctx.has_quest_items(player, TOTEM_SPIRIT_BLOOD) {
                            htmltext = "30502-06.html".to_owned();
                        }
                    }
                }
                MOIRA => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 100 {
                        qs.set_memo_state(ctx, 101);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "31979-01.html".to_owned();
                    } else if (101..108).contains(&memo_state) {
                        htmltext = "31979-02.html".to_owned();
                    } else if memo_state == 110 {
                        ctx.give_items(player, MASK_OF_MEDIUM, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 160_267, 11_496);
                        } else if level >= 19 {
                            ctx.add_exp_and_sp(player, 228_064, 14_845);
                        } else {
                            ctx.add_exp_and_sp(player, 295_862, 18_194);
                        }
                        ctx.give_adena(player, 81_900, true);
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "31979-03.html".to_owned();
                    }
                }
                DEAD_LEOPARDS_CARCASS => match qs.get_memo_state(ctx) {
                    102 | 103 => htmltext = "32090-01.html".to_owned(),
                    104 => {
                        qs.set_memo_state(ctx, 105);
                        qs.set_cond(ctx, 16, true);
                        htmltext = "32090-03.html".to_owned();
                    }
                    105 => htmltext = "32090-01.html".to_owned(),
                    106 => htmltext = "32090-04.html".to_owned(),
                    107 => htmltext = "32090-07.html".to_owned(),
                    108 => {
                        qs.set_memo_state(ctx, 109);
                        qs.set_cond(ctx, 20, true);
                        htmltext = "32090-08.html".to_owned();
                    }
                    _ => {}
                },
                DUDA_MARA_TOTEM_SPIRIT => {
                    if qs.is_memo_state(ctx, 1) {
                        if ctx.has_quest_items(player, BLOOD_CAULDRON) {
                            htmltext = "30593-01.html".to_owned();
                        } else if ctx.has_quest_items(player, SPIRIT_NET)
                            && !ctx.has_quest_items(player, BOUND_DURKA_SPIRIT)
                        {
                            htmltext = "30593-04.html".to_owned();
                        } else if !ctx.has_quest_items(player, SPIRIT_NET)
                            && ctx.has_quest_items(player, BOUND_DURKA_SPIRIT)
                        {
                            ctx.take_items(player, BOUND_DURKA_SPIRIT, -1);
                            ctx.give_items(player, TOTEM_SPIRIT_BLOOD, 1);
                            qs.set_cond(ctx, 11, true);
                            htmltext = "30593-05.html".to_owned();
                        } else if ctx.has_quest_items(player, TOTEM_SPIRIT_BLOOD) {
                            htmltext = "30593-06.html".to_owned();
                        }
                    }
                }
                HESTUI_TOTEM_SPIRIT => {
                    if qs.is_memo_state(ctx, 1) {
                        if ctx.has_all_quest_items(player, &[HESTUI_MASK, SECOND_FIERY_EGG]) {
                            htmltext = "30592-01.html".to_owned();
                        } else if ctx.has_quest_items(player, TOTEM_SPIRIT_CLAW) {
                            htmltext = "30592-04.html".to_owned();
                        } else if ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                GRIZZLY_BLOOD,
                                FLAME_CHARM,
                                BLOOD_CAULDRON,
                                SPIRIT_NET,
                                BOUND_DURKA_SPIRIT,
                                TOTEM_SPIRIT_BLOOD,
                                TATARUS_LETTER,
                            ],
                        ) {
                            htmltext = "30592-05.html".to_owned();
                        }
                    }
                }
                TOTEM_SPIRIT_OF_GANDI => match qs.get_memo_state(ctx) {
                    101 => htmltext = "32057-01.html".to_owned(),
                    102 => htmltext = "32057-03.html".to_owned(),
                    109 => htmltext = "32057-04.html".to_owned(),
                    _ => {}
                },
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00416_PathOfTheOrcShaman";

    const NPCS: &str = r#"
        <npc id="30585" type="Folk" level="70" name="Tataru Zu Hestui"/>
        <npc id="30502" type="Folk" level="70" name="Umos"/>
        <npc id="30592" type="Folk" level="70" name="Hestui Totem Spirit"/>
        <npc id="30593" type="Folk" level="70" name="Duda Mara Totem Spirit"/>
        <npc id="31979" type="Folk" level="70" name="Moira"/>
        <npc id="32057" type="Folk" level="70" name="Totem Spirit of Gandi"/>
        <npc id="32090" type="Folk" level="70" name="Dead Leopard's Carcass"/>
        <npc id="20415" type="Monster" level="20" name="Scarlet Salamander"/>
        <npc id="20478" type="Monster" level="20" name="Kasha Blade Spider"/>
        <npc id="20479" type="Monster" level="20" name="Kasha Bear"/>
        <npc id="20335" type="Monster" level="20" name="Grizzly Bear"/>
        <npc id="20038" type="Monster" level="20" name="Poison Spider"/>
        <npc id="27056" type="Monster" level="30" name="Durka Spirit"/>
        <npc id="27319" type="Monster" level="30" name="Black Leopard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1616" type="EtcItem" name="Fire Charm"><set name="is_questitem" val="true"/></item>
        <item id="1617" type="EtcItem" name="Kasha Bear Pelt"><set name="is_questitem" val="true"/></item>
        <item id="1618" type="EtcItem" name="Kasha Blade Spider Husk"><set name="is_questitem" val="true"/></item>
        <item id="1619" type="EtcItem" name="First Fiery Egg"><set name="is_questitem" val="true"/></item>
        <item id="1620" type="EtcItem" name="Hestui Mask"><set name="is_questitem" val="true"/></item>
        <item id="1621" type="EtcItem" name="Second Fiery Egg"><set name="is_questitem" val="true"/></item>
        <item id="1622" type="EtcItem" name="Totem Spirit Claw"><set name="is_questitem" val="true"/></item>
        <item id="1623" type="EtcItem" name="Tataru's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1624" type="EtcItem" name="Flame Charm"><set name="is_questitem" val="true"/></item>
        <item id="1625" type="EtcItem" name="Grizzly Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1626" type="EtcItem" name="Blood Cauldron"><set name="is_questitem" val="true"/></item>
        <item id="1627" type="EtcItem" name="Spirit Net"><set name="is_questitem" val="true"/></item>
        <item id="1628" type="EtcItem" name="Bound Durka Spirit"><set name="is_questitem" val="true"/></item>
        <item id="1629" type="EtcItem" name="Durka Parasite"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1630" type="EtcItem" name="Totem Spirit Blood"><set name="is_questitem" val="true"/></item>
        <item id="1631" type="EtcItem" name="Mask of Medium"/>"#;

    struct Npcs {
        tataru: Oid,
        umos: Oid,
        hestui: Oid,
        duda: Oid,
        moira: Oid,
        gandi: Oid,
        carcass: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00416PathOfTheOrcShaman)
            .page("30585-01.htm", &["START"])
            .page("30585-06.htm", &["30585-07.htm"])
            .page("30585-11.html", &["30585-12.html"])
            .page("30585-12.html", &["30585-13.html", "30585-14.html"])
            .page("30502-06.html", &["30502-07.html"])
            .page("30592-01.html", &["30592-02.html"])
            .page("30592-02.html", &["30592-03.html"])
            .page("30593-01.html", &["30593-02.html"])
            .page("30593-02.html", &["30593-03.html"])
            .page("32057-01.html", &["32057-02.html"])
            .page("32057-04.html", &["32057-05.html"])
            .page("32090-04.html", &["32090-05.html"])
            .page("32090-05.html", &["32090-06.html"])
            .plain(&[
                "30585-02.htm",
                "30585-03.htm",
                "30585-04.htm",
                "30585-05.htm",
                "30585-07.htm",
                "30585-08.html",
                "30585-09.html",
                "30585-10.html",
                "30585-13.html",
                "30585-14.html",
                "30585-15.html",
                "30585-16.html",
                "30502-01.html",
                "30502-02.html",
                "30502-03.html",
                "30502-04.html",
                "30502-05.html",
                "30502-07.html",
                "30592-03.html",
                "30592-04.html",
                "30592-05.html",
                "30593-03.html",
                "30593-04.html",
                "30593-05.html",
                "30593-06.html",
                "31979-01.html",
                "31979-02.html",
                "31979-03.html",
                "32057-02.html",
                "32057-03.html",
                "32057-05.html",
                "32090-01.html",
                "32090-03.html",
                "32090-06.html",
                "32090-07.html",
                "32090-08.html",
            ])
            .npcs(&[
                TATARU_ZU_HESTUI,
                UMOS,
                HESTUI_TOTEM_SPIRIT,
                DUDA_MARA_TOTEM_SPIRIT,
                MOIRA,
                TOTEM_SPIRIT_OF_GANDI,
                DEAD_LEOPARDS_CARCASS,
            ])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00416PathOfTheOrcShaman);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            tataru: t.spawn(TATARU_ZU_HESTUI, 20),
            umos: t.spawn(UMOS, 40),
            hestui: t.spawn(HESTUI_TOTEM_SPIRIT, 60),
            duda: t.spawn(DUDA_MARA_TOTEM_SPIRIT, 80),
            moira: t.spawn(MOIRA, 100),
            gandi: t.spawn(TOTEM_SPIRIT_OF_GANDI, 120),
            carcass: t.spawn(DEAD_LEOPARDS_CARCASS, 140),
        };
        (t, npcs)
    }

    /// Kills `npc_id` as the player with the hooks called directly (no ticks: the spawns of the
    /// quest would attack the test character in the 2.5 s of `kill`). Returns the NPC.
    fn kill_now(t: &mut ScriptTest, npc_id: i32) -> Oid {
        let mob = t.spawn(npc_id, 80);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00416PathOfTheOrcShaman.on_kill(ctx, npc, Oid(1), false);
            }
        });
        t.world.delete_npc(mob);
        mob
    }

    /// Kills `npc_id` until the player has `count` of `item_id`.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) {
        for _ in 0..2000 {
            if t.count(1, item_id) >= count {
                return;
            }
            kill_now(t, npc_id);
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    /// Starts the quest from Tataru: START, then the start page (memo state 1, the fire charm).
    fn start(t: &mut ScriptTest, npcs: &Npcs) {
        assert!(t.quest_page(1, npcs.tataru).contains("30585-01.htm"));
        assert!(t.event(1, NAME, "START").contains("30585-06.htm"));
        assert!(t.event(1, NAME, "30585-07.htm").contains("30585-07.htm"));
        assert_eq!(t.count(1, FIRE_CHARM), 1);
    }

    #[test]
    fn the_start_page_depends_on_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ORC, player_class::ORC_MAGE, vec![]);
        t.quest_page(1, npcs.tataru);
        assert!(t.event(1, NAME, "START").contains("30585-04.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ORC, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.tataru);
        assert!(t.event(1, NAME, "START").contains("30585-02.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ORC, player_class::ORC_FIGHTER, vec![]);
        t.quest_page(1, npcs.tataru);
        assert!(t.event(1, NAME, "START").contains("30585-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ORC, player_class::ORC_MAGE, vec![]);
        t.quest_page(1, npcs.tataru);
        assert!(t.event(1, NAME, "START").contains("30585-06.htm"));
    }

    #[test]
    fn the_umos_path_from_the_fire_charm_to_the_mask() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ORC, player_class::ORC_MAGE, vec![]);
        start(&mut t, &npcs);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // The first phase: one egg, one husk and one pelt (100% each): cond 2 with the last.
        assert!(t.quest_page(1, npcs.tataru).contains("30585-08.html"));
        kill_now(&mut t, FIRST_FIERY_EGG_MOB);
        kill_now(&mut t, KASHA_BLADE_MOB);
        assert!(t.take_writes().is_empty());
        kill_now(&mut t, KASHA_BEAR_MOB);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Tataru takes the three and gives the hestui mask and the second egg: cond 3.
        assert!(t.quest_page(1, npcs.tataru).contains("30585-09.html"));
        assert_eq!(
            (
                t.count(1, FIRE_CHARM),
                t.count(1, HESTUI_MASK),
                t.count(1, SECOND_FIERY_EGG)
            ),
            (0, 1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The hestui totem spirit takes both and gives the claw: cond 4.
        assert!(t.quest_page(1, npcs.hestui).contains("30592-01.html"));
        assert!(t.event(1, NAME, "30592-02.html").contains("30592-02.html"));
        assert!(t.event(1, NAME, "30592-03.html").contains("30592-03.html"));
        assert_eq!(t.count(1, TOTEM_SPIRIT_CLAW), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Tataru takes the claw and gives the tataru's letter: cond 5.
        assert!(t.quest_page(1, npcs.tataru).contains("30585-11.html"));
        assert!(t.event(1, NAME, "30585-12.html").contains("30585-12.html"));
        assert!(t.event(1, NAME, "30585-13.html").contains("30585-13.html"));
        assert_eq!(t.count(1, TATARUS_LETTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Umos gives the flame charm: cond 6.
        assert!(t.quest_page(1, npcs.umos).contains("30502-01.html"));
        assert_eq!((t.count(1, TATARUS_LETTER), t.count(1, FLAME_CHARM)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Three grizzly bloods; the third sets cond 7.
        hunt(&mut t, GRIZZLY_MOB, GRIZZLY_BLOOD, 3);
        assert_eq!(t.count(1, GRIZZLY_BLOOD), 3);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Umos takes the charm and the blood and gives the blood cauldron: cond 8.
        assert!(t.quest_page(1, npcs.umos).contains("30502-03.html"));
        assert_eq!((t.count(1, FLAME_CHARM), t.count(1, BLOOD_CAULDRON)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);
        assert!(t.quest_page(1, npcs.umos).contains("30502-04.html"));

        // Duda takes the cauldron and gives the spirit net: cond 9.
        assert!(t.quest_page(1, npcs.duda).contains("30593-01.html"));
        assert!(t.event(1, NAME, "30593-02.html").contains("30593-02.html"));
        assert!(t.event(1, NAME, "30593-03.html").contains("30593-03.html"));
        assert_eq!((t.count(1, BLOOD_CAULDRON), t.count(1, SPIRIT_NET)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);

        // Poison spiders give parasites until the durka spirit comes; the spirit is bound.
        for _ in 0..2000 {
            if !t.npcs_of(DURKA_SPIRIT_MOB).is_empty() {
                break;
            }
            kill_now(&mut t, POISON_SPIDER_MOB);
        }
        let spirit = t.npcs_of(DURKA_SPIRIT_MOB)[0];
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(spirit) {
                Q00416PathOfTheOrcShaman.on_kill(ctx, npc, Oid(1), false);
            }
        });
        t.world.delete_npc(spirit);
        assert_eq!(
            (
                t.count(1, SPIRIT_NET),
                t.count(1, DURKA_PARASITE),
                t.count(1, BOUND_DURKA_SPIRIT)
            ),
            (0, 0, 1)
        );
        assert!(t.take_writes().is_empty());

        // Duda takes the bound spirit and gives the totem spirit blood: cond 11.
        assert!(t.quest_page(1, npcs.duda).contains("30593-05.html"));
        assert_eq!(
            (t.count(1, BOUND_DURKA_SPIRIT), t.count(1, TOTEM_SPIRIT_BLOOD)),
            (0, 1)
        );
        // The jump from cond 9 to 11 skips 10: the completed-state flags (0x800005FF).
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "11"),
                rows::insert(NAME, "__compltdStateFlags", "-2147482113"),
            ]
        );

        // Umos' reward: the mask, the exp, the adena, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.umos).contains("30502-06.html"));
        assert!(t.event(1, NAME, "30502-07.html").contains("30502-07.html"));
        assert_eq!(t.count(1, MASK_OF_MEDIUM), 1);
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        // Done: Tataru's talk is for the created state only.
        assert!(!t.quest_page(1, npcs.tataru).contains("30585-01.htm"));
    }

    #[test]
    fn the_moira_path_through_the_leopards() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ORC, player_class::ORC_MAGE, vec![]);
        start(&mut t, &npcs);
        t.take_writes();

        // Phase one as above, to the claw.
        kill_now(&mut t, FIRST_FIERY_EGG_MOB);
        kill_now(&mut t, KASHA_BLADE_MOB);
        kill_now(&mut t, KASHA_BEAR_MOB);
        assert!(t.quest_page(1, npcs.tataru).contains("30585-09.html"));
        assert!(t.quest_page(1, npcs.hestui).contains("30592-01.html"));
        assert!(t.event(1, NAME, "30592-02.html").contains("30592-02.html"));
        assert!(t.event(1, NAME, "30592-03.html").contains("30592-03.html"));
        t.take_writes();

        // Tataru's way to the leopards: the claw is taken, memo state 100, cond 12.
        assert!(t.quest_page(1, npcs.tataru).contains("30585-11.html"));
        assert!(t.event(1, NAME, "30585-12.html").contains("30585-12.html"));
        assert!(t.event(1, NAME, "30585-14.html").contains("30585-14.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "12"),
                rows::insert(NAME, "__compltdStateFlags", "-2147481585"),
                rows::update(NAME, "memoState", "100"),
            ]
        );

        // Moira: memo state 101, cond 13. Gandi: memo state 102, cond 14.
        assert!(t.quest_page(1, npcs.moira).contains("31979-01.html"));
        assert!(t.quest_page(1, npcs.moira).contains("31979-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "101"),
                rows::update(NAME, "cond", "13"),
                rows::update(NAME, "__compltdStateFlags", "-2147477489")
            ]
        );
        assert!(t.quest_page(1, npcs.gandi).contains("32057-01.html"));
        assert!(t.event(1, NAME, "32057-02.html").contains("32057-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "102"),
                rows::update(NAME, "cond", "14"),
                rows::update(NAME, "__compltdStateFlags", "-2147469297")
            ]
        );

        // Black leopards: 102 -> 103, then 103 -> 104 (cond 15).
        kill_now(&mut t, BLACK_LEOPARD_MOB);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "103")]);
        kill_now(&mut t, BLACK_LEOPARD_MOB);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "104"),
                rows::update(NAME, "cond", "15"),
                rows::update(NAME, "__compltdStateFlags", "-2147452913")
            ]
        );

        // The carcass: memo state 105, cond 16.
        assert!(t.quest_page(1, npcs.carcass).contains("32090-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "105"),
                rows::update(NAME, "cond", "16"),
                rows::update(NAME, "__compltdStateFlags", "-2147420145")
            ]
        );

        // Leopard 105 -> 106 (cond 17); the carcass gives the second part: memo 107, cond 18.
        kill_now(&mut t, BLACK_LEOPARD_MOB);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "106"),
                rows::update(NAME, "cond", "17"),
                rows::update(NAME, "__compltdStateFlags", "-2147354609")
            ]
        );
        assert!(t.quest_page(1, npcs.carcass).contains("32090-04.html"));
        assert!(t.event(1, NAME, "32090-05.html").contains("32090-05.html"));
        assert!(t.event(1, NAME, "32090-06.html").contains("32090-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "107"),
                rows::update(NAME, "cond", "18"),
                rows::update(NAME, "__compltdStateFlags", "-2147223537")
            ]
        );

        // Leopard 107 -> 108 (cond 19); the carcass: memo 109, cond 20.
        kill_now(&mut t, BLACK_LEOPARD_MOB);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "108"),
                rows::update(NAME, "cond", "19"),
                rows::update(NAME, "__compltdStateFlags", "-2146961393")
            ]
        );
        assert!(t.quest_page(1, npcs.carcass).contains("32090-08.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "109"),
                rows::update(NAME, "cond", "20"),
                rows::update(NAME, "__compltdStateFlags", "-2146437105")
            ]
        );

        // Gandi: memo 110, cond 21; then Moira's reward: the mask, the adena, the quest ends.
        assert!(t.quest_page(1, npcs.gandi).contains("32057-04.html"));
        assert!(t.event(1, NAME, "32057-05.html").contains("32057-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "110"),
                rows::update(NAME, "cond", "21"),
                rows::update(NAME, "__compltdStateFlags", "-2145388529")
            ]
        );
        assert!(t.quest_page(1, npcs.moira).contains("31979-03.html"));
        assert_eq!((t.count(1, MASK_OF_MEDIUM), t.count(1, 57)), (1, 81_900));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    // The mob ids of the chain, named for the tests.
    const FIRST_FIERY_EGG_MOB: i32 = 20415;
    const KASHA_BLADE_MOB: i32 = 20478;
    const KASHA_BEAR_MOB: i32 = 20479;
    const GRIZZLY_MOB: i32 = 20335;
    const POISON_SPIDER_MOB: i32 = 20038;
    const DURKA_SPIRIT_MOB: i32 = 27056;
    const BLACK_LEOPARD_MOB: i32 = 27319;
}
