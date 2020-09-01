//! Module for channel estimation for multiple frequencies

mod config;
mod pkt_trigger;

pub use pkt_trigger::PktTrigger;

use num::Complex;

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Generate a Zadoff-Chu sequence with given N and u
fn zadoff_chu(n: u64, u: u64) -> Vec<Complex<f32>> {
    assert_eq!(gcd(n, u), 1);
    use std::f32::consts::PI;
    (0..n)
        .map(|i| Complex::<_>::new(0., PI * (u * i * (i + 1)) as f32 / n as f32).exp())
        .collect()
}
