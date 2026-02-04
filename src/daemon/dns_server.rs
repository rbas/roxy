use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use anyhow::Result;
use simple_dns::rdata::{RData, A, AAAA};
use simple_dns::{Name, Packet, PacketFlag, Question, ResourceRecord, CLASS, QTYPE, RCODE, TYPE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use crate::infrastructure::logging::LogFile;

pub struct DnsServer {
    port: u16,
    ttl: u32,
}

impl DnsServer {
    pub fn new(port: u16) -> Self {
        Self { port, ttl: 1 }
    }

    pub async fn run(&self) -> Result<()> {
        let log = LogFile::new();

        let ipv4_addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let ipv6_addr = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], self.port));

        // Bind UDP sockets
        let udp_v4 = UdpSocket::bind(ipv4_addr).await?;
        let udp_v6 = UdpSocket::bind(ipv6_addr).await?;

        // Bind TCP listeners
        let tcp_v4 = TcpListener::bind(ipv4_addr).await?;
        let tcp_v6 = TcpListener::bind(ipv6_addr).await?;

        let _ = log.log(&format!("DNS server listening on {} (UDP/TCP)", ipv4_addr));
        let _ = log.log(&format!("DNS server listening on {} (UDP/TCP)", ipv6_addr));
        println!("DNS server listening on {} (UDP/TCP)", ipv4_addr);
        println!("DNS server listening on {} (UDP/TCP)", ipv6_addr);

        let ttl = self.ttl;

        tokio::select! {
            r = serve_udp(udp_v4, ttl) => r,
            r = serve_udp(udp_v6, ttl) => r,
            r = serve_tcp(tcp_v4, ttl) => r,
            r = serve_tcp(tcp_v6, ttl) => r,
        }
    }
}

async fn serve_udp(socket: UdpSocket, ttl: u32) -> Result<()> {
    let mut buf = [0u8; 512]; // Standard DNS UDP size

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let response = handle_query(&buf[..len], ttl);
        let _ = socket.send_to(&response, addr).await;
    }
}

async fn serve_tcp(listener: TcpListener, ttl: u32) -> Result<()> {
    loop {
        let (stream, _addr) = listener.accept().await?;
        tokio::spawn(handle_tcp_connection(stream, ttl));
    }
}

async fn handle_tcp_connection(mut stream: TcpStream, ttl: u32) -> Result<()> {
    // TCP DNS uses 2-byte length prefix
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;

    let mut query_buf = vec![0u8; len];
    stream.read_exact(&mut query_buf).await?;

    let response = handle_query(&query_buf, ttl);

    // Send response with length prefix
    let resp_len = (response.len() as u16).to_be_bytes();
    stream.write_all(&resp_len).await?;
    stream.write_all(&response).await?;

    Ok(())
}

fn handle_query(query: &[u8], ttl: u32) -> Vec<u8> {
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
        QTYPE::TYPE(TYPE::A) => build_a_response(&packet, question, ttl),
        QTYPE::TYPE(TYPE::AAAA) => build_aaaa_response(&packet, question, ttl),
        QTYPE::ANY => build_any_response(&packet, question, ttl),
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

fn build_a_response(packet: &Packet, question: &Question, ttl: u32) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add A record: 127.0.0.1
    let name_str = question.qname.to_string();
    let name = Name::new_unchecked(&name_str);
    let a_record = A::from(Ipv4Addr::new(127, 0, 0, 1));
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

fn build_any_response(packet: &Packet, question: &Question, ttl: u32) -> Vec<u8> {
    let mut response = Packet::new_reply(packet.id());
    response.set_flags(
        PacketFlag::RESPONSE | PacketFlag::AUTHORITATIVE_ANSWER | PacketFlag::RECURSION_DESIRED,
    );
    *response.rcode_mut() = RCODE::NoError;

    // Add the question
    response.questions.push(question.clone());

    // Add both A and AAAA records
    let name_str = question.qname.to_string();

    let name_a = Name::new_unchecked(&name_str);
    let a_record = A::from(Ipv4Addr::new(127, 0, 0, 1));
    response
        .answers
        .push(ResourceRecord::new(name_a, CLASS::IN, ttl, RData::A(a_record)));

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

    #[test]
    fn test_query_handler_roxy_domain() {
        // Build a simple A query for test.roxy
        let mut packet = Packet::new_query(1234);
        let name = Name::new_unchecked("test.roxy");
        let question = Question::new(name, TYPE::A.into(), CLASS::IN.into(), false);
        packet.questions.push(question);

        let query = packet.build_bytes_vec().unwrap();
        let response = handle_query(&query, 1);

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
        let response = handle_query(&query, 1);

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
        let response = handle_query(&query, 1);

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
        let response = handle_query(&query, 1);

        let parsed = Packet::parse(&response).unwrap();
        assert_eq!(parsed.rcode(), RCODE::NoError);
        assert_eq!(parsed.answers.len(), 1);
    }
}
