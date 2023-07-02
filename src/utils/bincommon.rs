//! Tools common to both the swyh-rs GUI and CLI.

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Sample, SampleFormat, Stream,
};

use super::audiodevices::Device;

/// Inject silence into the audio stream to solve problems with Sonos when pausing audio.
/// contributed by @genekellyjr, see issue #71
///
/// Streams are asynchronous, so the silence stream is just returned to keep the object alive.
pub fn run_silence_injector(device: &Device) -> Stream {
    let config = device
        .default_config_any()
        .expect("Error while querying stream configs for the silence injector");
    let sample_format = config.sample_format();
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {err}");
    let config = config.into();

    // CPAL 0.15 switched to dasp_sample:
    // see https://github.com/RustAudio/cpal/commit/85d773d59f1725b25002c6f04aa2eb9b43a75b76#diff-babb62f9985b4798a655658e440a565984ce15b25e63a82fc4b3cc0b54fd2a02
    fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
        for sample in data.iter_mut() {
            *sample = Sample::EQUILIBRIUM;
        }
    }

    let device = device.as_ref();
    let stream = match sample_format {
        SampleFormat::F32 => device
            .build_output_stream(&config, write_silence::<f32>, err_fn, None)
            .unwrap(),
        SampleFormat::I16 => device
            .build_output_stream(&config, write_silence::<i16>, err_fn, None)
            .unwrap(),
        SampleFormat::U16 => device
            .build_output_stream(&config, write_silence::<u16>, err_fn, None)
            .unwrap(),
        format => panic!("Unsupported sample format: {format:?}"),
    };
    stream
        .play()
        .expect("Unable to inject silence into the output stream");
    stream
}
