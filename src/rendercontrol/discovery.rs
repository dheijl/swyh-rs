//! SSDP discovery of DLNA/OpenHome renderers on the network, and parsing of
//! their UPnP `GetDescription.xml` into [`Renderer`] values.

use super::types::{AvService, Renderer, SupportedProtocols};
use crate::{
    fl,
    globals::statics::{APP_VERSION, get_config},
    utils::ui_logger::{LogCategory, ui_log},
};
use ecow::EcoString;
use hashbrown::{HashMap, HashSet};
use log::{debug, error, info};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
use xml::reader::{EventReader, ParserConfig, XmlEvent};

static AV_SCHEMA: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
static AV_DEVICE: &str = AV_SCHEMA;
static OH_SCHEMA: &str = "urn:av-openhome-org:service:Product:1";
static OH_DEVICE: &str = OH_SCHEMA;

/// the relevant info extracted from an SSDP response
struct SsdpResponse {
    status_code: u32,
    location: String,
    is_av: bool,
    is_oh: bool,
}

/// parse the the relevant headers from the SSDP HTTP response
/// into an SsdpResponse struct
fn parse_ssdp_response(resp: &str) -> SsdpResponse {
    let mut lines = resp.lines();
    let status_code = lines
        .next()
        .unwrap_or("")
        .trim_start_matches("HTTP/1.1 ")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse::<u32>()
        .unwrap_or(0);
    let mut location = String::new();
    let mut is_av = false;
    let mut is_oh = false;
    lines
        .filter_map(|l| {
            let mut split = l.splitn(2, ':');
            match (split.next(), split.next()) {
                (Some(header), Some(value)) => Some((header, value.trim())),
                _ => None,
            }
        })
        .for_each(
            |(header, value)| match header.to_ascii_uppercase().as_str() {
                "LOCATION" => location = value.to_string(),
                "ST" => {
                    if value.contains(AV_SCHEMA) {
                        is_av = true;
                    } else if value.contains(OH_SCHEMA) {
                        is_oh = true;
                    }
                }
                _ => (),
            },
        );
    SsdpResponse {
        status_code,
        location,
        is_av,
        is_oh,
    }
}

/// Send SSDP M-SEARCH messages and collect AV/OH renderer responses.
///
/// Returns a deduplicated list of `(location_url, socket_addr)` pairs,
/// preferring OpenHome entries when a device advertises both protocols.
/// Returns `None` if the UDP socket cannot be created or bound.
fn ssdp_search(local_addr: IpAddr) -> Option<Vec<(String, SocketAddr)>> {
    const DEFAULT_SEARCH_TTL: u32 = 2;
    static SSDP_DISCOVER_MSG: &str = "M-SEARCH * HTTP/1.1\r\n\
Host: 239.255.255.250:1900\r\n\
Man: \"ssdp:discover\"\r\n\
ST: {device_type}\r\n\
MX: 3\r\n\r\n";

    let bind_addr = SocketAddr::new(local_addr, 0);
    let sock2 = if let Ok(s) = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
        s
    } else {
        ui_log(LogCategory::Error, &fl!("err-ssdp-bind"));
        return None;
    };
    if sock2.bind(&bind_addr.into()).is_err() {
        ui_log(LogCategory::Error, &fl!("err-ssdp-bind"));
        return None;
    }
    if sock2.set_broadcast(true).is_err() {
        ui_log(LogCategory::Error, &fl!("err-ssdp-broadcast"));
    }
    if sock2.set_multicast_ttl_v4(DEFAULT_SEARCH_TTL).is_err() {
        ui_log(LogCategory::Error, &fl!("err-ssdp-ttl"));
    }
    // On macOS, binding to a specific IP does not automatically route outgoing
    // multicast through the correct interface; IP_MULTICAST_IF must be set explicitly.
    if let IpAddr::V4(local_v4) = local_addr
        && sock2.set_multicast_if_v4(&local_v4).is_err()
    {
        ui_log(LogCategory::Error, &fl!("err-ssdp-ttl"));
    }
    let socket: UdpSocket = sock2.into();
    let broadcast_address: SocketAddr = ([239, 255, 255, 250], 1900).into();
    let mut oh_devices: Vec<(String, SocketAddr)> = Vec::new();
    let mut av_devices: Vec<(String, SocketAddr)> = Vec::new();

    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", OH_DEVICE);
    if socket.send_to(msg.as_bytes(), broadcast_address).is_err() {
        ui_log(LogCategory::Error, &fl!("err-ssdp-oh-send"));
    }
    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", AV_DEVICE);
    if socket.send_to(msg.as_bytes(), broadcast_address).is_err() {
        ui_log(LogCategory::Error, &fl!("err-ssdp-av-send"));
    }

    let start = Instant::now();
    loop {
        let duration = start.elapsed().as_millis() as u64;
        if duration >= 3100 {
            break;
        }
        let max_wait_time = 3100 - duration;
        socket
            .set_read_timeout(Some(Duration::from_millis(max_wait_time)))
            .ok();
        let mut buf: [u8; 2048] = [0; 2048];
        match socket.recv_from(&mut buf) {
            Ok((received, from)) => {
                let resp = String::from_utf8_lossy(&buf[0..received]).to_string();
                debug!(
                    "SSDP: HTTP response at {} from {}: \r\n{}",
                    start.elapsed().as_millis(),
                    from,
                    resp
                );
                let parsed = parse_ssdp_response(&resp);
                if parsed.status_code != 200 {
                    error!("SSDP: HTTP error response status={}", parsed.status_code);
                    continue;
                }
                if !parsed.location.is_empty() {
                    if parsed.is_av {
                        av_devices.push((parsed.location.clone(), from));
                        debug!("SSDP Discovery: AV renderer: {}", parsed.location);
                    } else if parsed.is_oh {
                        oh_devices.push((parsed.location.clone(), from));
                        debug!("SSDP Discovery: OH renderer: {}", parsed.location);
                    }
                }
            }
            Err(e) => {
                // ignore socket read timeout on Windows or EAGAIN/EWOULBLOCK on Linux/Unix/MacOS
                let error_text = e.to_string();
                let to_ignore = ["10060", "os error 11", "os error 35"]
                    .iter()
                    .any(|s| error_text.contains(*s));
                if !to_ignore {
                    ui_log(
                        LogCategory::Error,
                        &format!("Error reading SSDP M-SEARCH response: {e}"),
                    );
                }
            }
        }
    }

    // only keep OH devices and AV devices that are not also OH capable
    let oh_locations: HashSet<String> = oh_devices.iter().map(|(l, _)| l.clone()).collect();
    let mut usable: Vec<(String, SocketAddr)> =
        Vec::with_capacity(oh_devices.len() + av_devices.len());
    usable.extend(oh_devices);
    for (av_location, sa) in av_devices {
        if oh_locations.contains(av_location.as_str()) {
            debug!("SSDP Discovery: skipping AV renderer {av_location} as it is also OH");
        } else {
            usable.push((av_location, sa));
        }
    }
    Some(usable)
}

/// Build the shared HTTP [`ureq::Agent`] used for renderer discovery and control.
///
/// `ureq`'s default configuration leaves every request timeout unset (`connect`,
/// `recv_response` and `global` are all `None`), so a renderer that accepts the
/// TCP connection but never sends back a complete response would block the
/// request forever. Because [`discover`] waits for all per-renderer fetches to
/// finish before returning, a single such renderer would stall the whole
/// discovery cycle; and since the renderer control calls (`play`/`stop_play`/
/// `set_volume`) reuse this same agent, an unresponsive renderer could likewise
/// block their caller. Setting explicit timeouts bounds every request, so a
/// stalled renderer is skipped for that cycle and retried on the next one.
#[must_use]
pub fn new_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .timeout_connect(Some(Duration::from_secs(4)))
            .build(),
    )
}

//
// SSDP UPNP service discovery
//
// returns a list of all AVTransport DLNA and Openhome rendering devices
//
pub fn discover(agent: &ureq::Agent, rmap: &HashMap<String, Renderer>) -> Option<Vec<Renderer>> {
    debug!("SSDP discovery started");

    // get the address of the selected interface
    let ip = if let Some(s) = get_config().last_network.clone() {
        s
    } else {
        ui_log(LogCategory::Error, &fl!("err-ssdp-no-network"));
        return None;
    };
    info!("running SSDP on {ip}");
    let local_addr: IpAddr = if let Ok(addr) = ip.parse() {
        addr
    } else {
        ui_log(LogCategory::Error, &fl!("err-ssdp-parse-ip"));
        return None;
    };
    // build a hashset of the devices we have discovered previously
    let known_locations: HashSet<&str> = rmap.values().map(|r| r.location.as_str()).collect();
    // run the SSDP search and filter out the devices we already know about
    let devices: Vec<(String, SocketAddr)> = ssdp_search(local_addr)?
        .into_iter()
        .filter(|(location, _)| {
            if known_locations.contains(location.as_str()) {
                info!("SSDP discovery: Skipping known Renderer at {location}");
                false
            } else {
                info!("SSDP discovery: new Renderer found at : {location}");
                true
            }
        })
        .collect();

    // now get the new renderers description xml, fetched concurrently since each
    // is an independent blocking HTTP round-trip to a different renderer
    debug!("Getting new renderer descriptions");
    let renderers: Vec<Renderer> = std::thread::scope(|scope| {
        let handles: Vec<_> = devices
            .into_iter()
            .map(|(location, from)| {
                scope.spawn(move || {
                    let xml = get_service_description(agent, &location)?;
                    let mut rend = get_renderer(agent, &xml)?;
                    rend.location.clone_from(&location);
                    rend.remote_addr = from.ip().to_string();
                    // check for an absent URLBase in the description
                    // or devices like Yamaha WXAD-10 with bad URLBase port number
                    if rend.dev_url.is_empty() || !location.contains(&rend.dev_url) {
                        let mut url_base = location;
                        if url_base.contains("http://") {
                            url_base = url_base["http://".to_string().len()..].to_string();
                            let pos = url_base.find('/').unwrap_or_default();
                            if pos > 0 {
                                url_base = url_base[0..pos].to_string();
                            }
                        }
                        rend.dev_url = format!("http://{url_base}/");
                    }
                    rend.parse_url();
                    rend.get_volume();
                    Some(rend)
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().unwrap_or(None))
            .collect()
    });

    #[cfg(debug_assertions)]
    {
        for r in &renderers {
            debug!(
                "Renderer {} {} ip {} at location {} has {} services",
                r.dev_name,
                r.dev_model,
                r.remote_addr,
                r.location,
                r.services.len()
            );
            debug!(
                "  => OpenHome Playlist control url: '{}', AvTransport url: '{}'",
                r.oh_control_url, r.av_control_url
            );
            for s in &r.services {
                debug!(".. {} {} {}", s.service_type, s.service_id, s.control_url);
            }
        }
        debug!("SSDP discovery complete");
    }
    Some(renderers)
}

/// `get_service_description`
/// get the upnp service description xml for a media renderer
fn get_service_description(agent: &ureq::Agent, location: &str) -> Option<String> {
    debug!("Get service description for {location}");
    match agent
        .get(location)
        .header("User-Agent", format!("swyh-rs/{APP_VERSION}"))
        .header("Content-Type", "text/xml")
        .call()
    {
        Ok(mut resp) => {
            let descr_xml = resp.body_mut().read_to_string().unwrap_or_default();
            debug!("Service description:");
            debug!("{descr_xml}");
            if descr_xml.is_empty() {
                None
            } else {
                Some(descr_xml)
            }
        }
        Err(e) => {
            error!("Error {e} getting service description for {location}");
            None
        }
    }
}

/// build a renderer struct by (roughly) parsing the GetDescription.xml
fn get_renderer(agent: &ureq::Agent, xml: &str) -> Option<Renderer> {
    let parser =
        EventReader::new_with_config(xml.as_bytes(), ParserConfig::new().trim_whitespace(true));
    let mut cur_elem = EcoString::new();
    let mut service = AvService::new();
    let mut renderer = Renderer::new(agent);
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                cur_elem = EcoString::from(name.local_name);
            }
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name == "service" {
                    let id = service.service_id.as_str();
                    if id.contains("urn:av-openhome-org:service") {
                        if id.contains("Playlist") {
                            renderer.oh_control_url.clone_from(&service.control_url);
                            renderer.supported_protocols |= SupportedProtocols::OPENHOME;
                        } else if id.contains("Volume") {
                            renderer.oh_volume_url.clone_from(&service.control_url);
                        }
                    } else if id.contains(":AVTransport") {
                        renderer.av_control_url.clone_from(&service.control_url);
                        renderer.supported_protocols |= SupportedProtocols::AVTRANSPORT;
                    } else if id.contains(":RenderingControl") {
                        renderer.av_volume_url.clone_from(&service.control_url);
                    }
                    renderer.services.push(service);
                    service = AvService::new();
                }
            }
            Ok(XmlEvent::Characters(value)) => match cur_elem.as_str() {
                "serviceType" => service.service_type = value,
                "serviceId" => service.service_id = value,
                "modelName" => renderer.dev_model = value,
                "friendlyName" => renderer.dev_name = value,
                "deviceType" => renderer.dev_type = value,
                "URLBase" => renderer.dev_url = value,
                "controlURL" => service.control_url = normalize_url(&value),
                _ => {}
            },

            Err(e) => {
                error!("SSDP Get Renderer Description Error: {e}");
                return None;
            }
            _ => {}
        }
    }
    //debug!("{:?}", renderer);
    Some(renderer)
}

/// sometimes the control url is not prefixed with a '/'
fn normalize_url(value: &str) -> String {
    if value.is_empty() || value.starts_with('/') {
        value.to_owned()
    } else {
        '/'.to_string() + value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let ok_errors = ["10060", "os error 11", "os error 35"];
        let mut e = "bla bla os error 11 bla bla";
        let to_ignore = ok_errors.iter().any(|s| e.contains(*s));
        assert!(to_ignore);
        e = "bla bla os error 12 bla bla";
        let to_ignore = ok_errors.iter().any(|s| e.contains(*s));
        assert!(!to_ignore);
    }

    #[test]
    fn test_normalize() {
        let mut url = "/ctl".to_string();
        assert!(normalize_url(&url) == *"/ctl");
        url = "ctl".to_string();
        assert!(normalize_url(&url) == *"/ctl");
        url = String::new();
        assert!(normalize_url(&url) == url);
    }

    #[test]
    fn test_bubble() {
        static BUBBLE_SSDP: &str = "HTTP/1.1 200 OK\r\n\
Ext:\r\n\
St: urn:schemas-upnp-org:service:RenderingControl:1\r\n\
Server: Linux/6.8.4-3-pve UPnP/1.0 BubbleUPnPServer/0.9-update49\r\n\
Usn: uuid:e8dbf26b-de8f-4c96-0000-0000002ea642::urn:schemas-upnp-org:service:RenderingControl:1\r\n\
Cache-control: max-age=1800\r\n\
Location: http://192.168.1.181:33065/dev/e8dbf26b-de8f-4c96-0000-0000002ea642/desc.xml\r\n";
        let parsed = parse_ssdp_response(BUBBLE_SSDP);
        assert_eq!(parsed.status_code, 200);
        assert!(!parsed.location.is_empty());
        assert!(parsed.is_av);
        assert!(!parsed.is_oh);
    }

    #[test]
    fn new_agent_configures_request_timeouts() {
        // Fast always-on guard: the shared agent must carry explicit timeouts
        // so a renderer that accepts a connection but never responds can't block
        // a request — and therefore the whole discovery cycle — indefinitely.
        // This asserts the timeouts are *set*; `unresponsive_renderer_request_times_out`
        // (ignored, ~10s) proves they actually *fire* end-to-end.
        let timeouts = new_agent().config().timeouts();
        assert_eq!(timeouts.global, Some(Duration::from_secs(10)));
        assert_eq!(timeouts.connect, Some(Duration::from_secs(4)));
    }

    #[test]
    #[ignore = "~10s: drives the real global request timeout against a stalled peer"]
    fn unresponsive_renderer_request_times_out() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::time::Instant;
        // A stand-in "renderer" that accepts the TCP connection, reads the
        // request, then holds the socket open forever without ever replying —
        // the exact failure mode that used to hang discovery indefinitely.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut s = stream.unwrap();
                let _ = s.read(&mut [0u8; 1024]);
                std::thread::sleep(Duration::from_secs(3600));
            }
        });
        let start = Instant::now();
        let result = new_agent().get(format!("http://{addr}/desc.xml")).call();
        let elapsed = start.elapsed();
        assert!(result.is_err(), "expected a timeout error, got Ok");
        assert!(
            elapsed < Duration::from_secs(20),
            "request did not time out; it hung for {elapsed:?}"
        );
    }
}
