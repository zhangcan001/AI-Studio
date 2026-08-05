use crate::domain::SeedValue;
use uuid::Uuid;

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
                Uuid::new_v4().as_u128() as u64
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SeedResolver;
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
}
