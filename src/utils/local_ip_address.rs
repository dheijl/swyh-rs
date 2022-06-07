use if_addrs::IfAddr;
use std::net::{IpAddr, UdpSocket};

/// get_local_address - get the local ip address, return an `Option<String>`. when it fails, return `None`.
pub fn get_local_addr() -> Option<IpAddr> {
    // bind to IN_ADDR_ANY, can be multiple interfaces/addresses
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };
    // try to connect to Google DNS so that we bind to an interface connected to the internet
    match socket.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };
    // now we can return the IP address of this interface
    match socket.local_addr() {
        Ok(addr) => Some(addr.ip()),
        Err(_) => None,
    }
}

pub fn get_interfaces() -> Vec<String> {
    let mut interfaces: Vec<String> = Vec::new();
    let ifaces = if_addrs::get_if_addrs().expect("could not get interfaces");
    for iface in ifaces {
        if let IfAddr::V4(ref _if4_addr) = iface.addr.clone() {
            interfaces.push(iface.addr.ip().to_string())
        }
    }
    interfaces
}
