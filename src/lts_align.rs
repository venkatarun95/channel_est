use num::Complex;

/// The the long-train sequence (LTS) to align the start of the packet. Returns the symbol index at
/// which the packet starts. Takes the sequences in the packet and the known LTS
pub fn lts_align(pkt: &[Complex<f32>], lts: &[Complex<f32>]) -> usize {
    // Compute cross correlation with the known LTS
    let mut corr = Vec::with_capacity(pkt.len());
    for i in 0..pkt.len() {
        corr.push(
            lts.iter()
            .take(pkt.len() - i)
            .enumerate()
            .map(|(k, l)| l.conj() * pkt[i + k])
            .sum::<Complex<f32>>()
            .norm_sqr(),
        );
    }

    // To detect first of the two peaks, find argmax_i corr[i] * corr[i + lts.len()]
    let (mut max, mut max_idx) = (0., 0);
    for i in 0..pkt.len() - lts.len() {
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
    use crate::config::filename_to_cplx_vec;
    use super::lts_align;

    #[test]
    fn lts_align_example_pkt() {
        let lts = filename_to_cplx_vec("data/lts.txt".to_string());
        let pkt = filename_to_cplx_vec("data/example_pkt.txt".to_string());

        assert_eq!(lts_align(&pkt[0..1400], &lts), 171);
    }
}
