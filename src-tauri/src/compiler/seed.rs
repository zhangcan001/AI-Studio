use crate::domain::SeedValue;
use uuid::Uuid;

/// Maximum value accepted by ComfyUI's `Seed (rgthree)` node.
pub const COMFYUI_MAX_SEED: u64 = 1_125_899_906_842_624;

fn normalize_random_seed(seed: u64) -> u64 {
    seed % (COMFYUI_MAX_SEED + 1)
}

#[derive(Default)]
pub struct SeedResolver {
    random_seed: Option<u64>,
}

impl SeedResolver {
    pub fn resolve(&mut self, value: &SeedValue) -> u64 {
        match value {
            SeedValue::Fixed(seed) => *seed,
            SeedValue::Random => *self.random_seed.get_or_insert_with(|| {
                // UUID v4 is already an existing project dependency and supplies
                // cryptographically random bytes without adding a seed-only crate.
                // ComfyUI's rgthree seed input is narrower than u64, so normalize
                // the generated value before it reaches the compiled workflow.
                normalize_random_seed(Uuid::new_v4().as_u128() as u64)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_random_seed, SeedResolver, COMFYUI_MAX_SEED};
    use crate::domain::SeedValue;

    #[test]
    fn fixed_seed_is_preserved() {
        let mut resolver = SeedResolver::default();

        assert_eq!(resolver.resolve(&SeedValue::Fixed(123)), 123);
    }

    #[test]
    fn random_seed_is_resolved_once_per_resolver() {
        let mut resolver = SeedResolver::default();

        let first = resolver.resolve(&SeedValue::Random);
        let second = resolver.resolve(&SeedValue::Random);

        assert_eq!(first, second);
    }

    #[test]
    fn random_seed_stays_within_comfyui_limit() {
        let mut resolver = SeedResolver::default();

        let seed = resolver.resolve(&SeedValue::Random);

        assert!(seed <= COMFYUI_MAX_SEED);
    }

    #[test]
    fn random_seed_normalization_handles_full_u64_range() {
        for raw_seed in [0, COMFYUI_MAX_SEED, COMFYUI_MAX_SEED + 1, u64::MAX] {
            assert!(normalize_random_seed(raw_seed) <= COMFYUI_MAX_SEED);
        }
    }
}
