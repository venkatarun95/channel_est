use crate::config::ChannelEstConfig;
use num::{Complex, Zero};
use rustfft::FFTplanner;

/// Estimate equalization for each OFDM subcarrier that is in-use. If the subcarrier in the lts is
/// < 0.1 times the max subcarrier, we'll assume that subcarrier is absent return `None` there.
pub fn estimate_subcarrier_equalization(
    long: &[Complex<f32>],
    config: &ChannelEstConfig,
) -> Vec<Option<Complex<f32>>> {
    let lts_len = config.lts.as_ref().unwrap().0.len();
    assert_eq!(long.len(), 3 * lts_len / 2);

    // Copy long and CFO correct it

    // Compute the average LTS before taking fft
    let mut lts: Vec<_> = (0..lts_len / 2)
        .map(|i| (long[lts_len / 2 + i] + long[3 * lts_len / 2 + i]) / 2.)
        .collect();
    assert_eq!(lts.len(), lts_len);

    // FFT of the long preamble
    let mut long_fft = vec![Complex::zero(); lts_len];
    let mut planner = FFTplanner::new(true);
    let fft = planner.plan_fft(lts_len);
    fft.process(&mut lts, &mut long_fft);

    long_fft
        .iter()
        .zip(&config.lts.as_ref().unwrap().1)
        .map(|(x, l)| match l {
            Some(l) => Some(l / x),
            None => None,
        })
        .collect()
}

/// Take an IFFT to get the symbol and equalize the result using the given equalization (e.g. from
/// `estimate_subcarrier_equalization`). Returns a Vec of symbols (as many as there are `Some`
/// values in `equalization`)
pub fn equalize_symbol(
    samps: &[Complex<f32>],
    equalization: &[Option<Complex<f32>>],
) -> Vec<Complex<f32>> {
    assert_eq!(samps.len(), equalization.len());

    // Compute inverse FFT of samps
    let mut planner = FFTplanner::new(true);
    let fft = planner.plan_fft(samps.len());
    let mut ifft = vec![Complex::zero(); samps.len()];
    fft.process(&mut samps.to_vec(), &mut ifft);

    // Equalize and compute result
    ifft.iter()
        .zip(equalization)
        .filter_map(|(s, e)| e.map(|e| s * e))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use num::One;

    #[test]
    /// Little test to understand the conventions in rustfft
    fn understand_rustfft() {
        use std::f32::consts::PI;
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
}
