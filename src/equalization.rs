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
    assert_eq!(long.len(), 5 * lts_len / 2);

    // Compute the average LTS before taking fft
    let mut lts: Vec<_> = (0..lts_len)
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
        .filter_map(|(s, e)| e.map(|e| s * e / samps.len() as f32))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use num::One;
    use rand::Rng;

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

    /// Test equalization estimation and correct
    #[test]
    fn test_equalization() {
        let config = ChannelEstConfig::default();
        let lts = &config.lts.as_ref().unwrap().0;
        assert_eq!(lts.len() % 4, 0);

        // Create a random symbol
        let mut rng = rand::thread_rng();
        let mut symbol = Vec::new();
        let mut symbol_data = Vec::new();
        for x in &config.lts.as_ref().unwrap().1 {
            if x.is_some() {
                let sym = match rng.gen() {
                    true => Complex::new(-1., 0.),
                    false => Complex::new(1., 0.),
                };
                symbol.push(sym);
                symbol_data.push(sym);
            } else {
                symbol.push(Complex::zero());
            }
        }
        // Take FFT of the symbol
        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft(lts.len());
        let mut symbol_fft = vec![Complex::zero(); lts.len()];
        fft.process(&mut symbol.clone(), &mut symbol_fft);

        // Construct a 'packet' with a long preamble and one data symbol
        let mut pkt = Vec::<Complex<f32>>::new();
        // Long preamble
        pkt.extend(std::iter::repeat(Complex::zero()).take(lts.len() / 2));
        pkt.extend(lts);
        pkt.extend(lts);
        // Cyclic prefix
        pkt.extend(&symbol_fft[3 * lts.len() / 4..]);
        pkt.extend(&symbol_fft.clone());

        // Add multipath effect to this packet
        for i in lts.len() / 8..lts.len() {
            pkt[i] = pkt[i] + Complex::new(0.1, 0.2) * pkt[i - lts.len() / 8];
        }

        // Now estimate equalization
        let equalization = estimate_subcarrier_equalization(&pkt[..5 * lts.len() / 2], &config);

        // And eliminate the equalization
        let corr_symbol = equalize_symbol(&pkt[pkt.len() - fft.len()..], &equalization);

        // See that the symbol has been decoded correctly
        for (x, y) in corr_symbol.iter().zip(symbol_data) {
            assert!((x - y).norm() < 0.5);
            assert_eq!(x.re > 0., y.re > 0.);
        }
    }
}
