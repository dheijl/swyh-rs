use crate::{
    enums::{
        messages::MessageType,
        streaming::{BitDepth, StreamingContext, StreamingFormat, StreamingState},
    },
    globals::statics::get_clients_mut,
    openhome::rendercontrol::WavData,
    server::query_params::StreamingParams,
    utils::rwstream::ChannelStream,
    utils::ui_logger::{LogCategory, ui_log},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
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
    ui_log(
        LogCategory::Info,
        &format!("The streaming server is listening on http://{addr}/stream/swyh.wav"),
    );
    // get the needed config info upfront
    let stream_config = StreamingContext::from_config();
    let logmsg = {
        format!(
            "Default streaming sample rate: {}, bits per sample: {}, format: {}",
            wd.sample_rate.0, stream_config.bits_per_sample, stream_config.streaming_format,
        )
    };
    ui_log(LogCategory::Info, &logmsg);
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
                    #[cfg(debug_assertions)]
                    dump_rq_headers(&rq);
                    // create fresh streaming context from config info for each new streaming request
                    // as some parameters may have changed
                    let mut streaming_ctx = StreamingContext::from_config();
                    // parse the GET request and update context
                    streaming_ctx.set_remote_addr(&rq);
                    // update context from WavData
                    streaming_ctx.set_sample_data(wd);
                    //  - decode streaming query params if present
                    let sp = StreamingParams::from_query_string(rq.url());
                    // - check for valid request uri
                    if sp.path.is_none() {
                        return bad_request(rq, &streaming_ctx.remote_addr);
                    }
                    // - update streaming context from querystring (if present), this completes the context
                    streaming_ctx.update_format(&sp);
                    debug!("{streaming_ctx:?}");
                    // handle response, streaming if GET, headers only otherwise
                    match *rq.method() {
                        Method::Get => {
                            streaming_request(&streaming_ctx, &feedback_tx_c, rq);
                        }
                        Method::Head => {
                            head_request(&streaming_ctx, rq);
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
        h.join().unwrap();
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
) {
    ui_log(
        LogCategory::Info,
        &format!(
            "Streaming request {} from {}",
            streaming_ctx.url, streaming_ctx.remote_addr
        ),
    );
    // get the dlna headers
    let headers = get_dlna_headers(streaming_ctx);

    // create the channelstream that receives the samples and streams them on demand
    let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();
    let channel_stream = ChannelStream::new(
        tx,
        rx,
        streaming_ctx.remote_ip.clone(),
        streaming_ctx.needs_wav_hdr(),
        streaming_ctx.sample_rate,
        streaming_ctx.bits_per_sample as u16,
        streaming_ctx.streaming_format,
    );
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
        .unwrap();

    // check for upfront audio buffering needed
    if streaming_ctx.buffering_delay_msec > 0 {
        thread::sleep(Duration::from_millis(
            streaming_ctx.buffering_delay_msec.into(),
        ));
    }
    ui_log(
        LogCategory::Info,
        &format!(
            "Streaming {}, input sample format {:?}, \
                            channels=2, rate={}, bps = {}, to {}",
            streaming_ctx.dlna_string(),
            streaming_ctx.sample_format,
            streaming_ctx.sample_rate,
            streaming_ctx.bits_per_sample,
            streaming_ctx.remote_addr
        ),
    );
    let response = Response::new(
        tiny_http::StatusCode(200),
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
            LogCategory::Info,
            &format!(
                "=>Http connection with {} terminated [{e:?}]",
                streaming_ctx.remote_addr
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
        .unwrap();
    ui_log(
        LogCategory::Info,
        &format!("Streaming to {} has ended", streaming_ctx.remote_addr),
    );
}

/// HEAD METHOD request
fn head_request(streaming_ctx: &StreamingContext, rq: tiny_http::Request) {
    debug!("HEAD rq from {}", streaming_ctx.remote_addr);
    // get the dlna headers
    let headers = get_dlna_headers(streaming_ctx);

    let response = Response::new(
        tiny_http::StatusCode(200),
        headers,
        io::empty(),
        Some(0),
        None,
    );
    if let Err(e) = rq.respond(response) {
        ui_log(
            LogCategory::Info,
            &format!(
                "=>Http HEAD connection with {} terminated [{e}]",
                streaming_ctx.remote_addr
            ),
        );
    }
}

/// invalid METHOD request
fn invalid_request(streaming_ctx: &StreamingContext, rq: tiny_http::Request) {
    ui_log(
        LogCategory::Info,
        &format!(
            "Unsupported HTTP method request {:?} from {}",
            *rq.method(),
            streaming_ctx.remote_addr
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
            LogCategory::Info,
            &format!(
                "=>Http connection with {} terminated [{e}]",
                streaming_ctx.remote_addr
            ),
        );
    }
}

/// this request is not recognized, reject with an error 404
fn bad_request(rq: tiny_http::Request, remote_addr: &str) {
    ui_log(
        LogCategory::Info,
        &format!(
            "Unrecognized request '{}' from {}'",
            rq.url(),
            rq.remote_addr().unwrap()
        ),
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
            LogCategory::Info,
            &format!("=>Http streaming request with {remote_addr} terminated [{e}]"),
        );
    }
}

/// get the dlna headers
fn get_dlna_headers(stream_context: &StreamingContext) -> Vec<Header> {
    let mut headers = get_std_headers();
    // get the dlna format string
    let ct_text = {
        match stream_context.streaming_format {
            StreamingFormat::Flac => "audio/flac".to_string(),
            StreamingFormat::Wav | StreamingFormat::Rf64 => "audio/vnd.wave;codec=1".to_string(),
            StreamingFormat::Lpcm => match stream_context.bits_per_sample {
                BitDepth::Bits16 => {
                    format!("audio/L16;rate={};channels=2", stream_context.sample_rate)
                }
                BitDepth::Bits24 => {
                    format!("audio/L24;rate={};channels=2", stream_context.sample_rate)
                }
            },
        }
    };
    // and add dlna headers
    headers.push(Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap());
    headers.push(Header::from_bytes(&b"TransferMode.dlna.org"[..], &b"Streaming"[..]).unwrap());
    headers
}

/// get the standard headers
fn get_std_headers() -> Vec<Header> {
    let mut headers = Vec::with_capacity(8);
    headers.push(Header::from_bytes(&b"Server"[..], &b"swyh-rs tiny-http"[..]).unwrap());
    headers.push(Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap());
    headers.push(Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap());

    /* don't accept range headers (Linn) until I know how to handle them
    but don't send this header as the MPD player ignores the "none" value anyway and uses ranges
    headers.push(Header::from_bytes(&b"Accept-Ranges"[..], &b"none"[..]).unwrap()); */

    headers
}
