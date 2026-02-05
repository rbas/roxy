use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Result;
use simple_dns::rdata::{RData, A, AAAA};
use simple_dns::{Name, Packet, PacketFlag, Question, ResourceRecord, CLASS, QTYPE, RCODE, TYPE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use crate::infrastructure::logging::LogFile;

/// Docker Desktop for Mac's host gateway IP
const DOCKER_HOST_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 65, 254);

/// Determines the appropriate response IP based on the source of the DNS query.
/// This allows Docker containers to get the Docker host IP while regular
/// host applications get the LAN IP.
#[derive(Clone)]
pub struct IpResolver {
    lan_ip: Ipv4Addr,
    docker_host_ip: Ipv4Addr,
}

impl IpResolver {
    pub fn new(lan_ip: Ipv4Addr) -> Self {
        Self {
            lan_ip,
            docker_host_ip: DOCKER_HOST_IP,
        }
    }

    /// Returns the appropriate IP(s) based on the source address of the query.
    pub fn resolve_for_source(&self, source: IpAddr) -> Vec<Ipv4Addr> {
        match source {
            // Localhost queries - from host browser/apps
            IpAddr::V4(ip) if ip.is_loopback() => vec![self.lan_ip],
            IpAddr::V6(ip) if ip.is_loopback() => vec![self.lan_ip],

            // Docker network queries (172.x.x.x is Docker's default bridge)
            IpAddr::V4(ip) if Self::is_docker_network(ip) => vec![self.docker_host_ip],

            // LAN queries - from other devices on the network
            IpAddr::V4(ip) if ip.is_private() => vec![self.lan_ip],

            // Unknown source - return both IPs as fallback
            _ => vec![self.docker_host_ip, self.lan_ip],
        }
    }

    /// Checks if an IP is from Docker's typical network ranges
    fn is_docker_network(ip: Ipv4Addr) -> bool {
        let octets = ip.octets();
        // Docker default bridge: 172.17.0.0/16
        // Docker custom networks: 172.18-31.0.0/16
        // Docker for Mac VM network: 192.168.65.0/24
        octets[0] == 172 && (17..=31).contains(&octets[1])
            || (octets[0] == 192 && octets[1] == 168 && octets[2] == 65)
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
            "DNS using source-based IP resolution (LAN: {}, Docker: {})",
            self.ip_resolver.lan_ip, self.ip_resolver.docker_host_ip
        ));
        println!("DNS server listening on {} (UDP/TCP)", ipv4_addr);
        println!("DNS server listening on {} (UDP/TCP)", ipv6_addr);
        println!(
            "DNS using source-based IP resolution (LAN: {}, Docker: {})",
            self.ip_resolver.lan_ip, self.ip_resolver.docker_host_ip
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

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let response_ips = resolver.resolve_for_source(addr.ip());
        let response = handle_query(&buf[..len], ttl, &response_ips);
        let _ = socket.send_to(&response, addr).await;
    }
}

async fn serve_tcp(listener: TcpListener, ttl: u32, resolver: Arc<IpResolver>) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await?;
        let resolver = resolver.clone();
        tokio::spawn(handle_tcp_connection(stream, addr.ip(), ttl, resolver));
    }
}

async fn handle_tcp_connection(
    mut stream: TcpStream,
    source_ip: IpAddr,
    ttl: u32,
    resolver: Arc<IpResolver>,
) -> Result<()> {
    // TCP DNS uses 2-byte length prefix
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;

    let mut query_buf = vec![0u8; len];
    stream.read_exact(&mut query_buf).await?;

    let response_ips = resolver.resolve_for_source(source_ip);
    let response = handle_query(&query_buf, ttl, &response_ips);

    // Send response with length prefix
    let resp_len = (response.len() as u16).to_be_bytes();
    stream.write_all(&resp_len).await?;
    stream.write_all(&response).await?;

    Ok(())
}

fn handle_query(query: &[u8], ttl: u32, response_ips: &[Ipv4Addr]) -> Vec<u8> {
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
        QTYPE::TYPE(TYPE::A) => build_a_response(&packet, question, ttl, response_ips),
        QTYPE::TYPE(TYPE::AAAA) => build_aaaa_response(&packet, question, ttl),
        QTYPE::ANY => build_any_response(&packet, question, ttl, response_ips),
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

fn build_a_response(packet: &Packet, question: &Question, ttl: u32, ips: &[Ipv4Addr]) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add A records for all configured IPs
    let name_str = question.qname.to_string();
    for ip in ips {
        let name = Name::new_unchecked(&name_str);
        let a_record = A::from(*ip);
        let record = ResourceRecord::new(name, CLASS::IN, ttl, RData::A(a_record));
        response.answers.push(record);
    }

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

fn build_any_response(packet: &Packet, question: &Question, ttl: u32, ips: &[Ipv4Addr]) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add A records for all configured IPs
    let name_str = question.qname.to_string();
    for ip in ips {
        let name_a = Name::new_unchecked(&name_str);
        let a_record = A::from(*ip);
        response
            .answers
            .push(ResourceRecord::new(name_a, CLASS::IN, ttl, RData::A(a_record)));
    }

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

    fn test_ips() -> Vec<Ipv4Addr> {
        vec![Ipv4Addr::new(192, 168, 1, 100)]
    }

    fn test_resolver() -> IpResolver {
        IpResolver::new(Ipv4Addr::new(192, 168, 1, 100))
    }

    #[test]
    fn test_ip_resolver_localhost_returns_lan_ip() {
        let resolver = test_resolver();

        // IPv4 localhost
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(ips, vec![Ipv4Addr::new(192, 168, 1, 100)]);

        // IPv6 localhost
        let ips = resolver.resolve_for_source(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
        assert_eq!(ips, vec![Ipv4Addr::new(192, 168, 1, 100)]);
    }

    #[test]
    fn test_ip_resolver_docker_network_returns_docker_host_ip() {
        let resolver = test_resolver();

        // Docker default bridge (172.17.x.x)
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(172, 17, 0, 2)));
        assert_eq!(ips, vec![DOCKER_HOST_IP]);

        // Docker custom network (172.18.x.x)
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(172, 18, 0, 5)));
        assert_eq!(ips, vec![DOCKER_HOST_IP]);

        // Docker for Mac VM network (192.168.65.x)
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(192, 168, 65, 2)));
        assert_eq!(ips, vec![DOCKER_HOST_IP]);
    }

    #[test]
    fn test_ip_resolver_lan_returns_lan_ip() {
        let resolver = test_resolver();

        // Another device on LAN
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)));
        assert_eq!(ips, vec![Ipv4Addr::new(192, 168, 1, 100)]);

        // Different LAN subnet
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)));
        assert_eq!(ips, vec![Ipv4Addr::new(192, 168, 1, 100)]);
    }

    #[test]
    fn test_ip_resolver_unknown_returns_both() {
        let resolver = test_resolver();

        // Public IP (shouldn't happen in practice, but test fallback)
        let ips = resolver.resolve_for_source(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
        assert_eq!(ips.len(), 2);
        assert!(ips.contains(&DOCKER_HOST_IP));
        assert!(ips.contains(&Ipv4Addr::new(192, 168, 1, 100)));
    }

    #[test]
    fn test_query_handler_roxy_domain() {
        // Build a simple A query for test.roxy
        let mut packet = Packet::new_query(1234);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1, &test_ips());

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
        let response = handle_query(&query, 1, &test_ips());

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
        let response = handle_query(&query, 1, &test_ips());

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
        let response = handle_query(&query, 1, &test_ips());

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);
    }

    #[test]
    fn test_a_response_uses_configured_ips() {
        let mut packet = Packet::new_query(1111);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let custom_ips = vec![
            Ipv4Addr::new(10, 0, 0, 50),
            Ipv4Addr::new(192, 168, 65, 254),
        ];
        let response = handle_query(&query, 1, &custom_ips);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        // Should have 2 A records - one for each IP
        assert_eq!(parsed.answers.len(), 2);

        // Verify both IPs are in the response
        let mut found_ips: Vec<Ipv4Addr> = Vec::new();
        for answer in &parsed.answers {
            if let RData::A(a_record) = &answer.rdata {
                let expected = A::from(custom_ips[found_ips.len()]);
                assert_eq!(*a_record, expected);
                found_ips.push(custom_ips[found_ips.len()]);
            }
        }
        assert_eq!(found_ips.len(), 2);
    }
}
