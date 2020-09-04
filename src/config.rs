use num::Complex;
use serde::Deserialize;
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
        /// Length of one of the short training sequence pieces (which is repeated 10 times)
        pub short_piece_len: u64,
        > {
            /// Filename where the Long Training Sequence (LTS) is stored. This is read and
            /// converted to a vec of complex numbers by `filename_to_cplx_vec`
            pub lts: Option<String> => (filename_to_cplx_vec -> Option<Vec<Complex<f32>>>),
        }
    }
);

/// The file storing the LTS sequence is just a list of numbers, each on a separate line
pub fn filename_to_cplx_vec(fname: Option<String>) -> Option<Vec<Complex<f32>>> {
    let fname = match fname {
        Some(fname) => fname,
        None => return None,
    };

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
    Some(res)
}

#[cfg(test)]
mod test {
    use super::filename_to_cplx_vec;
    use num::Complex;

    #[test]
    fn test_filename_to_cplx_vec() {
        assert!(filename_to_cplx_vec(None).is_none());
        let v = filename_to_cplx_vec(Some("data/lts-802.11.txt".to_string())).unwrap();
        assert_eq!(v.len(), 64);
        assert!((v[0] - Complex::new(1.56e-1, 0.)).norm() < 1e-6);
        assert!((v[v.len()-1] - Complex::new(-5e-3, 1.2e-1)).norm() < 1e-6);
    }
}
