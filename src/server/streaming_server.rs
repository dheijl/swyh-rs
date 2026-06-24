//! `tiny-http`-based streaming server.
//!
//! [`run_server`] listens for incoming GET/HEAD requests and serves captured audio
//! as LPCM, WAV, RF64, or FLAC over HTTP with DLNA-compatible headers.
//! Each accepted connection gets its own [`ChannelStream`] fed by the audio capture pipeline.

use crate::{
    audio::rwstream::ChannelStream,
    enums::{
        messages::MessageType,
        streaming::{BitDepth, StreamingContext, StreamingFormat, StreamingState},
    },
    fl,
    globals::statics::{get_clients_mut, get_config},
    renderers::rendercontrol::WavData,
    server::query_params::StreamingParams,
    utils::ui_logger::{LogCategory, ui_log},
};
use crossbeam_channel::{Sender, unbounded};
use ecow::EcoString;
use log::debug;
use std::{io, net::IpAddr, sync::Arc, thread, time::Duration};
use tiny_http::{Header, Method, Response, Server};

/// streaming state feedback for a client
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamerFeedBack {
    pub remote_ip: EcoString,
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
    ui_log(LogCategory::Info, &fl!("srv-listening", "addr" = addr));
    {
        let cfg = get_config();
        ui_log(
            LogCategory::Info,
            &fl!(
                "srv-default-streaming",
                "rate" = wd.sample_rate,
                "bps" = BitDepth::from(cfg.bits_per_sample.unwrap_or(16)),
                "format" = cfg.streaming_format.unwrap_or(StreamingFormat::Flac),
            ),
        );
    } // drop the read lock before entering the server loop
    let server = Arc::new(Server::http(addr).unwrap_or_else(|e| {
        ui_log(
            LogCategory::Error,
            &fl!("srv-start-error", "error" = e.to_string()),
        );
        panic!("Can't start server thread: {e}");
    }));
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
                    #[cfg(debug_assertions)]
                    dump_rq_headers(&rq);
                    // validate the request URL before building the context
                    let sp = StreamingParams::from_url(rq.url());
                    if sp.path.is_none() {
                        return bad_request(rq);
                    }
                    // build a fully-initialised context from all available inputs
                    let streaming_ctx = StreamingContext::new(wd, &rq, &sp);
                    debug!("{streaming_ctx:?}");
                    // handle response, streaming if GET, headers only otherwise
                    let range = parse_range_header(rq.headers());
                    match *rq.method() {
                        Method::Get => {
                            streaming_request(&streaming_ctx, &feedback_tx_c, rq, range);
                        }
                        Method::Head => {
                            head_request(&streaming_ctx, rq, range);
                        }
                        _ => {
                            invalid_request(&streaming_ctx, rq);
                        }
                    }
                });
            }
        }));
    }

    for h in handles {
        h.join().unwrap_or_else(|e| {
            ui_log(
                LogCategory::Error,
                &fl!("srv-thread-error", "error" = format!("{e:?}")),
            );
            panic!("{e:?}");
        });
    }
}

/// dump response headers
#[cfg(debug_assertions)]
fn dump_resp_headers(response: &Response<ChannelStream>) {
    debug!("==> Response:");
    debug!(
        " ==> Content-Length: {}",
        response.data_length().unwrap_or(0)
    );
    for hdr in response.headers() {
        debug!(" ==> Response {hdr:?}");
    }
}

/// dump the request headers
#[cfg(debug_assertions)]
fn dump_rq_headers(rq: &tiny_http::Request) {
    for hdr in rq.headers() {
        debug!(" <== Request {hdr:?}");
    }
}

/// GET METHOD request - request to start streaming
fn streaming_request(
    streaming_ctx: &StreamingContext,
    feedback_channel: &Sender<MessageType>,
    request: tiny_http::Request,
    range: Option<RangeSpec>,
) {
    // Determine HTTP status code and how many header bytes to skip based on the Range request.
    // Linn streamers typically send `Range: bytes=0-`; we respond 206 and start from byte 0.
    // A range starting within the WAV/RF64 header is also satisfiable by trimming the header.
    // Anything else (bounded range or offset past the header) is rejected with 416.
    let (status_code, header_offset) = match &range {
        None => (200u16, 0usize),
        Some(RangeSpec::Bounded) => {
            return range_not_satisfiable(streaming_ctx, request);
        }
        Some(RangeSpec::From(start)) => {
            let hdr_size = streaming_ctx.wav_header_size() as u64;
            if *start <= hdr_size {
                // Start within the WAV/RF64 header (or byte 0 for any format): 206,
                // trim that many header bytes from the front of the stream.
                (206u16, *start as usize)
            } else {
                // Start is past the header and into the audio data — we cannot seek
                // into a live stream. Fall back to 200 and serve from the beginning,
                // which matches the pre-range-support behaviour and keeps MPD happy
                // (MPD probes with Range: bytes=<Content-Length>- after reading the
                // WAV header to verify the announced size).
                (200u16, 0usize)
            }
        }
    };

    ui_log(
        LogCategory::Info,
        &fl!(
            "srv-streaming-request",
            "url" = &streaming_ctx.url,
            "addr" = &streaming_ctx.remote_addr
        ),
    );
    // get the dlna headers
    let mut headers = get_dlna_headers(streaming_ctx);
    if status_code == 206 {
        headers.push(Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap());
        let cr = streaming_ctx.content_range_value(header_offset);
        headers.push(Header::from_bytes(&b"Content-Range"[..], cr.as_bytes()).unwrap());
    }

    // create the channelstream that receives the samples and streams them on demand
    let (tx, rx) = unbounded();
    let channel_stream = ChannelStream::new(tx, rx, streaming_ctx, header_offset);
    let nclients = {
        let mut clients = get_clients_mut();
        clients.insert(streaming_ctx.remote_addr.clone(), channel_stream.clone());
        clients.len()
    };
    debug!("Now have {nclients} streaming clients");

    feedback_channel
        .send(MessageType::PlayerMessage(StreamerFeedBack {
            remote_ip: streaming_ctx.remote_ip.clone(),
            streaming_state: StreamingState::Started,
        }))
        .unwrap_or_else(|e| {
            ui_log(
                LogCategory::Error,
                &fl!("srv-feedback-error", "error" = format!("{e:?}")),
            );
            panic!("Http server feedback error:{e:?}");
        });

    // check for upfront audio buffering needed
    if streaming_ctx.buffering_delay_msec > 0 {
        thread::sleep(Duration::from_millis(
            streaming_ctx.buffering_delay_msec.into(),
        ));
    }
    ui_log(
        LogCategory::Info,
        &fl!(
            "srv-streaming-info",
            "audio" = streaming_ctx.dlna_audio_string(),
            "fmt" = format!("{:?}", streaming_ctx.sample_format),
            "rate" = streaming_ctx.sample_rate,
            "bps" = streaming_ctx.bits_per_sample,
            "addr" = &streaming_ctx.remote_addr,
        ),
    );
    let response = Response::new(
        tiny_http::StatusCode(status_code),
        headers,
        channel_stream,
        streaming_ctx.streamsize,
        None,
    )
    .with_chunked_threshold(streaming_ctx.chunksize);
    #[cfg(debug_assertions)]
    dump_resp_headers(&response);
    let e = request.respond(response);
    if e.is_err() {
        ui_log(
            LogCategory::Error,
            &fl!(
                "srv-http-terminated",
                "addr" = &streaming_ctx.remote_addr,
                "error" = format!("{e:?}")
            ),
        );
    }
    let nclients = {
        let mut clients = get_clients_mut();
        if let Some(chs) = clients.remove(&streaming_ctx.remote_addr) {
            chs.stop_flac_encoder();
        };
        clients.len()
    };
    debug!("Now have {nclients} streaming clients left");
    // inform the main thread that this renderer has finished receiving
    // necessary if the connection close was not caused by our own GUI
    // so that we can update the corresponding button state
    feedback_channel
        .send(MessageType::PlayerMessage(StreamerFeedBack {
            remote_ip: streaming_ctx.remote_ip.clone(),
            streaming_state: StreamingState::Ended,
        }))
        .unwrap_or_else(|e| {
            ui_log(
                LogCategory::Error,
                &fl!("srv-feedback-error", "error" = e.to_string()),
            );
            panic!("Http server feedback channel error:{e}");
        });
    ui_log(
        LogCategory::Info,
        &fl!("srv-streaming-ended", "addr" = &streaming_ctx.remote_addr),
    );
}

/// HEAD METHOD request
fn head_request(
    streaming_ctx: &StreamingContext,
    rq: tiny_http::Request,
    range: Option<RangeSpec>,
) {
    debug!("HEAD rq from {}", streaming_ctx.remote_addr);
    let (status_code, header_offset) = match &range {
        None => (200u16, 0usize),
        Some(RangeSpec::Bounded) => {
            return range_not_satisfiable(streaming_ctx, rq);
        }
        Some(RangeSpec::From(start)) => {
            let hdr_size = streaming_ctx.wav_header_size() as u64;
            if *start <= hdr_size {
                (206u16, *start as usize)
            } else {
                (200u16, 0usize)
            }
        }
    };
    // get the dlna headers
    let mut headers = get_dlna_headers(streaming_ctx);
    if status_code == 206 {
        headers.push(Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap());
        let cr = streaming_ctx.content_range_value(header_offset);
        headers.push(Header::from_bytes(&b"Content-Range"[..], cr.as_bytes()).unwrap());
    }
    let response = Response::new(
        tiny_http::StatusCode(status_code),
        headers,
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(
            LogCategory::Error,
            &fl!(
                "srv-head-terminated",
                "addr" = &streaming_ctx.remote_addr,
                "error" = e.to_string()
            ),
        );
    }
}

/// invalid METHOD request
fn invalid_request(streaming_ctx: &StreamingContext, rq: tiny_http::Request) {
    ui_log(
        LogCategory::Error,
        &fl!(
            "srv-unsupported-method",
            "method" = format!("{:?}", *rq.method()),
            "addr" = &streaming_ctx.remote_addr
        ),
    );
    let headers = get_std_headers();
    let response = Response::new(
        tiny_http::StatusCode(405),
        headers,
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(
            LogCategory::Error,
            &fl!(
                "srv-http-terminated",
                "addr" = &streaming_ctx.remote_addr,
                "error" = e.to_string()
            ),
        );
    }
}

/// this request is not recognised, reject with 404
fn bad_request(rq: tiny_http::Request) {
    let remote_addr = rq.remote_addr().map(|a| a.to_string()).unwrap_or_default();
    ui_log(
        LogCategory::Warning,
        &fl!("srv-bad-request", "url" = rq.url(), "addr" = remote_addr),
    );
    let headers = get_std_headers();
    let response = Response::new(
        tiny_http::StatusCode(404),
        headers,
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(
            LogCategory::Error,
            &fl!(
                "srv-stream-terminated",
                "addr" = remote_addr,
                "error" = e.to_string()
            ),
        );
    }
}

/// get the dlna headers
fn get_dlna_headers(streaming_ctx: &StreamingContext) -> Vec<Header> {
    let mut headers = get_std_headers();
    // get the dlna format string
    let ct_text = match streaming_ctx.streaming_format {
        StreamingFormat::Flac => "audio/flac".to_string(),
        StreamingFormat::Wav | StreamingFormat::Rf64 => "audio/vnd.wave;codec=1".to_string(),
        StreamingFormat::Lpcm => match streaming_ctx.bits_per_sample {
            BitDepth::Bits16 => {
                format!("audio/L16;rate={};channels=2", streaming_ctx.sample_rate)
            }
            BitDepth::Bits24 => {
                format!("audio/L24;rate={};channels=2", streaming_ctx.sample_rate)
            }
        },
    };
    // and add dlna headers
    headers.push(Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap());
    headers.push(Header::from_bytes(&b"TransferMode.dlna.org"[..], &b"Streaming"[..]).unwrap());
    headers
}

/// get the standard headers
fn get_std_headers() -> Vec<Header> {
    // reserve space for dlna headers to be added later
    let mut headers = Vec::with_capacity(8);
    headers.push(Header::from_bytes(&b"Server"[..], &b"swyh-rs tiny-http"[..]).unwrap());
    headers.push(Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap());
    headers.push(Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap());
    headers
}

/// A parsed HTTP Range header of the form `bytes=<start>-[<end>]`.
enum RangeSpec {
    /// `bytes=<start>-` — open-ended; only `start` is known
    From(u64),
    /// `bytes=<start>-<end>` — bounded range; cannot be honoured on a live stream
    Bounded,
}

/// Parse the first `Range: bytes=…` header present in `headers`, if any.
fn parse_range_header(headers: &[tiny_http::Header]) -> Option<RangeSpec> {
    let h = headers.iter().find(|h| h.field.equiv("range"))?;
    let rest = h.value.as_str().strip_prefix("bytes=")?;
    let (start_str, end_str) = rest.split_once('-')?;
    let start = start_str.parse::<u64>().ok()?;
    if end_str.is_empty() {
        Some(RangeSpec::From(start))
    } else {
        Some(RangeSpec::Bounded)
    }
}

/// Respond 416 Range Not Satisfiable.
fn range_not_satisfiable(streaming_ctx: &StreamingContext, rq: tiny_http::Request) {
    ui_log(
        LogCategory::Warning,
        &fl!(
            "srv-range-not-satisfiable",
            "addr" = &streaming_ctx.remote_addr
        ),
    );
    let mut headers = get_std_headers();
    headers.push(Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap());
    let cr = match streaming_ctx.streamsize {
        Some(size) => format!("bytes */{size}"),
        None => "bytes */*".to_string(),
    };
    headers.push(Header::from_bytes(&b"Content-Range"[..], cr.as_bytes()).unwrap());
    let response = Response::new(
        tiny_http::StatusCode(416),
        headers,
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(
            LogCategory::Error,
            &fl!(
                "srv-http-terminated",
                "addr" = &streaming_ctx.remote_addr,
                "error" = e.to_string()
            ),
        );
    }
}
