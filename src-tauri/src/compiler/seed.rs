use crate::domain::SeedValue;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Default)]
pub struct SeedResolver {
    random_seeds: BTreeMap<String, u64>,
}

impl SeedResolver {
    pub fn resolve(
        &mut self,
        input_key: &str,
        value: &SeedValue,
        min: Option<u64>,
        max: Option<u64>,
    ) -> u64 {
        match value {
            SeedValue::Fixed(seed) => *seed,
            SeedValue::Random => *self
                .random_seeds
                .entry(input_key.to_owned())
                .or_insert_with(|| {
                    // UUID v4 is already an existing project dependency and supplies
                    // cryptographically random bytes without adding a seed-only crate.
                    resolve_random_seed(Uuid::new_v4().as_u128() as u64, min, max)
                }),
        }
    }
}

fn resolve_random_seed(raw_seed: u64, min: Option<u64>, max: Option<u64>) -> u64 {
    let lower = min.unwrap_or(0);
    let upper = max.unwrap_or(u64::MAX);

    // RecipeValidator rejects inverted ranges. Keep this helper total as well so
    // an invalid in-memory Recipe cannot cause an arithmetic panic.
    if lower > upper {
        return raw_seed;
    }

    // Use u128 for the inclusive span: 0..=u64::MAX has 2^64 values and
    // overflows a u64 when adding one.
    let span = (upper as u128) - (lower as u128) + 1;
    ((lower as u128) + (raw_seed as u128 % span)) as u64
}

#[cfg(test)]
mod tests {
    use super::{resolve_random_seed, SeedResolver};
    use crate::domain::SeedValue;

    #[test]
    fn fixed_seed_is_preserved() {
        let mut resolver = SeedResolver::default();

        assert_eq!(
            resolver.resolve("seed", &SeedValue::Fixed(123), None, None),
            123
        );
    }

    #[test]
    fn random_seed_is_resolved_once_per_input() {
        let mut resolver = SeedResolver::default();

        let first = resolver.resolve("seed", &SeedValue::Random, Some(10), Some(20));
        let second = resolver.resolve("seed", &SeedValue::Random, Some(10), Some(20));

        assert_eq!(first, second);
        assert!((10..=20).contains(&first));
    }

    #[test]
    fn different_random_inputs_have_independent_cache_entries() {
        let mut resolver = SeedResolver::default();

        resolver.resolve("noise_seed", &SeedValue::Random, None, None);
        resolver.resolve("motion_seed", &SeedValue::Random, None, None);

        assert_eq!(resolver.random_seeds.len(), 2);
        assert!(resolver.random_seeds.contains_key("noise_seed"));
        assert!(resolver.random_seeds.contains_key("motion_seed"));
    }

    #[test]
    fn random_seed_respects_min_and_max() {
        for raw_seed in [0, 1, 20, u64::MAX] {
            let resolved = resolve_random_seed(raw_seed, Some(10), Some(20));
            assert!((10..=20).contains(&resolved));
        }
    }

    #[test]
    fn random_seed_supports_unbounded_u64_range() {
        assert_eq!(resolve_random_seed(0, None, None), 0);
        assert_eq!(resolve_random_seed(u64::MAX, None, None), u64::MAX);
    }

    #[test]
    fn random_seed_uses_u128_for_full_range_math() {
        assert_eq!(
            resolve_random_seed(u64::MAX, Some(0), Some(u64::MAX)),
            u64::MAX
        );
        assert_eq!(
            resolve_random_seed(0, Some(u64::MAX), Some(u64::MAX)),
            u64::MAX
        );
    }
}
