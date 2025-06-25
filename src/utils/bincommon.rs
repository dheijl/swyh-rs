//! Tools common to both the swyh-rs GUI and CLI.

use cpal::{
    Sample, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use log::warn;

use super::audiodevices::Device;

/// Inject silence into the audio stream to solve problems with Sonos when pausing audio.
/// contributed by @genekellyjr, see issue #71
///
/// Streams are asynchronous, so the silence stream is just returned to keep the object alive.
pub fn run_silence_injector(device: &Device) -> Option<Stream> {
    // CPAL 0.15 switched to dasp_sample:
    // see https://github.com/RustAudio/cpal/commit/85d773d59f1725b25002c6f04aa2eb9b43a75b76#diff-babb62f9985b4798a655658e440a565984ce15b25e63a82fc4b3cc0b54fd2a02
    fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
        for sample in &mut *data {
            *sample = Sample::EQUILIBRIUM;
        }
    }

    let config = device.default_config();
    let sample_format = config.sample_format();
    let err_fn = |err| warn!("an error occurred on the output audio stream: {err}");
    let config: StreamConfig = config.clone().into();
    let device = device.as_ref();
    let try_stream = match sample_format {
        SampleFormat::F32 => {
            device.build_output_stream(&config, write_silence::<f32>, err_fn, None)
        }
        SampleFormat::I16 => {
            device.build_output_stream(&config, write_silence::<i16>, err_fn, None)
        }
        SampleFormat::U16 => {
            device.build_output_stream(&config, write_silence::<u16>, err_fn, None)
        }
        format => panic!("Unsupported sample format: {format:?}"),
    };
    match try_stream {
        Ok(stream) => {
            if stream.play().is_ok() {
                Some(stream)
            } else {
                None
            }
        }
        _ => None,
    }
}
