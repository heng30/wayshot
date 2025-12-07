use std::{
    fs::File,
    io::Read,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
};
use stunclient::StunClient;

fn main() {
    let mut f = File::open("data/stunserverlist.txt").unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();

    // end with `0.0.0.0:0` addr is not accessable
    buf.lines().for_each(|stun_server| {
        let wan_ip = get_public_address(stun_server);
        println!("\t{}", wan_ip);
    });
}

fn get_public_address(stun_server: &str) -> SocketAddr {
    let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let udp = UdpSocket::bind(local_addr).unwrap();

    print!("{}\t{}", stun_server, udp.local_addr().unwrap());

    let stun_addr = match stun_server.to_socket_addrs() {
        Ok(v) => v,
        Err(_) => return local_addr,
    }
    .filter(|x| x.is_ipv4())
    .next()
    .unwrap();

    let stun_client = StunClient::new(stun_addr);
    match stun_client.query_external_address(&udp) {
        Ok(external_address) => external_address,
        Err(_) => local_addr,
    }
}
