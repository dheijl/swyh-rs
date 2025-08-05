use crate::{
    enums::{
        messages::MessageType,
        streaming::{BitDepth, StreamContext, StreamingFormat, StreamingState},
    },
    globals::statics::get_clients_mut,
    openhome::rendercontrol::WavData,
    server::query_params::StreamingParams,
    utils::{rwstream::ChannelStream, ui_logger::ui_log},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use log::debug;
use std::{io, net::IpAddr, sync::Arc, thread, time::Duration};
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
    // get the needed config info upfront (this struct is Copy)
    let stream_config = StreamContext::from_config();
    let logmsg = {
        format!(
            "Default streaming sample rate: {}, bits per sample: {}, format: {}",
            wd.sample_rate.0, stream_config.bits_per_sample, stream_config.streaming_format,
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
                // start streaming in a new thread and continue serving new requests
                let feedback_tx_c = feedback_tx_c.clone();
                std::thread::spawn(move || {
                    debug!(
                        "{:?} {} from {}",
                        *rq.method(),
                        rq.url(),
                        rq.remote_addr().unwrap()
                    );
                    if cfg!(debug_assertions) {
                        dump_rq_headers(&rq);
                    }
                    // refresh context from config info for each new streaming request
                    // as some parameters may have changed
                    let mut stream_context = StreamContext::from_config();
                    // parse the GET request and update context
                    stream_context.remote_addr = rq.remote_addr().unwrap().to_string();
                    stream_context.remote_ip = get_remote_ip(&stream_context.remote_addr);
                    // update context from WavData
                    stream_context.sample_rate = wd.sample_rate.0;
                    stream_context.sample_format = wd.sample_format;
                    // - build standard headers
                    let mut headers = get_default_headers();
                    //  - decode streaming query params if present
                    let sp = StreamingParams::from_query_string(rq.url());
                    // - check for valid request uri
                    if sp.path.is_none() {
                        return unrecognized_request(rq, &stream_context.remote_addr, &headers);
                    }
                    // - update streaming context from querystring (if present)
                    stream_context.update_format(&sp);
                    // - now add the dlna headers to the header collection
                    add_dlna_headers(&mut headers, &stream_context);
                    // handle response, streaming if GET, headers only otherwise
                    match *rq.method() {
                        Method::Get => {
                            ui_log(&format!(
                                "Streaming request {} from {}",
                                rq.url(),
                                stream_context.remote_addr
                            ));
                            let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
                            let channel_stream = ChannelStream::new(
                                tx,
                                rx,
                                stream_context.remote_ip.clone(),
                                stream_context.streaming_format.needs_wav_hdr(),
                                stream_context.sample_rate,
                                stream_context.bits_per_sample,
                                stream_context.streaming_format,
                            );
                            let nclients = {
                                let mut clients = get_clients_mut();
                                clients.insert(
                                    stream_context.remote_addr.clone(),
                                    channel_stream.clone(),
                                );
                                clients.len()
                            };
                            debug!("Now have {nclients} streaming clients");

                            feedback_tx_c
                                .send(MessageType::PlayerMessage(StreamerFeedBack {
                                    remote_ip: stream_context.remote_ip.clone(),
                                    streaming_state: StreamingState::Started,
                                }))
                                .unwrap();

                            // check for upfront audio buffering needed
                            if stream_context.buffering_delay_msec > 0 {
                                thread::sleep(Duration::from_millis(
                                    stream_context.buffering_delay_msec.into(),
                                ));
                            }
                            let dlna_streaming_format = stream_context
                                .streaming_format
                                .dlna_string(BitDepth::from(stream_context.bits_per_sample));
                            ui_log(&format!(
                                "Streaming {dlna_streaming_format}, input sample format {:?}, \
                            channels=2, rate={}, bps = {}, to {}",
                                stream_context.sample_format,
                                stream_context.sample_rate,
                                stream_context.bits_per_sample,
                                stream_context.remote_addr
                            ));
                            let response = Response::new(
                                tiny_http::StatusCode(200),
                                headers,
                                channel_stream,
                                stream_context.streamsize,
                                None,
                            )
                            .with_chunked_threshold(stream_context.chunksize);
                            dump_resp_headers(&response);
                            let e = rq.respond(response);
                            if e.is_err() {
                                ui_log(&format!(
                                    "=>Http connection with {} terminated [{e:?}]",
                                    stream_context.remote_addr
                                ));
                            }
                            let nclients = {
                                let mut clients = get_clients_mut();
                                if let Some(chs) = clients.remove(&stream_context.remote_addr) {
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
                                    remote_ip: stream_context.remote_ip,
                                    streaming_state: StreamingState::Ended,
                                }))
                                .unwrap();
                            ui_log(&format!(
                                "Streaming to {} has ended",
                                stream_context.remote_addr
                            ));
                        }
                        Method::Head => {
                            debug!("HEAD rq from {}", stream_context.remote_addr);
                            let response = Response::new(
                                tiny_http::StatusCode(200),
                                headers,
                                io::empty(),
                                Some(0),
                                None,
                            );
                            if let Err(e) = rq.respond(response) {
                                ui_log(&format!(
                                    "=>Http HEAD connection with {} terminated [{e}]",
                                    stream_context.remote_addr
                                ));
                            }
                        }
                        _ => {
                            ui_log(&format!(
                                "Unsupported HTTP method request {:?} from {}",
                                *rq.method(),
                                stream_context.remote_addr
                            ));
                            let response = Response::new(
                                tiny_http::StatusCode(405),
                                headers,
                                io::empty(),
                                Some(0),
                                None,
                            );
                            if let Err(e) = rq.respond(response) {
                                ui_log(&format!(
                                    "=>Http connection with {} terminated [{e}]",
                                    stream_context.remote_addr
                                ));
                            }
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

/// extract IP address from remote address
fn get_remote_ip(remote_addr: &str) -> String {
    let mut remote_ip = remote_addr.to_owned();
    if let Some(i) = remote_ip.find(':') {
        remote_ip.truncate(i);
    }
    remote_ip
}

/// dump response headers
fn dump_resp_headers(response: &Response<ChannelStream>) {
    if cfg!(debug_assertions) {
        debug!("==> Response:");
        debug!(
            " ==> Content-Length: {}",
            response.data_length().unwrap_or(0)
        );
        for hdr in response.headers() {
            debug!(" ==> Response {hdr:?}");
        }
    }
}

/// dump the request headers
fn dump_rq_headers(rq: &tiny_http::Request) {
    for hdr in rq.headers() {
        debug!(" <== Request {hdr:?}");
    }
}

/// this request is not recognized, reject with an error 404
fn unrecognized_request(rq: tiny_http::Request, remote_addr: &str, headers: &[Header]) {
    ui_log(&format!(
        "Unrecognized request '{}' from {}'",
        rq.url(),
        rq.remote_addr().unwrap()
    ));
    let response = Response::new(
        tiny_http::StatusCode(404),
        headers.to_vec(),
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(&format!(
            "=>Http streaming request with {remote_addr} terminated [{e}]"
        ));
    }
}

/// Add the dlna headers
fn add_dlna_headers(headers: &mut Vec<Header>, stream_context: &StreamContext) {
    // get the dlna format string
    let ct_text = {
        if stream_context.streaming_format == StreamingFormat::Flac {
            "audio/flac".to_string()
        } else if stream_context.streaming_format == StreamingFormat::Wav
            || stream_context.streaming_format == StreamingFormat::Rf64
        {
            "audio/vnd.wave;codec=1".to_string()
        } else {
            // LPCM
            if BitDepth::from(stream_context.bits_per_sample) == BitDepth::Bits16 {
                format!(
                    "audio/L16;rate={};channels=2",
                    stream_context.bits_per_sample
                )
            } else {
                format!(
                    "audio/L24;rate={};channels=2",
                    stream_context.bits_per_sample
                )
            }
        }
    };
    // and add dlna headers
    headers.push(Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap());
    headers.push(Header::from_bytes(&b"TransferMode.dlna.org"[..], &b"Streaming"[..]).unwrap());
}

// get the default http response headers
fn get_default_headers() -> Vec<Header> {
    let mut headers = Vec::with_capacity(8);
    headers.push(Header::from_bytes(&b"Server"[..], &b"swyh-rs tiny-http"[..]).unwrap());
    headers.push(Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap());
    headers.push(Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap());
    // don't accept range headers (Linn) until I know how to handle them
    // but don't send this header as the MPD player ignores the "none" value anyway and uses ranges
    // headers.push(Header::from_bytes(&b"Accept-Ranges"[..], &b"none"[..]).unwrap());
    headers
}
