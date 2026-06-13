//! Audio subsystem: device enumeration, capture, sample conversion, and HTTP streaming.

pub mod audiodevices;
pub(crate) mod flacstream;
pub mod rwstream;
pub mod samples_conv;
