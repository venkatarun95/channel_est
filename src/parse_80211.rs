use crate::cfo::{correct_cfo, estimate_cfo};
use crate::config::ChannelEstConfig;
use crate::equalization::{equalize_symbol, estimate_subcarrier_equalization};
use crate::lts_align::lts_align;
use num::Complex;

/// Given a buffer possibly containing a packet (e.g. as detected by `pkt_trigger::PktTrigger`),
/// returns a parsed version of that packet if it is indeed a packet. Assumes the packet starts
/// within the first ChannelEstConfig::pkt_spacing samples
pub fn parse_80211_pkt(samps: &[Complex<f32>], config: &ChannelEstConfig) -> Vec<Complex<f32>> {
    // Lengths of the various piecs
    // Two repeats of the LTS + guard interval
    let lts_len = config.lts.as_ref().unwrap().0.len();
    let short_len = 10 * config.sts.as_ref().unwrap().len() as usize;
    assert!(samps.len() > 3 * lts_len / 2 + short_len);

    // The LTS symbol should be contained within this range
    let lts_bound = config.pkt_spacing as usize + short_len + 5 * lts_len / 2;
    // Sync the packet using LTS so we know where everything is
    let lts_start = lts_align(&samps[..lts_bound], &config.lts.as_ref().unwrap().0);

    let short = &samps[lts_start - short_len..lts_start];
    let long = &samps[lts_start..lts_start + 5 * lts_len / 2];

    let cfo = estimate_cfo(short, long, config);

    let long_corr = correct_cfo(long, cfo);
    let equalization = estimate_subcarrier_equalization(&long_corr, config);

    // Calculate the rms for the long preamble. If any symbol has <10% of this strength, we assume
    // the packet has ended there. Packet length is also available in the SIGNAL symbol right after
    // the long preamble, but we haven't implemented decoding yet
    let pkt_rms = long.iter().map(|x| x.norm_sqr()).sum::<f32>().sqrt();

    // Go through the symbols one by one and correct CFO and qualize
    assert_eq!(lts_len % 4, 0);
    let mut i = lts_start + 5 * lts_len / 2;
    let mut res = Vec::new();
    while i < samps.len() - 5 * lts_len / 4 {
        let symbol = &samps[i + lts_len / 4..i + 5 * lts_len / 4];
        let rms = symbol.iter().map(|x| x.norm_sqr()).sum::<f32>().sqrt();
        if rms < 0.1 * pkt_rms {
            break;
        }

        let symbol = correct_cfo(symbol, cfo);
        let mut symbol = equalize_symbol(&symbol, &equalization);
        res.append(&mut symbol);
        i += 5 * lts_len / 4;
    }
    res
}

#[cfg(test)]
mod test {
    use super::*;
    use num::Zero;
    use rand::Rng;
    use rustfft::FFTplanner;

    #[test]
    fn test_parse_80211_pkt() {
        let config = ChannelEstConfig::default();
        let lts = &config.lts.as_ref().unwrap().0;
        assert_eq!(lts.len() % 4, 0);

        // Create random symbols
        let mut symbols = Vec::new();
        let mut symbols_data = Vec::new();
        let mut rng = rand::thread_rng();
        for _ in 0..2 {
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

            // Add cyclic prefix to the symbol
            symbols.extend(&symbol_fft[3 * lts.len() / 4..]);
            symbols.append(&mut symbol_fft);
            symbols_data.append(&mut symbol_data);
        }

        // Construct a packet with preambles and data
        let mut pkt = Vec::<Complex<f32>>::new();

        // Add some silence period
        pkt.extend(std::iter::repeat(Complex::zero()).take(config.pkt_spacing as usize - 1));

        // Short preamble
        let sts = config.sts.as_ref().unwrap();
        pkt.extend(sts.iter().cycle().take(10 * sts.len()));

        // Long preamble
        pkt.extend(std::iter::repeat(Complex::zero()).take(lts.len() / 2));
        pkt.extend(lts);
        pkt.extend(lts);

        // The symbols
        pkt.extend(&symbols.clone());

        // Add some silence period
        pkt.extend(std::iter::repeat(Complex::zero()).take(lts.len() * 2));

        // Add multipath effect to this packet
        for i in lts.len() / 8..lts.len() {
            pkt[i] = pkt[i] + Complex::new(0.1, 0.2) * pkt[i - lts.len() / 8];
        }

        let parsed_symbols = parse_80211_pkt(&pkt, &config);

        // See that the symbol has been decoded correctly
        assert_eq!(parsed_symbols.len(), symbols_data.len());
        for (x, y) in parsed_symbols.iter().zip(symbols_data) {
            assert!((x - y).norm() < 0.5);
            assert_eq!(x.re > 0., y.re > 0.);
        }
    }
}
