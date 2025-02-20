use crate::{
    enums::{
        messages::MessageType,
        streaming::{
            BitDepth,
            StreamingFormat::{self, Flac, Lpcm, Rf64, Wav},
            StreamingState,
        },
    },
    globals::statics::{get_clients_mut, get_config},
    openhome::rendercontrol::WavData,
    server::query_params::StreamingParams,
    utils::{rwstream::ChannelStream, ui_logger::ui_log},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use log::debug;
use std::{net::IpAddr, sync::Arc, thread, time::Duration};
use tiny_http::{Header, Method, Response, Server};

/// streaming state feedback for a client
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamerFeedBack {
    pub remote_ip: String,
    pub streaming_state: StreamingState,
}

/// `run_server` - run a tiny-http webserver to serve streaming requests from renderers
///
/// all music is sent with the sample rate of the source in the requested audio format (lpcm/wav/rf64/flac)
/// in the requested bit depth (16 or 24)
/// the samples are read as f32 slices from a crossbeam channel fed by the `wave_reader`
/// a `ChannelStream` is created for this purpose, and inserted in the array of active
/// "clients" for the `wave_reader`
pub fn run_server(
    local_addr: &IpAddr,
    server_port: u16,
    wd: WavData,
    feedback_tx: &Sender<MessageType>,
) {
    let addr = format!("{local_addr}:{server_port}");
    ui_log(&format!(
        "The streaming server is listening on http://{addr}/stream/swyh.wav"
    ));
    let logmsg = {
        let cfg = get_config();
        format!(
            "Default streaming sample rate: {}, bits per sample: {}, format: {}",
            wd.sample_rate.0,
            cfg.bits_per_sample.unwrap_or(16),
            cfg.streaming_format.unwrap_or(Flac),
        )
    };
    ui_log(&logmsg);
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
                            debug!(
                                " <== Incoming Request {hdr:?} from {}",
                                rq.remote_addr().unwrap()
                            );
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
                        Header::from_bytes(&b"Server"[..], &b"swyh-rs tiny-http"[..]).unwrap();
                    let nm_hdr = Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap();
                    let cc_hdr = Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap();
                    // don't accept range headers (Linn) until I know how to handle them
                    let acc_rng_hdr =
                        Header::from_bytes(&b"Accept-Ranges"[..], &b"none"[..]).unwrap();
                    // parse the GET request
                    let sp = StreamingParams::from_query_string(rq.url());
                    // check url
                    if sp.path.is_none() {
                        ui_log(&format!(
                            "Unrecognized request '{}' from {}'",
                            rq.url(),
                            rq.remote_addr().unwrap()
                        ));
                        let response = Response::empty(404)
                            .with_header(cc_hdr)
                            .with_header(srvr_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(&format!(
                                "=>Http streaming request with {remote_addr} terminated [{e}]"
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
                    // prepare streaming headers
                    let conf = get_config().clone();
                    // format from config or from GET Path
                    let cf_format = conf.streaming_format.unwrap_or(Lpcm);
                    let format = if let Some(fmt) = sp.fmt {
                        fmt
                    } else {
                        cf_format
                    };
                    // bit depth from config or from GET query string
                    let cf_bps = conf.bits_per_sample.unwrap_or(16);
                    // check if client requests the configured format
                    let bps = if let Some(bd) = sp.bd {
                        bd
                    } else {
                        BitDepth::from(cf_bps)
                    };
                    let ct_text = if format == StreamingFormat::Flac {
                        "audio/flac".to_string()
                    } else if format == StreamingFormat::Wav || format == StreamingFormat::Rf64 {
                        "audio/vnd.wave;codec=1".to_string()
                    } else {
                        // LPCM
                        if bps == BitDepth::Bits16 {
                            format!("audio/L16;rate={};channels=2", wd.sample_rate.0)
                        } else {
                            format!("audio/L24;rate={};channels=2", wd.sample_rate.0)
                        }
                    };
                    let ct_hdr =
                        Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap();
                    let tm_hdr =
                        Header::from_bytes(&b"TransferMode.dlna.org"[..], &b"Streaming"[..])
                            .unwrap();
                    // handle response, streaming if GET, headers only otherwise
                    if matches!(rq.method(), Method::Get) {
                        ui_log(&format!(
                            "Received request {} from {}",
                            rq.url(),
                            rq.remote_addr().unwrap()
                        ));
                        let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
                        let use_wav_hdr =
                            [StreamingFormat::Wav, StreamingFormat::Rf64].contains(&format);
                        let channel_stream = ChannelStream::new(
                            tx,
                            rx,
                            remote_ip.clone(),
                            use_wav_hdr,
                            wd.sample_rate.0,
                            bps as u16,
                            format,
                        );
                        let nclients = {
                            let mut clients = get_clients_mut();
                            clients.insert(remote_addr.clone(), channel_stream.clone());
                            clients.len()
                        };
                        debug!("Now have {} streaming clients", nclients);

                        feedback_tx_c
                            .send(MessageType::PlayerMessage(StreamerFeedBack {
                                remote_ip: remote_ip.clone(),
                                streaming_state: StreamingState::Started,
                            }))
                            .unwrap();

                        // check for upfront audio buffering needed
                        if let Some(bufferdelay) = conf.buffering_delay_msec {
                            if bufferdelay > 0 {
                                thread::sleep(Duration::from_millis(bufferdelay.into()));
                            }
                        }

                        let streaming_format = match format {
                            Flac => "audio/FLAC",
                            Wav | Rf64 => "audio/wave;codec=1 (WAV)",
                            Lpcm => {
                                if bps == BitDepth::Bits16 {
                                    "audio/L16 (LPCM)"
                                } else {
                                    "audio/L24 (LPCM)"
                                }
                            }
                        };
                        ui_log(&format!(
                            "Streaming {streaming_format}, input sample format {:?}, \
                            channels=2, rate={}, bps = {}, to {}",
                            wd.sample_format,
                            wd.sample_rate.0,
                            bps as u16,
                            rq.remote_addr().unwrap()
                        ));
                        // use the configured content length and chunksize params
                        let (mut streamsize, mut chunksize) = match format {
                            Lpcm => conf.lpcm_stream_size.unwrap().values(),
                            Wav => conf.wav_stream_size.unwrap().values(),
                            Rf64 => conf.rf64_stream_size.unwrap().values(),
                            Flac => conf.flac_stream_size.unwrap().values(),
                        };
                        // unless overridden by the GET query string
                        if sp.ss.is_some() {
                            (streamsize, chunksize) = sp.ss.unwrap().values();
                        }
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
                            debug!(
                                " ==> Content-Length: {}",
                                response.data_length().unwrap_or(0)
                            );
                            for hdr in response.headers() {
                                debug!(" ==> Response {:?} to {}", hdr, rq.remote_addr().unwrap());
                            }
                        }
                        let e = rq.respond(response);
                        if e.is_err() {
                            ui_log(&format!(
                                "=>Http connection with {remote_addr} terminated [{e:?}]"
                            ));
                        }
                        let nclients = {
                            let mut clients = get_clients_mut();
                            if let Some(chs) = clients.remove(&remote_addr) {
                                chs.stop_flac_encoder();
                            };
                            clients.len()
                        };
                        debug!("Now have {nclients} streaming clients left");
                        // inform the main thread that this renderer has finished receiving
                        // necessary if the connection close was not caused by our own GUI
                        // so that we can update the corresponding button state
                        feedback_tx_c
                            .send(MessageType::PlayerMessage(StreamerFeedBack {
                                remote_ip,
                                streaming_state: StreamingState::Ended,
                            }))
                            .unwrap();
                        ui_log(&format!("Streaming to {remote_addr} has ended"));
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
                            ui_log(&format!(
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
                            ui_log(&format!(
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
