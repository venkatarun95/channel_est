use crate::config::ChannelEstConfig;
use num::Complex;
use std::collections::VecDeque;

enum PktTriggerState {
    /// Skip the first few samples (counts the number of samples so far)
    Skip(u64),
    /// No packets so far
    Idle,
    /// Yup, we are sampling a packet now. Number indicates the number of samples whose `norm_sqr`
    /// has been less than `config.power_trig`
    Packet(u64),
}

/// Looks for a sudden increase in received signal strength and returns a `Vec<Complex<f32>>` that
/// should contain the packet. It is conservative and may return some extra samples on either side.
/// Other techniques should be used to detect the start of the packet.
pub struct PktTrigger {
    config: ChannelEstConfig,
    /// Short history of samples. If state is `Packet`, then the entire (suspected) packet is
    /// contained in `hist`
    hist: VecDeque<Complex<f32>>,
    state: PktTriggerState,
}

impl PktTrigger {
    pub fn new(config: &ChannelEstConfig) -> Self {
        Self {
            config: config.clone(),
            hist: VecDeque::new(),
            state: PktTriggerState::Skip(0),
        }
    }

    /// Takes in samples and returns a packets if detected
    pub fn push_samp(&mut self, samp: Complex<f32>) -> Option<Vec<Complex<f32>>> {
        match self.state {
            PktTriggerState::Skip(skip) => {
                if skip >= self.config.stabilize_samps {
                    self.state = PktTriggerState::Idle;
                } else {
                    self.state = PktTriggerState::Skip(skip + 1);
                }
                None
            }
            PktTriggerState::Idle => {
                self.hist.push_back(samp);
                if samp.norm_sqr() > self.config.power_trig {
                    self.state = PktTriggerState::Packet(0);
                } else {
                    if self.hist.len() as u64 > self.config.pkt_spacing {
                        self.hist.pop_front();
                    }
                }
                None
            }
            PktTriggerState::Packet(n) => {
                self.hist.push_back(samp);
                // Signal strength should be < power_trig for at-least pkt_spacing samples
                if samp.norm() >= self.config.power_trig {
                    self.state = PktTriggerState::Packet(0);
                    None
                } else {
                    if n >= self.config.pkt_spacing {
                        // This is our packet
                        let res = self.hist.iter().map(|x| *x).collect();
                        // Clear hist and while keeping last self.config.pkt_spacing elements in it
                        while self.hist.len() as u64 > self.config.pkt_spacing {
                            self.hist.pop_front();
                        }
                        self.state = PktTriggerState::Idle;
                        Some(res)
                    } else {
                        self.state = PktTriggerState::Packet(n + 1);
                        None
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PktTrigger;
    use crate::config::ChannelEstConfig;
    use num::Complex;

    #[test]
    fn test_basic_pkt_trigger() {
        let mut config = ChannelEstConfig::default();
        config.stabilize_samps = 100;
        let mut trigger = PktTrigger::new(&config);

        // Initialization period can be weird. Samples should be skipped
        for _ in 0..50 {
            assert!(trigger.push_samp(Complex::new(1., 1.)).is_none());
        }
        for _ in 0..150 {
            assert!(trigger.push_samp(Complex::new(0.001, 0.)).is_none());
        }

        // Push some packet
        for _ in 0..5 {
            // Push a few samples we know and can hence test for
            assert!(trigger.push_samp(Complex::new(1.1, 0.9)).is_none());
            assert!(trigger.push_samp(Complex::new(0.9, 1.1)).is_none());
            for i in 0..500 {
                // Some weird values
                assert!(trigger
                    .push_samp(Complex::new(i as f32, 2. * i as f32).exp())
                    .is_none());
            }
            // Some empty values, so it can detect end of packet
            for _ in 0..config.pkt_spacing {
                assert!(trigger.push_samp(Complex::new(0., 0.)).is_none());
            }
            let pkt = trigger.push_samp(Complex::new(0., 0.));
            assert!(pkt.is_some());
            assert!(pkt.unwrap()[config.pkt_spacing as usize] == Complex::new(1.1, 0.9));
        }
    }
}
