use std::net::SocketAddr;
use std::net::UdpSocket;
use std::time::{Duration, Instant};
use stringreader::StringReader;
use xml::reader::{EventReader, XmlEvent};

macro_rules! DEBUG {
    ($x:stmt) => {
        if cfg!(debug_assertions) {
            $x
        }
    };
}

#[derive(Debug, Clone)]
pub struct AvService {
    service_id: String,
    service_type: String,
    control_url: String,
}

impl AvService {
    fn new() -> AvService {
        AvService {
            service_id: String::new(),
            service_type: String::new(),
            control_url: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Renderer {
    pub dev_name: String,
    pub dev_model: String,
    pub dev_type: String,
    pub dev_url: String,
    pub svc_type: String,
    pub svc_id: String,
    pub pl_control_url: String,
    pub vol_control_url: String,
    pub services: Vec<AvService>,
}

impl Renderer {
    fn new() -> Renderer {
        Renderer {
            dev_name: String::new(),
            dev_url: String::new(),
            dev_model: String::new(),
            dev_type: String::new(),
            vol_control_url: String::new(),
            pl_control_url: String::new(),
            svc_id: String::new(),
            svc_type: String::new(),
            services: Vec::new(),
        }
    }
}

// SSDP search for media renderers with a 3.0 second MX response time
static SSDP_DISCOVER_MSG: &str = "M-SEARCH * HTTP/1.1\r\n\
Host: 239.255.255.250:1900\r\n\
Man: \"ssdp:discover\"\r\n\
ST: urn:schemas-upnp-org:service:RenderingControl:1\r\n\
MX: 3\r\n\r\n";

pub fn discover(logger: &dyn Fn(String)) -> Option<Vec<Renderer>> {
    logger(format!("SSDP discovery started"));

    // get the address of the internet connected interface
    let any: SocketAddr = ([0, 0, 0, 0], 0).into();
    let socket = UdpSocket::bind(any).expect("Could not bind the UDP socket to INADDR_ANY");
    let googledns: SocketAddr = ([8, 8, 8, 8], 80).into();
    socket.connect(googledns).expect("No network connectivity");
    let bind_addr = socket
        .local_addr()
        .expect("Could not obtain local ip address for udp broadcast socket");

    //  SSDP UDP broadcast address
    let broadcast_address: SocketAddr = ([239, 255, 255, 250], 1900).into();
    let socket = UdpSocket::bind(&bind_addr).unwrap();
    let _ = socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let _ = socket.set_broadcast(true).unwrap();

    // broadcast the M-SEARCH message (MX is 3 secs)
    socket
        .send_to(SSDP_DISCOVER_MSG.as_bytes(), &broadcast_address)
        .unwrap();

    // collect the responses and remeber all renderers
    let mut devices: Vec<String> = Vec::new();
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        // keep capturing responses for 3.1 seconds
        if duration > Duration::from_millis(3100) {
            break;
        }
        let mut buf: [u8; 2048] = [0; 2048];
        let resp: String;
        match socket.recv(&mut buf) {
            Ok(received) => {
                resp = std::str::from_utf8(&buf[0..received]).unwrap().to_string();
                DEBUG!(eprintln!(
                    "UDP response at {}: \r\n{}",
                    start.elapsed().as_millis(),
                    resp
                ));
                let response: Vec<&str> = resp.split("\r\n").collect();
                if response.len() > 0 {
                    let status_code = response[0]
                        .trim_start_matches("HTTP/1.1 ")
                        .chars()
                        .take_while(|x| x.is_numeric())
                        .collect::<String>()
                        .parse::<u32>()
                        .unwrap_or(0);

                    if status_code != 200 {
                        continue; // ignore
                    }

                    let iter = response.iter().filter_map(|l| {
                        let mut split = l.splitn(2, ':');
                        match (split.next(), split.next()) {
                            (Some(header), Some(value)) => Some((header, value.trim())),
                            _ => None,
                        }
                    });
                    let mut is_renderer = false;
                    let mut dev_url: String = String::new();
                    for (header, value) in iter {
                        if header == "LOCATION" {
                            dev_url = value.to_string();
                        } else if header == "ST" && value.contains("RenderingControl") {
                            is_renderer = true;
                        }
                    }
                    if is_renderer {
                        logger(format!("Renderer at : {}", dev_url.clone()));
                        devices.push(dev_url);
                    }
                } else {
                    continue;
                }
            }
            Err(_e) => {}
        }
    }

    logger(format!("Getting renderer descriptions"));
    let mut renderers: Vec<Renderer> = Vec::new();

    for dev in devices {
        //let mut rend = Renderer:: new();
        //rend.dev_url = dev.clone();
        match get_service_description(&dev) {
            Some(xml) => match get_renderer(&xml) {
                Some(rend) => {
                    renderers.push(rend);
                }
                None => {}
            },
            None => {}
        }
    }

    for r in renderers.iter() {
        logger(format!(
            "Renderer {} {} {} has {} services",
            r.dev_name,
            r.dev_model,
            r.dev_url,
            r.services.len()
        ));
        for s in r.services.iter() {
            DEBUG!(eprintln!(
                ".. {} {} {}",
                s.service_type, s.service_id, s.control_url
            ));
        }
    }
    Some(renderers)
}

/// get_service_description - get the upnp service description xml for a media renderer
fn get_service_description(dev_url: &String) -> Option<String> {
    DEBUG!(eprintln!("Get service description for {}", dev_url.clone()));
    let url = dev_url.clone();
    let resp = ureq::get(url.as_str())
        .set("User-Agent", "swyh-rs-Rust")
        .set("Content-Type", "text/xml")
        .send_string("");
    if resp.error() {
        return None;
    }
    let descr_xml = resp.into_string().unwrap_or_default();
    DEBUG!(eprintln!("Service description: \r\n{}", descr_xml));
    if !descr_xml.is_empty() {
        Some(descr_xml)
    } else {
        None
    }
}

fn get_renderer(xml: &String) -> Option<Renderer> {
    let xmlstream = StringReader::new(&xml);
    let parser = EventReader::new(xmlstream);
    let mut cur_elem = String::new();
    let mut service = AvService::new();
    let mut renderer = Renderer::new();
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                cur_elem = name.local_name;
            }
            Ok(XmlEvent::EndElement { name }) => {
                let end_elem = name.local_name;
                if end_elem == "service" {
                    if service.service_id.contains("Playlist") {
                        renderer.pl_control_url = service.control_url.clone();
                    } else if service.service_id.contains("Volume") {
                        renderer.vol_control_url = service.control_url.clone();
                    }
                    renderer.services.push(service);
                    service = AvService::new();
                }
            }
            Ok(XmlEvent::Characters(value)) => {
                if cur_elem.contains("serviceType") {
                    service.service_type = value;
                } else if cur_elem.contains("serviceId") {
                    service.service_id = value;
                } else if cur_elem.contains("controlURL") {
                    service.control_url = value;
                } else if cur_elem.contains("modelName") {
                    renderer.dev_model = value;
                } else if cur_elem.contains("friendlyName") {
                    renderer.dev_name = value;
                } else if cur_elem.contains("deviceType") {
                    renderer.dev_type = value;
                } else if cur_elem.contains("URLBase") {
                    renderer.dev_url = value;
                }
            }
            Err(e) => {
                DEBUG!(eprintln!("Error: {}", e));
                return None;
            }
            _ => {}
        }
    }

    Some(renderer)
}
