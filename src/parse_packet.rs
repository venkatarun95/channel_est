use crate::config::ChannelEstConfig;
use crate::lts_align::lts_align;
use num::{Complex, Zero};
use rustfft::FFTplanner;
use std::f32::consts::PI;

/// CFO correct using the short and long preambles. Returns the per-sample phase shift due to CFO
///( hence correction should be in the opposite direction)
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

/// Estimate equalization for each OFDM subcarrier that is in-use. If the subcarrier in the lts is
/// < 0.1 times the max subcarrier, we'll assume that subcarrier is absent return `None` there.
pub fn estimate_subcarrier_equalization(
    long: &[Complex<f32>],
    config: &ChannelEstConfig,
) -> Vec<Option<Complex<f32>>> {
    let lts_len = config.lts.as_ref().unwrap().0.len();
    let mut planner = FFTplanner::new(true);
    assert_eq!(long.len(), 3 * lts_len / 2);

    let fft = planner.plan_fft(2 * lts_len);

    // FFT of the long preamble
    let mut long_fft = vec![Complex::zero(); lts_len];
    fft.process(
        &mut long[long.len() - lts_len / 2..].to_vec(),
        &mut long_fft,
    );

    Vec::new()
}

/// Super-struct that parses packets
pub struct ParsePacket<'p, 'c> {
    /// The short preamble
    short: &'p [Complex<f32>],
    /// The long preamble
    long: &'p [Complex<f32>],
    /// Per sample CFO correction from the short and long preambles
    cfo_corr: Complex<f32>,
    config: &'c ChannelEstConfig,
}

impl<'p, 'c> ParsePacket<'p, 'c> {
    pub fn new(pkt: &'p [Complex<f32>], config: &'c ChannelEstConfig) -> Self {
        // Lengths of the various piecs
        // Two repeats of the LTS + guard interval
        let lts_len = config.lts.as_ref().unwrap().0.len();
        let short_len = 10 * config.sts.as_ref().unwrap().len() as usize;

        // Sync the packet using LTS so we know where everything is
        let lts_start = lts_align(pkt, &config.lts.as_ref().unwrap().0);

        let short = &pkt[lts_start - short_len..lts_start];
        let long = &pkt[lts_start..5 * lts_len / 2];

        let cfo_corr = Complex::new(1., -estimate_cfo(short, long, config)).exp();

        ParsePacket {
            short,
            long,
            cfo_corr,
            config,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num::One;

    #[test]
    /// Little test to understand the conventions in rustfft
    fn understand_rustfft() {
        let mut data: Vec<Complex<f32>> = (0..16u64)
            .map(|i| Complex::new(1f32, -2. * PI * 2. * i as f32 / 16.).exp().re * Complex::one())
            .collect();
        let mut out = vec![Complex::new(0., 0.); 16];

        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft(16);
        fft.process(&mut data, &mut out);

        let _norm = out.iter().map(|x| x.norm()).collect::<Vec<_>>();
        //println!("{:?}", _norm);
    }

    /// Test if CFO estimation is going ok
    #[test]
    fn test_estimate_cfo() {
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
            .map(|(i, s)| s * Complex::new(1., cfo * i as f32).exp())
            .collect();
        let lts_len = config.lts.as_ref().unwrap().0.len();
        let long: Vec<_> = std::iter::repeat(Complex::new(0., 0.))
            .take(lts_len / 2)
            .chain(
                config
                    .lts
                    .as_ref()
                    .unwrap()
                    .0
                    .iter()
                    .cycle()
                    .take(lts_len * 2)
                    .enumerate()
                    .map(|(i, s)| s * Complex::new(1., cfo * i as f32).exp()),
            )
            .collect();

        assert!((estimate_cfo(&short, &long, &config) - cfo).abs() < 1e-45);
    }
}
