use std::net::UdpSocket;
use std::time::Duration;

fn le_u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

fn main() {
    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    let server = "127.0.0.1:16666";
    let username = "test";
    let client_session_id: u32 = 0x11223344;

    // SERVER_AUTH_NEGOTIATE (0xD003CA01)
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&le_u32(0xD003CA01));
    pkt.push(2); // protocol version
    pkt.extend_from_slice(&le_u32(client_session_id));
    pkt.extend_from_slice(username.as_bytes());
    pkt.push(0);

    eprintln!("sending negotiate to {server}...");
    sock.send_to(&pkt, server).expect("send_to");

    let mut buf = [0u8; 2048];
    let (n, peer) = sock.recv_from(&mut buf).expect("recv_from");
    eprintln!("got {n} bytes from {peer}");

    // Print first few bytes.
    for (i, b) in buf[..n].iter().take(32).enumerate() {
        eprint!("{:02x}{}", b, if (i + 1) % 2 == 0 { " " } else { "" });
    }
    eprintln!();
}
