use num::Complex;

/// The the long-train sequence (LTS) to align the start of the packet. Returns the symbol index at
/// which the packet starts. Takes the sequences in the packet and the known LTS. Note: Should only
/// be called on a small portion of data that is known to contain the LTS. Providing more data may
/// case spurious peaks
pub fn lts_align(pkt: &[Complex<f32>], lts: &[Complex<f32>]) -> usize {
    // Compute cross correlation with the known LTS
    let mut corr = Vec::<f32>::with_capacity(pkt.len());
    for i in 0..pkt.len() - lts.len() {
        corr.push(
            lts.iter()
                .enumerate()
                .map(|(k, l)| l.conj() * pkt[i + k])
                .sum::<Complex<f32>>()
                .norm_sqr(),
        );
    }

    // To detect first of the two peaks, find argmax_i corr[i] * corr[i + lts.len()]
    let (mut max, mut max_idx) = (0., 0);
    for i in 0..pkt.len() - 2 * lts.len() {
        let val = corr[i] * corr[i + lts.len()];
        if val > max {
            max = val;
            max_idx = i;
        }
    }

    // Subtract config.lts.len() to account for the fact that a guard interval is present
    max_idx - lts.len() / 2
}

#[cfg(test)]
mod test {
    use super::lts_align;
    use crate::config::{filename_to_cplx_vec, ChannelEstConfig};
    use num::{Complex, One, Zero};

    #[test]
    fn lts_align_example_pkt() {
        let lts = filename_to_cplx_vec("data/lts-802.11.txt".to_string());
        let pkt = filename_to_cplx_vec("data/example_pkt.txt".to_string());

        assert_eq!(lts_align(&pkt[0..1400], &lts), 171);
    }

    #[test]
    fn lts_align_synth_pkt() {
        let config = ChannelEstConfig::default();
        let mut pkt = Vec::new();
        // Some initial junk
        pkt.extend(
            [
                Complex::new(0.1, 0.1),
                Complex::new(-0.1, -0.1),
                Complex::zero(),
            ]
            .iter()
            .cycle()
            .take(100),
        );

        let real_start = pkt.len();
        // Add the long preamble
        let lts = &config.lts.as_ref().unwrap().0;
        assert_eq!(lts.len() % 2, 0);
        pkt.extend(std::iter::repeat(Complex::zero()).take(lts.len() / 2));
        pkt.extend(lts);

        // More junk
        pkt.extend(
            [
                Complex::new(-0.1, 0.05),
                Complex::new(0.11, -0.04),
                Complex::one() * 0.1,
                -Complex::one() * 0.1,
            ]
            .iter()
            .cycle()
            .take(100),
        );

        assert_eq!(lts_align(&pkt, &lts), real_start);
    }
}
