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

mod config;
mod lts_align;
mod parse_packet;
mod pkt_trigger;

pub use parse_packet::ParsePacket;
pub use pkt_trigger::PktTrigger;
pub use lts_align::lts_align;
