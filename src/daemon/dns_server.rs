use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Result;
use simple_dns::rdata::{A, AAAA, RData};
use simple_dns::{CLASS, Name, Packet, PacketFlag, QTYPE, Question, RCODE, ResourceRecord, TYPE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use crate::infrastructure::logging::LogFile;

/// Resolves .roxy domains to the configured LAN IP.
#[derive(Clone)]
pub struct IpResolver {
    lan_ip: Ipv4Addr,
}

impl IpResolver {
    pub fn new(lan_ip: Ipv4Addr) -> Self {
        Self { lan_ip }
    }

    /// Returns the LAN IP for all queries.
    pub fn resolve(&self) -> Ipv4Addr {
        self.lan_ip
    }
}

pub struct DnsServer {
    port: u16,
    ttl: u32,
    ip_resolver: Arc<IpResolver>,
}

impl DnsServer {
    pub fn new(port: u16, lan_ip: Ipv4Addr) -> Self {
        Self {
            port,
            ttl: 1,
            ip_resolver: Arc::new(IpResolver::new(lan_ip)),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let log = LogFile::new();

        // Bind to all interfaces so Docker containers can reach us directly
        let ipv4_addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let ipv6_addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, self.port));

        // Bind UDP sockets
        let udp_v4 = UdpSocket::bind(ipv4_addr).await?;
        let udp_v6 = UdpSocket::bind(ipv6_addr).await?;

        // Bind TCP listeners
        let tcp_v4 = TcpListener::bind(ipv4_addr).await?;
        let tcp_v6 = TcpListener::bind(ipv6_addr).await?;

        let _ = log.log(&format!("DNS server listening on {} (UDP/TCP)", ipv4_addr));
        let _ = log.log(&format!("DNS server listening on {} (UDP/TCP)", ipv6_addr));
        let _ = log.log(&format!(
            "DNS resolving all .roxy domains to {}",
            self.ip_resolver.lan_ip
        ));
        println!("DNS server listening on {} (UDP/TCP)", ipv4_addr);
        println!("DNS server listening on {} (UDP/TCP)", ipv6_addr);
        println!(
            "DNS resolving all .roxy domains to {}",
            self.ip_resolver.lan_ip
        );

        let ttl = self.ttl;
        let resolver = self.ip_resolver.clone();

        tokio::select! {
            r = serve_udp(udp_v4, ttl, resolver.clone()) => r,
            r = serve_udp(udp_v6, ttl, resolver.clone()) => r,
            r = serve_tcp(tcp_v4, ttl, resolver.clone()) => r,
            r = serve_tcp(tcp_v6, ttl, resolver) => r,
        }
    }
}

async fn serve_udp(socket: UdpSocket, ttl: u32, resolver: Arc<IpResolver>) -> Result<()> {
    let mut buf = [0u8; 512]; // Standard DNS UDP size
    let response_ip = resolver.resolve();

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let response = handle_query(&buf[..len], ttl, response_ip);
        let _ = socket.send_to(&response, addr).await;
    }
}

async fn serve_tcp(listener: TcpListener, ttl: u32, resolver: Arc<IpResolver>) -> Result<()> {
    let response_ip = resolver.resolve();

    loop {
        let (stream, _addr) = listener.accept().await?;
        tokio::spawn(handle_tcp_connection(stream, ttl, response_ip));
    }
}

async fn handle_tcp_connection(
    mut stream: TcpStream,
    ttl: u32,
    response_ip: Ipv4Addr,
) -> Result<()> {
    // TCP DNS uses 2-byte length prefix
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;

    let mut query_buf = vec![0u8; len];
    stream.read_exact(&mut query_buf).await?;

    let response = handle_query(&query_buf, ttl, response_ip);

    // Send response with length prefix
    let resp_len = (response.len() as u16).to_be_bytes();
    stream.write_all(&resp_len).await?;
    stream.write_all(&response).await?;

    Ok(())
}

fn handle_query(query: &[u8], ttl: u32, response_ip: Ipv4Addr) -> Vec<u8> {
    // Parse incoming query
    let packet = match Packet::parse(query) {
        Ok(p) => p,
        Err(_) => return build_format_error(query),
    };

    let question = match packet.questions.first() {
        Some(q) => q,
        None => return build_format_error(query),
    };

    let domain = question.qname.to_string().to_lowercase();

    // Check if domain ends with .roxy
    if !domain.trim_end_matches('.').ends_with(".roxy") {
        return build_refused_response(&packet);
    }

    // Build response based on query type
    match question.qtype {
        QTYPE::TYPE(TYPE::A) => build_a_response(&packet, question, ttl, response_ip),
        QTYPE::TYPE(TYPE::AAAA) => build_aaaa_response(&packet, question, ttl),
        QTYPE::ANY => build_any_response(&packet, question, ttl, response_ip),
        _ => build_empty_response(&packet),
    }
}

fn build_format_error(query: &[u8]) -> Vec<u8> {
    // Try to extract transaction ID from query
    let id = if query.len() >= 2 {
        u16::from_be_bytes([query[0], query[1]])
    } else {
        0
    };

    let mut response = Packet::new_reply(id);
    response.set_flags(PacketFlag::RESPONSE | PacketFlag::RECURSION_DESIRED);
    *response.rcode_mut() = RCODE::FormatError;

    response.build_bytes_vec().unwrap_or_default()
}

fn build_refused_response(packet: &Packet) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(PacketFlag::RESPONSE | PacketFlag::RECURSION_DESIRED);
    *response.rcode_mut() = RCODE::Refused;

    response.build_bytes_vec().unwrap_or_default()
}

fn build_empty_response(packet: &Packet) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Clone questions into the response
    for q in &packet.questions {
        response.questions.push(q.clone());
    }

    response.build_bytes_vec().unwrap_or_default()
}

fn build_a_response(packet: &Packet, question: &Question, ttl: u32, ip: Ipv4Addr) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add A record
    let name_str = question.qname.to_string();
    let name = Name::new_unchecked(&name_str);
    let a_record = A::from(ip);
    let record = ResourceRecord::new(name, CLASS::IN, ttl, RData::A(a_record));
    response.answers.push(record);

    response.build_bytes_vec().unwrap_or_default()
}

fn build_aaaa_response(packet: &Packet, question: &Question, ttl: u32) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add AAAA record: ::1
    let name_str = question.qname.to_string();
    let name = Name::new_unchecked(&name_str);
    let aaaa_record = AAAA::from(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    let record = ResourceRecord::new(name, CLASS::IN, ttl, RData::AAAA(aaaa_record));
    response.answers.push(record);

    response.build_bytes_vec().unwrap_or_default()
}

fn build_any_response(packet: &Packet, question: &Question, ttl: u32, ip: Ipv4Addr) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add A record
    let name_str = question.qname.to_string();
    let name_a = Name::new_unchecked(&name_str);
    let a_record = A::from(ip);
    response.answers.push(ResourceRecord::new(
        name_a,
        CLASS::IN,
        ttl,
        RData::A(a_record),
    ));

    // Add AAAA record
    let name_aaaa = Name::new_unchecked(&name_str);
    let aaaa_record = AAAA::from(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    response.answers.push(ResourceRecord::new(
        name_aaaa,
        CLASS::IN,
        ttl,
        RData::AAAA(aaaa_record),
    ));

    response.build_bytes_vec().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 100);

    fn test_resolver() -> IpResolver {
        IpResolver::new(TEST_IP)
    }

    #[test]
    fn test_ip_resolver_returns_configured_ip() {
        let resolver = test_resolver();
        assert_eq!(resolver.resolve(), TEST_IP);
    }

    #[test]
    fn test_query_handler_roxy_domain() {
        // Build a simple A query for test.roxy
        let mut packet = Packet::new_query(1234);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1, TEST_IP);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);
    }

    #[test]
    fn test_query_handler_non_roxy_domain() {
        // Build a simple A query for google.com
        let mut packet = Packet::new_query(1234);
        let name = Name::new_unchecked("google.com");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1, TEST_IP);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::Refused);
        assert_eq!(parsed.answers.len(), 0);
    }

    #[test]
    fn test_query_handler_aaaa() {
        let mut packet = Packet::new_query(5678);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::AAAA.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1, TEST_IP);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);
    }

    #[test]
    fn test_query_handler_subdomain() {
        // Test nested subdomains like app.test.roxy
        let mut packet = Packet::new_query(9999);
        let name = Name::new_unchecked("app.test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1, TEST_IP);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);
    }

    #[test]
    fn test_a_response_uses_configured_ip() {
        let mut packet = Packet::new_query(1111);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let custom_ip = Ipv4Addr::new(10, 0, 0, 50);
        let response = handle_query(&query, 1, custom_ip);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);

        // Verify the IP in the response
        if let RData::A(a_record) = &parsed.answers[0].rdata {
            assert_eq!(*a_record, A::from(custom_ip));
        } else {
            panic!("Expected A record");
        }
    }
}
