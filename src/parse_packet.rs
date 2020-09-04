use crate::config::ChannelEstConfig;
use crate::lts_align::lts_align;
use num::Complex;

/// CFO correct using the short and long preambles. Returns the per-sample phase shift due to CFO
///( hence correction should be in the opposite direction)
pub fn estimate_cfo(
    short: &[Complex<f32>],
    long: &[Complex<f32>],
    config: &ChannelEstConfig,
) -> f32 {
    // Coarse estimation using the short preamble
    assert_eq!(short.len(), 10 * config.short_piece_len as usize);
    let coarse = (0..9 * config.short_piece_len as usize)
        .map(|i| short[i].conj() * short[i + config.short_piece_len as usize])
        .sum::<Complex<_>>()
        .arg()
        / config.short_piece_len as f32;

    // Correct the long preamble using the coarse estimate and estimate the residual CFO
    let lts_len = config.lts.as_ref().unwrap().len();
    assert_eq!(long.len(), 3 * lts_len / 2);
    assert_eq!(lts_len % 2, 0);
    // CFO correction for config.lts.len() samples
    let coarse_lts_corr = Complex::new(1., -coarse * lts_len as f32).exp();
    let fine = (lts_len / 2..3 * lts_len / 2)
        .map(|i| long[i].conj() * long[i + lts_len] * coarse_lts_corr)
        .sum::<Complex<_>>()
        .arg()
        / lts_len as f32;

    coarse + fine
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
        let lts_len = config.lts.as_ref().unwrap().len();
        let short_len = 10 * config.short_piece_len as usize;

        // Sync the packet using LTS so we know where everything is
        let lts_start = lts_align(pkt, &config.lts.as_ref().unwrap());

        let short = &pkt[lts_start - short_len..lts_start];
        let long = &pkt[lts_start..3 * lts_len / 2];

        let cfo_corr = Complex::new(1., -estimate_cfo(short, long, config)).exp();

        ParsePacket {
            short,
            long,
            cfo_corr,
            config,
        }
    }
}
