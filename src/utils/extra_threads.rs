#![cfg(feature = "gui")]
use std::{thread, time::Duration};

use crossbeam_channel::{Receiver, Sender};
use fltk::{app, misc::Progress};
use hashbrown::HashMap;
use log::info;
use wide::f32x4;

use crate::{
    enums::messages::MessageType,
    globals::statics::ONE_MINUTE,
    openhome::rendercontrol::{Renderer, WavData, discover},
};

// run the `ssdp_updater` - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
pub fn run_ssdp_updater(ssdp_tx: &Sender<MessageType>, ssdp_interval_mins: f64) {
    let agent = ureq::agent();
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&agent, &rmap).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.location.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx
                    .send(MessageType::SsdpMessage(Box::new(r.clone())))
                    .unwrap();
                app::awake();
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * ONE_MINUTE) as u64,
        ));
    }
}

/// compute the left and right channel RMS value for every 100 ms period
/// and show the values in the UI
/// sums left and right channel samples, 4 samples at a time
/// this could use SIMD SSE movps/addps/mulps with 4 f32s at a time
/// it does so in GodBolt but not here for some reason
/// so I switched to the wide crate
pub fn run_rms_monitor(
    wd: WavData,
    rms_receiver: &Receiver<Vec<f32>>,
    mut rms_frame_l: Progress,
    mut rms_frame_r: Progress,
) {
    const I16_MAX: f32 = (i16::MAX as f32) + 1.0;
    // compute # of samples needed to get a 10 Hz refresh rate
    let samples_per_update = ((wd.sample_rate * u32::from(wd.channels)) / 10) as usize;
    let mut total_samples = 0usize;
    let mut ch_sum = f32x4::splat(0f32);
    let imax = f32x4::splat(I16_MAX);
    while let Ok(samples) = rms_receiver.recv() {
        total_samples += samples.len();
        let chunks = samples.chunks_exact(4);
        let remainder = chunks.remainder();
        ch_sum = chunks.fold(ch_sum, |acc, x| {
            let f4 = f32x4::from(x); // moved into xmm reg with a single MOVUPS
            let i4 = f4 * imax;
            i4.mul_add(i4, acc)
        });
        if remainder.len() == 2 {
            let rem = f32x4::from([remainder[0], remainder[1], 0.0, 0.0]);
            let i4 = rem * imax;
            ch_sum = i4.mul_add(i4, ch_sum);
        }
        // compute and show current RMS values if enough samples collected
        if total_samples >= samples_per_update {
            let rms = ch_sum.to_array();
            let samples_per_channel = (total_samples / wd.channels as usize) as f32;
            let rms_l = f64::from(((rms[0] + rms[2]) / samples_per_channel).sqrt());
            let rms_r = f64::from(((rms[1] + rms[3]) / samples_per_channel).sqrt());
            total_samples = 0;
            ch_sum = f32x4::splat(0f32);
            rms_frame_l.set_value(rms_l);
            rms_frame_r.set_value(rms_r);
            app::awake();
        }
    }
}
