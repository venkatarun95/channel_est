//! Module for channel estimation for multiple frequencies. We assume the following packet
//! structure, which mirrors the 802.11 standard:
//! <Short Preamble> <Long Training Sequence> [<Data symbols> ...]
//!
//! Short Preamble:
//!  - 10 repeats of a short training sequence
//!
//! Long Preamble:
//!  - <Guard Interval> 2 * <Long Training Sequence>
//!    The guard interval is 1/2 the size of the LTS
//!    In 802.11, the LTS is 64 samples long. The symbols are in `data/lts.txt`
//!
//! Data Symbol
//!  - <Cyclic Prefix> <Symbol>
//!    The cyclic prefix is 1/4 the size of the symbol. In 802.11, the symbol is 64 samples long

mod cfo;
mod config;
mod equalization;
mod lts_align;
mod pkt_trigger;
mod parse_80211;

pub use cfo::{correct_cfo, estimate_cfo};
pub use equalization::{estimate_subcarrier_equalization, equalize_symbol};
pub use lts_align::lts_align;
pub use pkt_trigger::PktTrigger;
pub use parse_80211::parse_80211_pkt;
