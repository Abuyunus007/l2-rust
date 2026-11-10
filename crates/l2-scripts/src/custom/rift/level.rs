//! Which zone fits the players, and which level a rift's monsters take, without the world.

/// How many of `levels` count for each zone (`ranges`: the lowest and highest level of each
/// zone). A player counts for one zone only: the one nearest his level (0 inside its range), the
/// lower one of two as near; and only when he is at most `margin` levels outside its range.
pub fn zone_weights(ranges: &[(i32, i32)], levels: &[i32], margin: i32) -> Vec<usize> {
    let mut weights = vec![0; ranges.len()];
    for level in levels {
        let distance = |(min, max): &(i32, i32)| (min - level).max(level - max).max(0);
        let nearest = ranges
            .iter()
            .enumerate()
            .map(|(index, range)| (distance(range), range.0, index))
            .min();
        if let Some((distance, _, index)) = nearest
            && distance <= margin
        {
            weights[index] += 1;
        }
    }
    weights
}

/// The zone `roll` (in `0..total`, `total` the sum of `weights`) lands on: a zone is as likely as
/// its weight. `None` when no zone has a weight.
pub fn pick_weighted(weights: &[usize], roll: usize) -> Option<usize> {
    let mut left = roll;
    for (index, weight) in weights.iter().enumerate() {
        if left < *weight {
            return Some(index);
        }
        left -= weight;
    }
    None
}

/// The median of `levels` (of an even count the lower of the two middle ones), `None` for none.
pub fn median(levels: &[i32]) -> Option<i32> {
    let mut sorted = levels.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

/// An adapted level: `level`, but at most `range` below the zone's lowest and above its highest.
pub fn adapt(level: i32, min: i32, max: i32, range: i32) -> i32 {
    level.clamp(min - range, max + range)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped zones: 40s, 50s, 60s, 70s, 80s.
    const ZONES: [(i32, i32); 5] = [(40, 46), (50, 56), (60, 66), (70, 76), (80, 84)];

    #[test]
    fn players_count_for_the_zones_their_level_fits() {
        // Ten in their 60s, two in their 80s, one level 20 who fits nothing.
        let mut levels = vec![61; 10];
        levels.extend([82, 85, 20]);
        assert_eq!(zone_weights(&ZONES, &levels, 5), vec![0, 0, 10, 0, 2]);
        // A level 58 is as near the 50s as the 60s: the lower zone. 59 is nearer the 60s.
        assert_eq!(zone_weights(&ZONES, &[58], 5), vec![0, 1, 0, 0, 0]);
        assert_eq!(zone_weights(&ZONES, &[59], 5), vec![0, 0, 1, 0, 0]);
        // Without the margin, only a level inside a range counts.
        assert_eq!(zone_weights(&ZONES, &[58], 0), vec![0, 0, 0, 0, 0]);
        // Below every zone: 35 counts for the 40s (5 below), 34 for nothing.
        assert_eq!(zone_weights(&ZONES, &[35, 34], 5), vec![1, 0, 0, 0, 0]);
        assert_eq!(zone_weights(&ZONES, &[], 5), vec![0; 5]);
    }

    #[test]
    fn a_zone_is_as_likely_as_its_weight() {
        let weights = [0, 0, 10, 0, 2];
        let picks: Vec<Option<usize>> = (0..12).map(|roll| pick_weighted(&weights, roll)).collect();
        assert_eq!(picks.iter().filter(|p| **p == Some(2)).count(), 10);
        assert_eq!(picks.iter().filter(|p| **p == Some(4)).count(), 2);
        assert_eq!(pick_weighted(&weights, 12), None);
        assert_eq!(pick_weighted(&[0, 0], 0), None);
    }

    #[test]
    fn the_monsters_take_the_median_kept_near_the_zone() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[60]), Some(60));
        assert_eq!(median(&[80, 58, 61]), Some(61));
        // Even: the lower middle one (a strong player does not lift the rift).
        assert_eq!(median(&[58, 61, 64, 85]), Some(61));
        // A level 80 rift with level 60 players: at most 5 below the zone.
        assert_eq!(adapt(60, 80, 84, 5), 75);
        assert_eq!(adapt(82, 80, 84, 5), 82);
        assert_eq!(adapt(99, 80, 84, 5), 89);
        assert_eq!(adapt(60, 80, 84, 0), 80);
    }
}
