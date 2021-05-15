use crate::{ui_log, ChannelStream, StreamerFeedBack, StreamingState, WavData, CLIENTS, CONFIG};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fltk::app;
use log::debug;
use std::net::IpAddr;
use std::sync::Arc;
use tiny_http::{Header, Method, Response, Server};

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
    let addr = format!("{}:{}", local_addr, server_port);
    let logmsg = format!(
        "The streaming server is listening on http://{}/stream/swyh.wav",
        addr,
    );
    ui_log(logmsg);
    let logmsg = format!(
        "Sample rate: {}, sample format: audio/l16 (PCM)",
        wd.sample_rate.0.to_string(),
    );
    ui_log(logmsg);
    let server = Arc::new(Server::http(addr).unwrap());
    let mut handles = Vec::new();
    for _ in 0..2 {
        let server = server.clone();
        let feedback_tx_c = feedback_tx.clone();
        handles.push(std::thread::spawn(move || {
            for rq in server.incoming_requests() {
                let feedback_tx_c = feedback_tx_c.clone();
                let _ = std::thread::spawn(move || {
                    // get remote ip
                    let remote_addr = format!("{}", rq.remote_addr());
                    let mut remote_ip = remote_addr.clone();
                    if let Some(i) = remote_ip.find(':') {
                        remote_ip.truncate(i);
                    }
                    // default headers
                    let srvr_hdr =
                        Header::from_bytes(&b"Server"[..], &b"UPnP/1.0 DLNADOC/1.50 LAB/1.0"[..])
                            .unwrap();
                    let nm_hdr = Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap();
                    let cc_hdr = Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap();
                    // check url
                    if rq.url() != "/stream/swyh.wav" {
                        ui_log(format!(
                            "Unrecognized request '{}' from {}'",
                            rq.url(),
                            rq.remote_addr()
                        ));
                        let response = Response::empty(404)
                            .with_header(cc_hdr)
                            .with_header(srvr_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http POST connection with {} terminated [{}]",
                                remote_addr, e
                            ));
                        }
                        return;
                    }
                    // get remote ip
                    let remote_addr = format!("{}", rq.remote_addr());
                    let mut remote_ip = remote_addr.clone();
                    if let Some(i) = remote_ip.find(':') {
                        remote_ip.truncate(i);
                    }
                    // prpare streaming headers
                    let conf = CONFIG.read().clone();
                    let ct_text = if conf.use_wave_format {
                        "audio/vnd.wave;codec=1".to_string()
                    } else {
                        format!("audio/L16;rate={};channels=2", wd.sample_rate.0.to_string())
                    };
                    let ct_hdr = Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap();
                    let tm_hdr =
                        Header::from_bytes(&b"TransferMode.DLNA.ORG"[..], &b"Streaming"[..]).unwrap();
                    // handle response, streaming if GET, headers only otherwise
                    if matches!(rq.method(), Method::Get) {
                        ui_log(format!(
                            "Received request {} from {}",
                            rq.url(),
                            rq.remote_addr()
                        ));
                        // set transfer encoding chunked unless disabled
                        let (streamsize, chunked_threshold) = {
                            if conf.disable_chunked {
                                (Some(usize::MAX - 1), usize::MAX)
                            } else {
                                (None, 8192)
                            }
                        };
                        let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = unbounded();
                        let channel_stream = ChannelStream::new(
                            tx.clone(),
                            rx.clone(),
                            remote_ip.clone(),
                            conf.use_wave_format,
                            wd.sample_rate.0,
                        );
                        let nclients = {
                            let mut clients = CLIENTS.write();
                            clients.insert(remote_addr.clone(), channel_stream);
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
                        let mut channel_stream = ChannelStream::new(
                            tx,
                            rx,
                            remote_ip.clone(),
                            conf.use_wave_format,
                            wd.sample_rate.0,
                        );
                        channel_stream.create_silence(wd.sample_rate.0);
                        let streaming_format = if conf.use_wave_format {
                            "audio/wave;codec=1 (WAV)"
                        } else {
                            "audio/l16 (LPCM)"
                        };
                        ui_log(format!(
                            "Streaming {}, input sample format {:?}, channels=2, rate={}, disable chunked={} to {}",
                            streaming_format,
                            wd.sample_format,
                            wd.sample_rate.0,
                            conf.disable_chunked,
                            rq.remote_addr()
                        ));
                        let response = Response::empty(200)
                            .with_data(channel_stream, streamsize)
                            .with_chunked_threshold(chunked_threshold)
                            .with_header(cc_hdr)
                            .with_header(ct_hdr)
                            .with_header(tm_hdr)
                            .with_header(srvr_hdr)
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http connection with {} terminated [{}]",
                                remote_addr, e
                            ));
                        }
                        let nclients = {
                            let mut clients = CLIENTS.write();
                            clients.remove(&remote_addr);
                            clients.len()
                        };
                        debug!("Now have {} streaming clients left", nclients);
                        ui_log(format!("Streaming to {} has ended", remote_addr));
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
                            .with_header(nm_hdr);
                        if let Err(e) = rq.respond(response) {
                            ui_log(format!(
                                "=>Http HEAD connection with {} terminated [{}]",
                                remote_addr, e
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
                                "=>Http POST connection with {} terminated [{}]",
                                remote_addr, e
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
