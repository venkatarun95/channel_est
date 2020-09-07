use crate::config::ChannelEstConfig;
use num::{Complex, One};

/// CFO correct using the short and long preambles. Returns the per-sample phase shift due to CFO
/// (hence correction should be in the opposite direction)
pub fn estimate_cfo(
    short: &[Complex<f32>],
    long: &[Complex<f32>],
    config: &ChannelEstConfig,
) -> f32 {
    // Coarse estimation using the short preamble
    let sts_len = config.sts.as_ref().unwrap().len();
    assert_eq!(short.len(), 10 * sts_len as usize);
    let coarse = (0..9 * sts_len as usize)
        .map(|i| short[i].conj() * short[i + sts_len as usize])
        .sum::<Complex<_>>()
        .arg()
        / sts_len as f32;

    // Correct the long preamble using the coarse estimate and estimate the residual CFO
    let lts_len = config.lts.as_ref().unwrap().0.len();
    assert_eq!(lts_len % 2, 0);
    assert_eq!(long.len(), 5 * lts_len / 2);
    // CFO correction for config.lts.len() samples
    let coarse_lts_corr = Complex::new(1., -coarse * lts_len as f32).exp();
    let fine = (lts_len / 2..3 * lts_len / 2)
        .map(|i| long[i].conj() * long[i + lts_len] * coarse_lts_corr)
        .sum::<Complex<_>>()
        .arg()
        / lts_len as f32;

    coarse + fine
}

/// Take a buffer and CFO estimate (in radians per sample) and correct the samples for the CFO
pub fn correct_cfo(samps: &[Complex<f32>], cfo: f32) -> Vec<Complex<f32>> {
    let cfo = Complex::new(0., -cfo).exp();
    let mut corr = Complex::one();
    let mut res = Vec::with_capacity(samps.len());
    for s in samps {
        res.push(s * corr);
        corr = corr * cfo;
    }
    res
}

#[cfg(test)]
mod test {
    use super::*;
    use num::Zero;

    /// Test if CFO estimation is going ok
    #[test]
    fn test_cfo_estimation_and_correction() {
        // CFO that we will introduce (in radians per sample)
        let cfo = 0.1;

        let config = ChannelEstConfig::default();

        // Prepare long and short sequences
        let short: Vec<_> = config
            .sts
            .as_ref()
            .unwrap()
            .iter()
            .cycle()
            .take(config.sts.as_ref().unwrap().len() * 10)
            .enumerate()
            .map(|(i, s)| s * Complex::new(0., cfo * i as f32).exp())
            .collect();
        let lts_len = config.lts.as_ref().unwrap().0.len();
        let lts = config.lts.as_ref().unwrap().0.clone();
        // Create the long preamble with CFO added in
        let long: Vec<_> = std::iter::repeat(Complex::new(0., 0.))
            .take(lts_len / 2)
            .chain(
                lts.iter()
                    .cycle()
                    .take(lts_len * 2)
                    .enumerate()
                    .map(|(i, s)| s * Complex::new(0., cfo * (i + lts_len / 2) as f32).exp()),
            )
            .collect();

        let cfo_est = estimate_cfo(&short, &long, &config);
        assert!((cfo_est - cfo).abs() < 1e-45);

        // Correct the CFO and see if that restores to lts
        let corrected_long = correct_cfo(&long, cfo_est);
        assert_eq!(corrected_long.len(), long.len());

        for i in 0..corrected_long.len() {
            // Norms shouldn't change
            assert!((corrected_long[i].norm() - long[i].norm()).abs() < 1e-6);

            // Check the entire complex number
            if i < lts_len / 2 {
                assert_eq!(corrected_long[i], Complex::zero());
            } else if i < 3 * lts_len / 2 {
                assert!((lts[i - lts_len / 2] - corrected_long[i]).norm() < 1e-6);
            } else {
                assert!((lts[i - 3 * lts_len / 2] - corrected_long[i]).norm() < 1e-6);
            }
        }
    }
}
