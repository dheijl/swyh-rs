use if_addrs::IfAddr;
use std::net::{IpAddr, UdpSocket};

/// `get_local_address` - get the local ip address, return an `Option<String>`. when it fails, return `None`.
#[must_use]
pub fn get_local_addr() -> Option<IpAddr> {
    // bind to IN_ADDR_ANY, can be multiple interfaces/addresses
    // try to connect to Google DNS so that we bind to an interface connected to the internet
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return None;
    };
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

#[must_use]
pub fn get_interfaces() -> Vec<String> {
    let mut interfaces: Vec<String> = Vec::new();
    let ifaces = if_addrs::get_if_addrs().expect("could not get interfaces");
    ifaces
        .iter()
        .filter(|iface| matches!(iface.addr, IfAddr::V4(..)))
        .for_each(|iface| interfaces.push(iface.addr.ip().to_string()));
    interfaces
}
