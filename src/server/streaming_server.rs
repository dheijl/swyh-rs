use crate::{
    enums::streaming::{StreamingFormat, StreamingFormat::*, StreamingState},
    globals::statics::{CLIENTS, CONFIG},
    openhome::rendercontrol::WavData,
    utils::{rwstream::ChannelStream, ui_logger::ui_log},
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fltk::app;
use log::debug;
use std::{net::IpAddr, sync::Arc};
use tiny_http::{Header, Method, Response, Server};

/// streaming state feedback for a client
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamerFeedBack {
    pub remote_ip: String,
    pub streaming_state: StreamingState,
}

/// run_server - run a tiny-http webserver to serve streaming requests from renderers
///
/// all music is sent in audio/l16 PCM format (i16) with the sample rate of the source
/// the samples are read from a crossbeam channel fed by the wave_reader
/// a ChannelStream is created for this purpose, and inserted in the array of active
/// "clients" for the wave_reader
pub fn run_server(
    local_addr: &IpAddr,
    server_port: u16,
    wd: WavData,
    feedback_tx: Sender<StreamerFeedBack>,
) {
    let addr = format!("{local_addr}:{server_port}");
    let logmsg = format!("The streaming server is listening on http://{addr}/stream/swyh.wav");
    ui_log(logmsg);
    let logmsg = {
        let cfg = CONFIG.read();
        format!(
            "Streaming sample rate: {}, bits per sample: {}, format: {}",
            wd.sample_rate.0,
            cfg.bits_per_sample.unwrap_or(16),
            cfg.streaming_format.unwrap_or(Flac),
        )
    };
    ui_log(logmsg);
    let server = Arc::new(Server::http(addr).unwrap());
    let mut handles = Vec::new();
    // always have two threads ready to serve new requests
    for _ in 0..2 {
        let server = server.clone();
        let feedback_tx_c = feedback_tx.clone();
        handles.push(std::thread::spawn(move || {
            for rq in server.incoming_requests() {
                let feedback_tx_c = feedback_tx_c.clone();
                // start streaming in a new thread and continue serving new requests
                std::thread::spawn(move || {
                    if cfg!(debug_assertions) {
                        debug!("<== Incoming {:?}", rq);
                        for hdr in rq.headers() {
                            debug!(" <== Incoming Request {:?} from {}", hdr, rq.remote_addr().unwrap());
                        }
                    }
                    // get remote ip
                    let remote_addr = format!("{}", rq.remote_addr().unwrap());
                    let mut remote_ip = remote_addr.clone();
                    if let Some(i) = remote_ip.find(':') {
                        remote_ip.truncate(i);
                    }
                    // default headers
                    let srvr_hdr =
                        Header::from_bytes(&b"Server"[..], &b"swyh-rs tiny-http"[..])
                            .unwrap();
                    let nm_hdr = Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap();
                    let cc_hdr = Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap();
                    // don't accept range headers (Linn) until I know how to handle them
                    let acc_rng_hdr = Header::from_bytes(&b"Accept-Ranges"[..], &b"none"[..]).unwrap();
                    // check url
                    const VALID_URLS: [&str; 4] = ["/stream/swyh.wav", "/stream/swyh.raw", "/stream/swyh.flac", "/stream/swyh.rf64"];
                    if ! VALID_URLS.contains(&rq.url()) {
                        ui_log(format!(
                            "Unrecognized request '{}' from {}'",
                            rq.url(),
                            rq.remote_addr().unwrap()
                        ));
                        let response = Response::empty(404)
                            .with_header(cc_hdr)
                            .with_header(srvr_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http POST connection with {remote_addr} terminated [{e}]"
                            ));
                        }
                        return;
                    }
                    // get remote ip
                    let remote_addr = format!("{}", rq.remote_addr().unwrap());
                    let mut remote_ip = remote_addr.clone();
                    if let Some(i) = remote_ip.find(':') {
                        remote_ip.truncate(i);
                    }
                    // prpare streaming headers
                    let conf = CONFIG.read().clone();
                    let format = conf.streaming_format.unwrap();
                    let ct_text = if format == StreamingFormat::Flac {
                        "audio/flac".to_string()
                    } else if conf.use_wave_format {
                        "audio/vnd.wave;codec=1".to_string()
                    } else { // LPCM
                        if conf.bits_per_sample == Some(16) {
                            format!("audio/L16;rate={};channels=2", wd.sample_rate.0) 
                        } else {
                            format!("audio/L24;rate={};channels=2", wd.sample_rate.0) 
                        }
                    };
                    let ct_hdr = Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap();
                    let tm_hdr =
                        Header::from_bytes(&b"TransferMode.dlna.org"[..], &b"Streaming"[..]).unwrap();
                    // handle response, streaming if GET, headers only otherwise
                    if matches!(rq.method(), Method::Get) {
                        ui_log(format!(
                            "Received request {} from {}",
                            rq.url(),
                            rq.remote_addr().unwrap()
                        ));
                        let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
                        let channel_stream = ChannelStream::new(
                            tx,
                            rx,
                            remote_ip.clone(),
                            conf.use_wave_format,
                            wd.sample_rate.0,
                            conf.bits_per_sample.unwrap_or(16),
                            conf.streaming_format.unwrap_or(Flac),
                        );
                        let nclients = {
                            let mut clients = CLIENTS.write();
                            clients.insert(remote_addr.clone(), channel_stream.clone());
                            clients.len()
                        };
                        debug!("Now have {} streaming clients", nclients);

                        feedback_tx_c
                            .send(StreamerFeedBack {
                                remote_ip: remote_ip.clone(),
                                streaming_state: StreamingState::Started,
                            })
                            .unwrap();
                        std::thread::yield_now();
                        let streaming_format = if format == StreamingFormat::Flac {
                            "audio/FLAC"
                        } else if format == StreamingFormat::Wav {
                            "audio/wave;codec=1 (WAV)"
                        } else { // LPCM
                            if conf.bits_per_sample == Some(16) {
                                "audio/L16 (LPCM)"
                            } else {
                                "audio/L24 (LPCM)"
                            }
                        };
                        ui_log(format!(
                            "Streaming {streaming_format}, input sample format {:?}, channels=2, rate={}, to {}",
                            wd.sample_format,
                            wd.sample_rate.0,
                            rq.remote_addr().unwrap()
                        ));
                        // make sure that tiny-http does not use chunked encoding 
                        let (streamsize, chunksize) = if format == StreamingFormat::Wav {
                             (Some((u32::MAX - 1) as usize), (u32::MAX) as usize)
                         } else {
                            (Some((i64::MAX - 1) as usize), i64::MAX as usize)
                         };
                        let response = Response::empty(200)
                            .with_data(channel_stream, streamsize)
                            .with_chunked_threshold(chunksize)
                            .with_header(cc_hdr)
                            .with_header(ct_hdr)
                            .with_header(tm_hdr)
                            .with_header(srvr_hdr)
                            .with_header(acc_rng_hdr)
                            .with_header(nm_hdr);
                        if cfg!(debug_assertions) {
                           debug!("==> Response:");
                           debug!(" ==> Content-Length: {}", response.data_length().unwrap_or(0));
                            for hdr in response.headers() {
                              debug!(" ==> Response {:?} to {}", hdr, rq.remote_addr().unwrap());
                            }
                        }
                        let e = rq.respond(response);
                        if e.is_err() {
                            ui_log(format!(
                                "=>Http connection with {remote_addr} terminated [{e:?}]"
                            ));
                        }
                        let nclients = {
                            let mut clients = CLIENTS.write();
                            if let Some(chs) = clients.remove(&remote_addr) { chs.stop_flac_encoder() };
                            clients.len()
                        };
                        debug!("Now have {} streaming clients left", nclients);
                        ui_log(format!("Streaming to {remote_addr} has ended"));
                        // inform the main thread that this renderer has finished receiving
                        // necessary if the connection close was not caused by our own GUI
                        // so that we can update the corresponding button state
                        feedback_tx_c
                            .send(StreamerFeedBack {
                                remote_ip,
                                streaming_state: StreamingState::Ended,
                            })
                            .unwrap();
                        app::awake();
                        std::thread::yield_now();
                    } else if matches!(rq.method(), Method::Head) {
                        debug!("HEAD rq from {}", remote_addr);
                        let response = Response::empty(200)
                            .with_header(cc_hdr)
                            .with_header(ct_hdr)
                            .with_header(tm_hdr)
                            .with_header(srvr_hdr)
                            .with_header(acc_rng_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http HEAD connection with {remote_addr} terminated [{e}]"
                            ));
                        }
                    } else if matches!(rq.method(), Method::Post) {
                        debug!("POST rq from {}", remote_addr);
                        let response = Response::empty(200)
                            .with_header(cc_hdr)
                            .with_header(srvr_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http POST connection with {remote_addr} terminated [{e}]"
                            ));
                        }
                    }
                });
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
