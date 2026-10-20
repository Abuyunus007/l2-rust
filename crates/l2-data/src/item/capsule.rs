//! The `<capsuled_items>` of an etc item: what an extractable box can give.
//!
//! Java: the `capsuled_items` branch of `util/DocumentItem.java`,
//! `entity/item/holders/ExtractableProduct.java` and `EtcItem.addCapsuledItem`.

use super::EtcInfo;
use crate::loader::DataError;
use crate::stats::XmlNode;
use crate::stats::jnum::{decode_i32, decode_i64, parse_double};

/// Java `ExtractableProduct`: one `<item id min max chance minEnchant maxEnchant/>` of a box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractableProduct {
    /// The item created.
    pub id: i32,
    /// `min`: fewest items created (before `RateExtractable`).
    pub min: i64,
    /// `max`: most items created (before `RateExtractable`).
    pub max: i64,
    /// `(int) (chance * 1000)`: the XML gives a percent, `Rnd.get(100000)` is compared with this.
    pub chance: i32,
    /// `minEnchant` (default 0).
    pub min_enchant: i32,
    /// `maxEnchant` (default 0): above 0 the created items get a random enchant level.
    pub max_enchant: i32,
}

/// Java `DocumentItem`'s `capsuled_items` loop for one item. Like Java, only `<item>` children
/// count; a missing `id`, `min`, `max` or `chance` is an error (Java unboxes a null). The
/// products are only kept by etc items (`ItemTemplate.addCapsuledItem` does nothing).
pub(super) fn apply_capsuled_items(node: &XmlNode, etc: Option<&mut EtcInfo>) -> Result<(), DataError> {
    let mut products = Vec::new();
    for child in node.children.iter().filter(|c| c.name == "item") {
        let required = |name: &str| {
            child
                .attr(name)
                .ok_or_else(|| DataError::Invalid(format!("capsuled item without {name}")))
        };
        let int = |text: &str| {
            decode_i32(text)
                .ok_or_else(|| DataError::Invalid(format!("integer value required, found: {text}")))
        };
        let long = |text: &str| {
            decode_i64(text).ok_or_else(|| DataError::Invalid(format!("long value required, found: {text}")))
        };
        let id = int(required("id")?)?;
        let min = long(required("min")?)?;
        let max = long(required("max")?)?;
        let chance_text = required("chance")?;
        let chance = parse_double(chance_text)
            .ok_or_else(|| DataError::Invalid(format!("number required, found: {chance_text}")))?;
        let min_enchant = child.attr("minEnchant").map(int).transpose()?.unwrap_or(0);
        let max_enchant = child.attr("maxEnchant").map(int).transpose()?.unwrap_or(0);
        products.push(ExtractableProduct {
            id,
            min,
            max,
            // Java `(int) (chance * 1000)` saturates like `as`.
            chance: (chance * 1000.0) as i32,
            min_enchant,
            max_enchant,
        });
    }
    if let Some(etc) = etc {
        etc.extractable_items.extend(products);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::EtcItemType;
    use super::*;

    fn etc() -> EtcInfo {
        EtcInfo {
            etc_type: EtcItemType::None,
            handler: None,
            extractable_count_min: 0,
            extractable_count_max: 0,
            extractable_items: Vec::new(),
            blessed: false,
        }
    }

    fn node(items: &[&[(&str, &str)]]) -> XmlNode {
        XmlNode::new(
            "capsuled_items",
            &[],
            items
                .iter()
                .map(|a| XmlNode::new("item", a, Vec::new()))
                .collect(),
        )
    }

    #[test]
    fn products_are_read_with_java_scaling_and_defaults() {
        let mut info = etc();
        let n = node(&[
            &[("id", "57"), ("min", "10"), ("max", "20"), ("chance", "12.5")],
            &[
                ("id", "0x10"),
                ("min", "1"),
                ("max", "1"),
                ("chance", "100"),
                ("minEnchant", "1"),
                ("maxEnchant", "3"),
            ],
        ]);
        apply_capsuled_items(&n, Some(&mut info)).unwrap();
        assert_eq!(
            info.extractable_items,
            vec![
                ExtractableProduct {
                    id: 57,
                    min: 10,
                    max: 20,
                    chance: 12_500,
                    min_enchant: 0,
                    max_enchant: 0
                },
                ExtractableProduct {
                    id: 16,
                    min: 1,
                    max: 1,
                    chance: 100_000,
                    min_enchant: 1,
                    max_enchant: 3
                },
            ]
        );
    }

    #[test]
    fn a_missing_attribute_is_an_error_and_other_nodes_are_skipped() {
        let mut info = etc();
        let n = node(&[&[("id", "57"), ("min", "1"), ("max", "1")]]);
        assert!(apply_capsuled_items(&n, Some(&mut info)).is_err());
        let mut other = XmlNode::new("capsuled_items", &[], Vec::new());
        other
            .children
            .push(XmlNode::new("Item", &[("id", "x")], Vec::new()));
        apply_capsuled_items(&other, Some(&mut info)).unwrap();
        assert!(info.extractable_items.is_empty());
    }

    #[test]
    fn only_etc_items_keep_the_products() {
        let n = node(&[&[("id", "57"), ("min", "1"), ("max", "1"), ("chance", "100")]]);
        apply_capsuled_items(&n, None).unwrap();
    }
}
