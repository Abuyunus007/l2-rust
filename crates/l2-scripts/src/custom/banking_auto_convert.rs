//! `config/Custom/Banking.ini` `BankingAutoConvert`: a player whose purse goes over
//! `BankingAutoConvertAdenaLimit` Adena gives `BankingAdenaCount` Adena for the Gold Bars.
// Java registers the listener only with `BankingAutoConvert = True`; the registry here is built
// before the config is known to a script, so the script is always registered and the world
// checks the switch (`World::banking_auto_convert`) when the event comes.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptItem, ScriptSetup};

/// `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `custom.BankingAutoConvert`.
pub struct BankingAutoConvert;

impl Script for BankingAutoConvert {
    fn name(&self) -> &'static str {
        "BankingAutoConvert"
    }

    fn path(&self) -> &'static str {
        "custom"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        // `Containers.Players().addListener(... ON_PLAYER_ITEM_ADD ...)`
        s.set_on_player_item_add();
    }

    fn on_player_item_add(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, item: ScriptItem) {
        if item.id != ADENA_ID {
            return;
        }
        ctx.world_mut().banking_auto_convert(player);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, WorldSettings};

    use super::*;
    use crate::testing::{ScriptTest, item};

    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="3470" type="EtcItem" name="Gold Bar"><set name="is_stackable" val="true" /></item>"#;

    fn test(switch: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(BankingAutoConvert);
        let mut t = ScriptTest::new_with(registry, "", ITEMS, &[], |world, _| {
            let mut settings = WorldSettings::default();
            settings.custom_misc.banking.auto_convert_enabled = switch;
            settings.custom_misc.banking.auto_convert_adena_limit = 5_000;
            settings.custom_misc.banking.adena_count = 1_000;
            settings.custom_misc.banking.goldbar_count = 1;
            world.set_settings(settings);
        });
        t.enter(1, 80, vec![item(100, 57, 5_000)]);
        t.ticks(2);
        t.take();
        t
    }

    #[test]
    fn switch_off_nothing_is_converted() {
        let mut t = test(false);
        t.world.add_adena(ObjectId(1), 10, false);
        t.ticks(2);
        assert_eq!((t.count(1, 57), t.count(1, 3470)), (5_010, 0));
    }

    #[test]
    fn switch_on_converts_a_purse_over_the_limit() {
        let mut t = test(true);
        // The limit itself is not over it.
        t.world.add_adena(ObjectId(1), 0, false);
        t.ticks(2);
        assert_eq!((t.count(1, 57), t.count(1, 3470)), (5_000, 0));
        t.world.add_adena(ObjectId(1), 1, false);
        t.ticks(2);
        assert_eq!((t.count(1, 57), t.count(1, 3470)), (4_001, 1));
    }
}
