//! The radar estimates the channel repeatedly and reports it. To coexist with other radios on the
//! same frequency, we can configure a duty cycle that makes the estimator quite for a period of
//! time so others can transmit. It uses the following structure:
//!
//! [<short preamble> <long preamble>] x repeat n times

use channel_est::cfo::{correct_cfo, estimate_cfo};
use channel_est::config::{ChannelEstConfig, ChannelEstConfigDes};
use channel_est::equalization::estimate_subcarrier_equalization;
use channel_est::lts_align::lts_align;
use channel_est::pkt_trigger::PktTrigger;
use failure::Error;
use num::{Complex, Zero};
use rand::SeedableRng;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use usrp::{create_simulator, RadioRx, RadioSimulatorConfig, RadioTx};

#[derive(Clone, Debug)]
pub struct MonitorConfig {
    /// General OFDM config
    ofdm: ChannelEstConfig,
    /// Number of times the preambles are repeated,
    num_repeats: u64,
    /// Duty cycle, so we can give time for others to transmit
    duty_cycle: f32,
}

/// Loops forever as a transmitter until signalled to close by `close`
pub fn run_tx<T: RadioTx>(
    tx: &mut T,
    config: &MonitorConfig,
    close: Arc<AtomicBool>,
) -> Result<(), Error> {
    let sts = config.ofdm.sts.as_ref().unwrap();
    let lts = config.ofdm.lts.as_ref().unwrap();
    assert_eq!(lts.0.len() % 2, 0);

    // Construct the preamble that will be repeatedly transmitted
    let mut preamble = Vec::new();
    preamble.extend(sts.iter().cycle().take(10 * sts.len()));
    preamble.extend(std::iter::repeat(Complex::zero()).take(lts.0.len() / 2));
    preamble.extend(lts.0.iter().cycle().take(2 * lts.0.len()));

    assert!(0. <= config.duty_cycle && config.duty_cycle <= 1.);
    // Silence period to allow any other radios in the vicinity time to transmit
    let silence_len = (preamble.len() as f32 * (1. / config.duty_cycle - 1.)).round() as usize;
    let silence = vec![Complex::zero(); silence_len];
    assert!(silence_len > lts.0.len() / 2);

    // Construct the packet we will transmit repeatedly
    'outer: while !close.load(Ordering::Relaxed) {
        for _ in 0..config.num_repeats {
            let res = tx.send(&preamble);
            if res.is_err() {
                println!("{:?}", res);
                break 'outer;
            }
        }

        let res = tx.send(&silence);
        if res.is_err() {
            break;
        }
    }

    println!("Tx closed");
    Ok(())
}

/// Loops forever as a receiver until signalled to close by `close`
pub fn run_rx<R: RadioRx, F: FnMut(&[Option<Complex<f32>>])>(
    rx: &mut R,
    config: &MonitorConfig,
    mut callback: F,
    close: Arc<AtomicBool>,
) -> Result<(), Error> {
    let mut pkt_trigger = PktTrigger::new(&config.ofdm);
    // Known preambles; the lts and sts
    let sts = config.ofdm.sts.as_ref().unwrap();
    let lts = config.ofdm.lts.as_ref().unwrap();

    while !close.load(Ordering::Relaxed) {
        let buf = if let Ok(buf) = rx.recv(512) {
            buf
        } else {
            break;
        };
        for samp in buf.0 {
            let pkt = pkt_trigger.push_samp(*samp);

            if pkt.is_none() {
                continue;
            }
            // A packet has been detected, let's process it.
            let pkt = pkt.unwrap();
            println!("Packet detected");

            // The preamble (short + long) is this many samples long. We use an additional
            // lts.0.len() / 2 samples, so we have some margin for error
            let preamble_len = 10 * sts.len() + 5 * lts.0.len() / 2 + lts.0.len() / 2;
            // First align the first LTS. The long preamble will be within a margin of the
            // beginning of the packet. We only pass that to `lts_align` so it doesn't get confused
            // by what comes after.
            let first_lts_margin = config.ofdm.pkt_spacing as usize + preamble_len;
            let mut cur_lts_start = lts_align(&pkt[..first_lts_margin], &lts.0);

            // Now process each repetition one-by-one
            for i in 0..config.num_repeats {
                // Figure out where the preambles are
                let short = &pkt[cur_lts_start - 10 * sts.len()..cur_lts_start];
                let long = &pkt[cur_lts_start..cur_lts_start + 5 * lts.0.len() / 2];

                // Calculate the CFO and correct it in the long preamble
                let cfo = estimate_cfo(short, long, &config.ofdm);
                let long = correct_cfo(long, cfo);

                // Calculate the equalization
                let equalization = estimate_subcarrier_equalization(&long, &config.ofdm);
                callback(&equalization);

                // Estimate the start of the next long preamble. Sample frequency offset aside, it
                // should be pretty close to `cur_lts_start + preamble_len`. No need to do this if
                // this was the last repeat
                if i < config.num_repeats - 1 {
                    // Leave this much margin for samples to have drifted
                    let margin = 5;
                    // If margin is so large it includes the previous LTS, it can cause trouble
                    assert!(margin < lts.0.len() / 2);
                    let expected_start = cur_lts_start + preamble_len;
                    cur_lts_start = expected_start - margin
                        + lts_align(
                            &pkt[expected_start - margin..expected_start + preamble_len],
                            &lts.0,
                        );
                    if (cur_lts_start as i64 - expected_start as i64).abs() > margin as i64 {
                        eprintln!("It seems that the LTS drifted more than the expected margin. Skipping the rest of the packet: {} {} {} {}", i, cur_lts_start, expected_start, pkt.len());
                        break;
                    }
                }
            }
        }
    }
    println!("Rx closed");

    Ok(())
}

fn main() {
    // Register signal handler to close USRP on Ctrl-C
    let close = Arc::new(AtomicBool::new(false));
    let close_handler = close.clone();
    ctrlc::set_handler(move || {
        close_handler.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    // Config for the radio
    let radio_config = RadioSimulatorConfig {
        max_start_time_offset: 1000,
        samp_rate: 20_000_000,
        start_freq: 5.5e9,
        max_cfo: 0.1,
        cfo_drift: 0.000,
        phase_noise: 0.000,
        noise: 0.000,
        multipath: vec![(2e-6, Complex::new(0.01, 0.01))],
    };

    // Create Tx and Rx
    let (mut tx, mut rx) = create_simulator(&radio_config, rand::rngs::StdRng::seed_from_u64(0));

    // Start the transmitter and receiver
    let mut monitor_config = MonitorConfig {
        ofdm: ChannelEstConfigDes {
            stabilize_samps: 0,
            power_trig: 0.1,
            pkt_spacing: 0, // will set later
            sts: Some("data/short-802.11.txt".to_string()),
            lts: Some("data/lts-802.11.txt".to_string()),
        }
        .into(),
        num_repeats: 100,
        duty_cycle: 0.5,
    };
    // The minimum gap between packets has to be at least this large, so we don't mistake the LTS
    // guard interval for the end of the packet
    monitor_config.ofdm.pkt_spacing = monitor_config.ofdm.lts.as_ref().unwrap().0.len() as u64;

    let close_rx = close.clone();
    let monitor_config_rx = monitor_config.clone();
    let callback = |x: &[Option<Complex<f32>>]| {
        //println!("{:?}", x);
    };
    let rx_handle =
        std::thread::spawn(move || run_rx(&mut rx, &monitor_config_rx, callback, close_rx));

    let tx_handle = std::thread::spawn(move || run_tx(&mut tx, &monitor_config, close));

    rx_handle.join().unwrap().unwrap();
    tx_handle.join().unwrap().unwrap();
}
