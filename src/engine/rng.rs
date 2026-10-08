//! Port of keybr-rand: the LCG generator and sampling helpers.

/// A source of uniform random numbers in `[0, 1)`.
pub trait Rng {
    fn next_f64(&mut self) -> f64;
}

/// keybr's linear congruential generator.
///
/// JavaScript evaluates `a * x + c` in doubles, which loses precision above
/// 2^53 before the bit mask is applied. We reproduce that exactly so seeded
/// sequences match keybr.
#[derive(Debug, Clone)]
pub struct Lcg {
    x: u32,
}

impl Lcg {
    pub fn new(seed: u32) -> Self {
        Self {
            x: seed ^ 0x3fc2_8cf6,
        }
    }
}

impl Rng for Lcg {
    fn next_f64(&mut self) -> f64 {
        const A: f64 = 0x41c6_4e6d as f64;
        const C: f64 = 0x3039 as f64;
        // Rounded like a JS double, then ToInt32 + mask.
        let product = A * self.x as f64 + C;
        self.x = (product as i64 as u64 & 0x7fff_ffff) as u32;
        self.x as f64 / 2_147_483_648.0
    }
}

/// A good-quality generator for real lessons.
///
/// keybr's LCG loses precision in its multiply, so its sequences cycle after
/// about 10k values (only 220 from some seeds), which makes lesson text
/// repeat. `Lcg` is kept only to check results against keybr's test vectors.
pub struct LessonRng(rand::rngs::SmallRng);

impl LessonRng {
    pub fn from_entropy() -> Self {
        use rand::SeedableRng;
        Self(rand::rngs::SmallRng::from_os_rng())
    }

    pub fn seeded(seed: u64) -> Self {
        use rand::SeedableRng;
        Self(rand::rngs::SmallRng::seed_from_u64(seed))
    }
}

impl Rng for LessonRng {
    fn next_f64(&mut self) -> f64 {
        rand::Rng::random::<f64>(&mut self.0)
    }
}

/// Picks a uniformly random element. Panics on an empty slice.
pub fn random_sample<'a, T>(list: &'a [T], rng: &mut impl Rng) -> &'a T {
    assert!(!list.is_empty(), "random_sample on empty list");
    &list[(rng.next_f64() * list.len() as f64) as usize]
}

/// Picks an element with probability proportional to `weight`.
/// Panics on an empty slice.
pub fn weighted_sample<'a, T>(
    list: &'a [T],
    weight: impl Fn(&T) -> f64,
    rng: &mut impl Rng,
) -> &'a T {
    assert!(!list.is_empty(), "weighted_sample on empty list");
    let sum: f64 = list.iter().map(&weight).sum();
    let mut r = rng.next_f64() * sum;
    for v in list {
        let w = weight(v);
        if r <= w {
            return v;
        }
        r -= w;
    }
    // Only reachable through float rounding; keybr throws here.
    list.last().unwrap()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Returns the given values in a cycle.
    pub struct FakeRng(pub Vec<f64>, pub usize);

    impl Rng for FakeRng {
        fn next_f64(&mut self) -> f64 {
            let v = self.0[self.1 % self.0.len()];
            self.1 += 1;
            v
        }
    }

    #[test]
    fn lcg_seed_zero_matches_keybr() {
        let mut r = Lcg::new(0);
        let got: Vec<f64> = (0..10).map(|_| r.next_f64()).collect();
        assert_eq!(
            got,
            [
                0.8207141160964966,
                0.8991895914077759,
                0.26380741596221924,
                0.2583709955215454,
                0.42385780811309814,
                0.9650942087173462,
                0.1808091402053833,
                0.6519886553287506,
                0.7223324775695801,
                0.956657886505127,
            ]
        );
    }

    #[test]
    fn lcg_values_unique_and_in_range() {
        let mut r = Lcg::new(123);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10000 {
            let v = r.next_f64();
            assert!((0.0..1.0).contains(&v));
            assert!(seen.insert(v.to_bits()));
        }
    }

    #[test]
    fn lesson_rng_is_seedable_and_does_not_cycle_early() {
        let a: Vec<f64> = {
            let mut r = LessonRng::seeded(7);
            (0..5).map(|_| r.next_f64()).collect()
        };
        let b: Vec<f64> = {
            let mut r = LessonRng::seeded(7);
            (0..5).map(|_| r.next_f64()).collect()
        };
        assert_eq!(a, b);
        let mut r = LessonRng::seeded(7);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100_000 {
            let v = r.next_f64();
            assert!((0.0..1.0).contains(&v));
            assert!(seen.insert(v.to_bits()));
        }
        let mut e = LessonRng::from_entropy();
        assert!((0.0..1.0).contains(&e.next_f64()));
    }

    #[test]
    fn random_sample_uses_floor() {
        let list = ['a', 'b', 'c', 'd'];
        assert_eq!(*random_sample(&list, &mut FakeRng(vec![0.0], 0)), 'a');
        assert_eq!(*random_sample(&list, &mut FakeRng(vec![0.49], 0)), 'b');
        assert_eq!(*random_sample(&list, &mut FakeRng(vec![0.99], 0)), 'd');
    }

    #[test]
    fn weighted_sample_walks_with_r_le_w() {
        let list = [('a', 1.0), ('b', 2.0), ('c', 1.0)];
        let w = |x: &(char, f64)| x.1;
        // sum = 4; r = rand * 4
        assert_eq!(weighted_sample(&list, w, &mut FakeRng(vec![0.0], 0)).0, 'a');
        assert_eq!(
            weighted_sample(&list, w, &mut FakeRng(vec![0.25], 0)).0,
            'a'
        ); // r=1 <= 1
        assert_eq!(weighted_sample(&list, w, &mut FakeRng(vec![0.5], 0)).0, 'b'); // r=2 -> 1 <= 2
        assert_eq!(weighted_sample(&list, w, &mut FakeRng(vec![0.9], 0)).0, 'c'); // r=3.6 -> 0.6
    }
}
