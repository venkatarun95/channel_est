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
        > {
            /// Filename where the Long Training Sequence (LTS) is stored. This is read and
            /// converted to a vec of complex numbers by `filename_to_cplx_vec`
            pub lts: String => (filename_to_cplx_vec -> Vec<Complex<f32>>),
        }
    }
);

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
