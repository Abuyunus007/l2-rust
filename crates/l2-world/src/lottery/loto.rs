//! The `Loto` NPC bypass: the lottery sales windows, the ticket list and the prize claim.
// Java: dist/game/data/scripts/handlers/bypass/npc/Loto.java
//
// The pages (values of the command): 0 first buy window, 1-20 the number buttons, 21 the second
// buy window, 22 the chosen ticket, 23 the jackpot, 24 earlier winning numbers and the claim
// list, 25 the instructions, above 25 the object id of a ticket to claim.
//
// Pages 24 and the claim need the winning numbers of earlier draws; they are read for the request
// (`PersistCommand::LotteryRequest`, D-50) and the page is built when they come back.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{LOTTERY_TICKET, LotteryResult, LotteryThen, check_ticket, decode_numbers, encode_numbers};
use crate::outbox::PersistCommand;
use crate::world::World;

/// Java `Loto.onCommand`.
pub(crate) fn loto_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    // `Integer.parseInt(command.substring(5))`: anything that fails is 0.
    let val = command.get(5..).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    if val == 0 {
        // new loto ticket
        *world.lottery_picks(player) = [0; 5];
    }
    world.loto_window_with(player, npc, val, None);
    false
}

/// `DateFormat.getDateInstance().format(date)` for the US locale (`Nov 1, 2026`), in the
/// machine's zone.
fn format_date(unix_ms: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let local = unix_ms.wrapping_add(l2_util::time::local_offset_millis(unix_ms));
    let days = local.div_euclid(86_400_000);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let name = MONTHS
        .get(usize::try_from(month - 1).unwrap_or(0))
        .copied()
        .unwrap_or("");
    format!("{name} {day}, {year}")
}

impl World {
    /// `Npc.getHtmlPath(npcId, n)` and the file: the page text, or Java's "missing" page.
    fn loto_page(&self, player: ObjectId, npc: ObjectId, n: i32) -> Option<String> {
        let path = self.npc_html_path(npc, n)?;
        self.html_from_file(player, npc.0, &path).map(|m| m.html)
    }

    /// Java `Loto.showLotoWindow(player, npc, value)`. `rows` are the draws' numbers, read for
    /// page 24 and the claim (`None` asks for them and returns).
    pub(crate) fn loto_window_with(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        value: i32,
        rows: Option<&[LotteryResult]>,
    ) {
        let settings = self.lottery.settings.clone();
        let mut html: String;
        if value < 0 {
            // NOTE(java-quirk): Java sends an `NpcHtmlMessage` without a page for a negative
            // value. Nothing is sent here.
            return;
        } else if value == 0 {
            // 0 - first buy lottery ticket window
            let Some(page) = self.loto_page(player, npc, 1) else {
                return;
            };
            html = page;
        } else if (1..=21).contains(&value) {
            // 1-20 - buttons, 21 - second buy lottery ticket window
            if !self.lottery.started {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::LotteryTicketsAreNotCurrentlyBeingSold),
                );
                return;
            }
            if !self.lottery.selling {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TicketsForTheCurrentLotteryAreNoLongerAvailable),
                );
                return;
            }
            let Some(page) = self.loto_page(player, npc, 5) else {
                return;
            };
            html = page;

            let mut count = 0;
            let mut found = false;
            let picks = self.lottery_picks(player);
            // counting buttons and unsetting button if found
            for pick in picks.iter_mut() {
                if *pick == value {
                    // unsetting button
                    *pick = 0;
                    found = true;
                } else if *pick > 0 {
                    count += 1;
                }
            }
            // if not reached limit 5 and not unset value
            if count < 5
                && !found
                && value <= 20
                && let Some(slot) = picks.iter_mut().find(|p| **p == 0)
            {
                *slot = value;
            }
            // setting pushed buttons
            let picks = *picks;
            let mut count = 0;
            for pick in picks {
                if pick > 0 {
                    count += 1;
                    let button = if pick < 10 {
                        format!("0{pick}")
                    } else {
                        pick.to_string()
                    };
                    html = html.replace(
                        &format!("fore=\"L2UI.lottoNum{button}\" back=\"L2UI.lottoNum{button}a_check\""),
                        &format!("fore=\"L2UI.lottoNum{button}a_check\" back=\"L2UI.lottoNum{button}\""),
                    );
                }
            }
            if count == 5 {
                html = html.replace("0\">Return", "22\">Your lucky numbers have been selected above.");
            }
        } else if value == 22 {
            // 22 - selected ticket with 5 numbers
            if !self.lottery.started {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::LotteryTicketsAreNotCurrentlyBeingSold),
                );
                return;
            }
            if !self.lottery.selling {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TicketsForTheCurrentLotteryAreNoLongerAvailable),
                );
                return;
            }
            let price = settings.ticket_price;
            let lotonumber = self.lottery.number;
            let picks = *self.lottery_picks(player);
            if picks.contains(&0) {
                return;
            }
            let (enchant, type2) = encode_numbers(&picks);
            if self.adena_of(player) < price {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                );
                return;
            }
            if !self.reduce_adena(player, price, true) {
                return;
            }
            self.lottery_increase_prize(price);
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS1)
                    .with(SystemMessageParam::ItemName(LOTTERY_TICKET)),
            );
            self.loto_give_ticket(player, lotonumber, enchant, type2);
            let Some(page) = self.loto_page(player, npc, 6) else {
                return;
            };
            html = page;
        } else if value == 23 {
            // 23 - current lottery jackpot
            let Some(page) = self.loto_page(player, npc, 3) else {
                return;
            };
            html = page;
        } else if value == 24 {
            // 24 - Previous winning numbers/Prize claim
            let Some(rows) = rows else {
                self.outbox.persist(PersistCommand::LotteryRequest {
                    then: LotteryThen::Page { player, npc },
                });
                return;
            };
            let Some(page) = self.loto_page(player, npc, 4) else {
                return;
            };
            html = page;

            let lotonumber = self.lottery.number;
            let mut message = String::new();
            // NOTE: Java walks the inventory map; the order here is the inventory's.
            let tickets: Vec<_> = self
                .players
                .get(&player)
                .map(|p| {
                    p.data
                        .items
                        .iter()
                        .filter(|i| i.item_id == LOTTERY_TICKET && i.custom_type1 < lotonumber)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            for item in tickets {
                message.push_str(&format!(
                    "<a action=\"bypass -h npc_%objectId%_Loto {}\">{} Event Number ",
                    item.object_id.0, item.custom_type1
                ));
                for number in decode_numbers(item.enchant_level, item.custom_type2) {
                    message.push_str(&format!("{number} "));
                }
                let check = check_ticket(
                    rows,
                    item.custom_type1,
                    item.enchant_level,
                    item.custom_type2,
                    settings.two_and_one_prize,
                );
                if check[0] > 0 {
                    // NOTE(java-quirk): "3th Prize".
                    match check[0] {
                        1 => message.push_str("- 1st Prize"),
                        2 => message.push_str("- 2nd Prize"),
                        3 => message.push_str("- 3th Prize"),
                        4 => message.push_str("- 4th Prize"),
                        _ => {}
                    }
                    message.push_str(&format!(" {}a.", check[1]));
                }
                message.push_str("</a><br>");
            }
            if message.is_empty() {
                message.push_str("There has been no winning lottery ticket.<br>");
            }
            html = html.replace("%result%", &message);
        } else if value == 25 {
            // 25 - lottery instructions
            let Some(page) = self.loto_page(player, npc, 2) else {
                return;
            };
            html = page;
        } else {
            // >25 - check lottery ticket by item object id
            let lotonumber = self.lottery.number;
            let Some(item) = self.inventory_item(player, ObjectId(value)).cloned() else {
                return;
            };
            if item.item_id != LOTTERY_TICKET || item.custom_type1 >= lotonumber {
                return;
            }
            let Some(rows) = rows else {
                self.outbox.persist(PersistCommand::LotteryRequest {
                    then: LotteryThen::Claim {
                        player,
                        npc,
                        object_id: value,
                    },
                });
                return;
            };
            let check = check_ticket(
                rows,
                item.custom_type1,
                item.enchant_level,
                item.custom_type2,
                settings.two_and_one_prize,
            );
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                    .with(SystemMessageParam::ItemName(LOTTERY_TICKET)),
            );
            let adena = check[1];
            if adena > 0 {
                self.add_adena(player, adena, true);
            }
            self.loto_destroy_ticket(player, item.object_id);
            return;
        }

        html = html.replace("%objectId%", &npc.0.to_string());
        html = html.replace("%race%", &self.lottery.number.to_string());
        html = html.replace("%adena%", &self.lottery.prize.to_string());
        html = html.replace("%ticket_price%", &settings.ticket_price.to_string());
        // `Float.toString(rate * 100)`.
        html = html.replace("%prize5%", &format!("{:?}", settings.rate_5 * 100.0));
        html = html.replace("%prize4%", &format!("{:?}", settings.rate_4 * 100.0));
        html = html.replace("%prize3%", &format!("{:?}", settings.rate_3 * 100.0));
        html = html.replace("%prize2%", &settings.two_and_one_prize.to_string());
        html = html.replace("%enddate%", &format_date(self.lottery.enddate));
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        // Send a Server->Client ActionFailed to the Player in order to avoid that the client wait
        // another packet.
        self.send(player, &ActionFailed);
    }

    /// The ticket of page 22: a new item 4442 with the draw, the picks and the update packets.
    fn loto_give_ticket(&mut self, player: ObjectId, lotonumber: i32, enchant: i32, type2: i32) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(LOTTERY_TICKET) else {
            return;
        };
        let Some(id) = self.allocate_item_id() else {
            return;
        };
        let mut data = self.new_item_data(template, id, 1);
        data.loc = "INVENTORY".to_owned();
        data.loc_data = 0;
        data.custom_type1 = lotonumber;
        data.enchant_level = enchant;
        data.custom_type2 = type2;
        self.add_item_object(player, data, false);
    }

    /// `player.destroyItem(FEE, item, npc, false)`: the whole ticket goes, without a message.
    fn loto_destroy_ticket(&mut self, player: ObjectId, object_id: ObjectId) {
        let Some(removed) = self.destroy_inventory_item(player, object_id, 1) else {
            return;
        };
        let entry = self.update_entry(&removed, l2_protocol_game::server::InventoryUpdate::REMOVED);
        self.send_inventory_update_now(player, vec![entry]);
        self.send_cur_load(player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_formatted_like_the_us_medium_style() {
        // 2026-11-01 12:00 UTC: the date is Nov 1 in every zone from UTC-12 to UTC+11.
        let text = format_date(1_793_534_400_000);
        assert!(
            text == "Nov 1, 2026" || text == "Nov 2, 2026" || text == "Oct 31, 2026",
            "{text}"
        );
        assert_eq!(super::super::next_draw_end(0, |_| 0) % 86_400_000, 19 * 3_600_000);
    }
}
