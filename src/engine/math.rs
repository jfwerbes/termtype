//! Port of keybr-math `filter.ts`.

/// Exponential moving average. The first sample is taken as-is.
/// See <https://en.wikipedia.org/wiki/Exponential_smoothing>.
#[derive(Debug, Clone)]
pub struct Filter {
    alpha: f64,
    n: usize,
    value: f64,
}

impl Filter {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            n: 0,
            value: f64::NAN,
        }
    }

    /// Number of samples added so far.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Adds a sample and returns the new smoothed value.
    pub fn add(&mut self, v: f64) -> f64 {
        self.n += 1;
        self.value = if self.n > 1 {
            self.alpha * v + (1.0 - self.alpha) * self.value
        } else {
            v
        };
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_keybr_vectors() {
        let mut f = Filter::new(0.5);
        assert_eq!(f.n(), 0);
        let expected = [
            (1.0, 1.0),
            (1.0, 1.0),
            (1.0, 1.0),
            (5.0, 3.0),
            (5.0, 4.0),
            (5.0, 4.5),
        ];
        for (i, (input, out)) in expected.into_iter().enumerate() {
            assert_eq!(f.add(input), out);
            assert_eq!(f.n(), i + 1);
        }
    }
}
