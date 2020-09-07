use num::Complex;
use rustfft::FFTplanner;
use serde::Deserialize;
use std::default::Default;
use transform_struct::transform_struct;

transform_struct!(
    #[derive(Deserialize)]
    pub struct ChannelEstConfigDes
    #[derive(Clone, Debug)]
    pub struct ChannelEstConfig {
        /// Number of samples to skip in the beginning to let the frontend stabilize
        pub stabilize_samps: u64,
        /// Power (i.e. |x|^2) level for packet detection
        pub power_trig: f32,
        /// We may assume there are at-least these many samples between packets
        pub pkt_spacing: u64,
        > {
            /// The short training sequence. This sequence is repeated 10 times
            pub sts: Option<String>
            => (option_filename_to_cplx_vec -> Option<Vec<Complex<f32>>>),
            /// Filename where the Long Training Sequence (LTS) is stored. This is read and
            /// converted to a vec of complex numbers by `filename_to_cplx_vec`. We store both the
            /// lts and its FFT. If the FFT element has a magnitude < 1% of the maximum, then we
            /// store None. This implies that the sub-carrier isn't used
            pub lts: Option<String>
            => (read_lts -> Option<(Vec<Complex<f32>>, Vec<Option<Complex<f32>>>)>),
        }
    }
);

impl Default for ChannelEstConfig {
    fn default() -> Self {
        ChannelEstConfigDes {
            stabilize_samps: 0,
            power_trig: 0.01,
            pkt_spacing: 20,
            sts: Some("data/short-802.11.txt".to_string()),
            lts: Some("data/lts-802.11.txt".to_string())
        }.into()
    }
}

/// The file storing the LTS sequence is just a list of numbers, each on a separate line
pub fn filename_to_cplx_vec(fname: String) -> Vec<Complex<f32>> {
    let str_data = std::fs::read_to_string(fname).unwrap();
    // Split string into lines and parse floats
    let f32_data: Vec<f32> = str_data
        .split('\n')
        .filter_map(|s| {
            if s.len() == 0 {
                None
            } else {
                Some(s.parse().unwrap())
            }
        })
        .collect();

    // Convert into complex. Even numbers are the real part and odd ones are the imaginary
    assert_eq!(f32_data.len() % 2, 0);
    let mut res = Vec::with_capacity(f32_data.len() / 2);
    for i in 0..f32_data.len() / 2 {
        res.push(Complex::new(f32_data[2 * i], f32_data[2 * i + 1]));
    }

    res
}

fn option_filename_to_cplx_vec(fname: Option<String>) -> Option<Vec<Complex<f32>>> {
    let fname = match fname {
        Some(fname) => fname,
        None => return None,
    };
    Some(filename_to_cplx_vec(fname))
}

pub fn read_lts(fname: Option<String>) -> Option<(Vec<Complex<f32>>, Vec<Option<Complex<f32>>>)> {
    let fname = match fname {
        Some(fname) => fname,
        None => return None,
    };
    let lts = filename_to_cplx_vec(fname);

    // FFT of lts. Do it in f64 for extra precision. This is a one-time calculation, so we can
    // invest CPU here
    let mut planner = FFTplanner::new(true);
    let fft = planner.plan_fft(lts.len());
    let mut lts_fft = vec![Complex::new(0., 0.); lts.len()];
    let mut lts_clone: Vec<Complex<f64>> = lts
        .iter()
        .map(|x| Complex::new(x.re as f64, x.im as f64))
        .collect();
    fft.process(&mut lts_clone, &mut lts_fft);

    // Find the max of lts
    let lts_max = lts_fft
        .iter()
        .fold(0., |max, x| {
            if x.norm_sqr() > max {
                x.norm_sqr()
            } else {
                max
            }
        })
        .sqrt();

    // Make all the elements < 1% of lts_max as None. Convert back to f32 now that FFT is done
    let lts_fft = lts_fft
        .iter()
        .map(|x| {
            if x.norm() < lts_max * 0.01 {
                None
            } else {
                Some(Complex::new(x.re as f32, x.im as f32))
            }
        })
        .collect::<Vec<_>>();

    Some((lts, lts_fft))
}

#[cfg(test)]
mod test {
    use super::{filename_to_cplx_vec, read_lts};
    use num::Complex;

    #[test]
    fn test_filename_to_cplx_vec() {
        let v = filename_to_cplx_vec("data/lts-802.11.txt".to_string());
        assert_eq!(v.len(), 64);

        assert!((v[0] - Complex::new(1.56e-1, 0.)).norm() < 1e-6);
        assert!((v[v.len() - 1] - Complex::new(-5e-3, 1.2e-1)).norm() < 1e-6);
    }

    #[test]
    fn test_read_lts() {
        assert!(read_lts(None).is_none());

        let v = read_lts(Some("data/lts-802.11.txt".to_string())).unwrap();
        assert_eq!(v.0.len(), 64);
        assert_eq!(v.0.len(), v.1.len());

        assert!(v.1[0].is_none());
        assert!(v.1[1].unwrap().re - 1. < 0.01);
        for x in v.1 {
            if let Some(x) = x {
                assert!(x.im < 1e-2);
                assert!(x.re - 1. < 1e-2 || x.im + 1. < 1e-2);
            }
        }
    }
}
