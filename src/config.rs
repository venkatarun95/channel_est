#[derive(Clone, Debug)]
pub struct ChannelEstConfig {
    /// Number of samples to skip in the beginning to let the frontend stabilize
    pub stabilize_samps: u64,
    /// Power (i.e. |x|^2) level for packet detection
    pub power_trig: f32,
    /// We may assume there are at-least these many samples between packets
    pub pkt_spacing: u64,
}
