use crate::config::TcpConfig;
use crate::metrics::Metrics;
use crate::protocol::QueryProcessor;
use crate::ratelimit::RateLimiter;
use anyhow::{Context, Result};
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

const MAX_DNS_PACKET_SIZE: usize = 512;
const MAX_TCP_DNS_PACKET_SIZE: usize = 65535;

pub struct DnsServer {
    processor: Arc<QueryProcessor>,
    listen_addr: String,
    metrics: Arc<Metrics>,
    rate_limiter: Option<Arc<RateLimiter>>,
    tcp_config: Option<TcpConfig>,
}

impl DnsServer {
    pub fn new(
        processor: QueryProcessor,
        listen_addr: String,
        metrics: Arc<Metrics>,
        rate_limiter: Option<Arc<RateLimiter>>,
        tcp_config: Option<TcpConfig>,
    ) -> Self {
        DnsServer {
            processor: Arc::new(processor),
            listen_addr,
            metrics,
            rate_limiter,
            tcp_config,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let udp_future = self.run_udp();
        let tcp_future = self.run_tcp();

        // Run both servers concurrently
        tokio::try_join!(udp_future, tcp_future)?;

        Ok(())
    }

    async fn run_udp(&self) -> Result<()> {
        let socket = UdpSocket::bind(&self.listen_addr)
            .await
            .context(format!("Failed to bind UDP to {}", self.listen_addr))?;

        tracing::info!("DNS server listening on {} (UDP)", self.listen_addr);

        let socket = Arc::new(socket);
        let mut buf = vec![0u8; MAX_DNS_PACKET_SIZE];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let processor = self.processor.clone();
                    let socket = socket.clone();

                    let metrics = self.metrics.clone();
                    let rate_limiter = self.rate_limiter.clone();

                    // Spawn a task to handle the query
                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_udp_query(data, addr, processor, socket, metrics, rate_limiter)
                                .await
                        {
                            tracing::error!("Error handling UDP query from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Error receiving UDP packet: {}", e);
                }
            }
        }
    }

    async fn run_tcp(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.listen_addr)
            .await
            .context(format!("Failed to bind TCP to {}", self.listen_addr))?;

        tracing::info!("DNS server listening on {} (TCP)", self.listen_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let processor = self.processor.clone();
                    let metrics = self.metrics.clone();
                    let rate_limiter = self.rate_limiter.clone();
                    let zones = processor.get_zones();
                    let tcp_config = self.tcp_config.clone();

                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        if let Err(e) = handle_tcp_connection(
                            stream,
                            addr,
                            processor,
                            metrics,
                            rate_limiter,
                            zones,
                            tcp_config,
                        )
                        .await
                        {
                            tracing::error!("Error handling TCP connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Error accepting TCP connection: {}", e);
                }
            }
        }
    }
}

async fn handle_udp_query(
    data: Vec<u8>,
    addr: std::net::SocketAddr,
    processor: Arc<QueryProcessor>,
    socket: Arc<UdpSocket>,
    metrics: Arc<Metrics>,
    rate_limiter: Option<Arc<RateLimiter>>,
) -> Result<()> {
    use crate::metrics::Protocol;
    use std::time::Instant;

    let start = Instant::now();

    // Check rate limiting
    if let Some(ref limiter) = rate_limiter
        && !limiter.check_rate_limit(addr.ip())
    {
        metrics.record_rate_limited();
        tracing::warn!("Rate limited query from {}", addr);

        // Send REFUSED response
        let mut response = Message::new();
        if data.len() >= 2 {
            let id = u16::from_be_bytes([data[0], data[1]]);
            response.set_id(id);
        }
        response.set_message_type(hickory_proto::op::MessageType::Response);
        response.set_response_code(hickory_proto::op::ResponseCode::Refused);

        let response_buf = response
            .to_bytes()
            .context("Failed to encode rate limit response")?;
        socket.send_to(&response_buf, addr).await?;
        return Ok(());
    }

    // Parse the DNS query
    let query = match Message::from_bytes(&data) {
        Ok(msg) => msg,
        Err(e) => {
            metrics.record_error();
            tracing::warn!("Failed to parse DNS query from {}: {}", addr, e);

            // Send FORMERR response
            let mut response = Message::new();
            if data.len() >= 2 {
                let id = u16::from_be_bytes([data[0], data[1]]);
                response.set_id(id);
            }
            response.set_message_type(hickory_proto::op::MessageType::Response);
            response.set_response_code(hickory_proto::op::ResponseCode::FormErr);

            metrics.record_response(hickory_proto::op::ResponseCode::FormErr);

            let response_buf = response
                .to_bytes()
                .context("Failed to encode FORMERR response")?;
            socket.send_to(&response_buf, addr).await?;

            metrics.record_latency(start.elapsed());
            return Ok(());
        }
    };

    // Record query metrics
    let has_edns = query.extensions().is_some();
    metrics.record_query(Protocol::Udp, has_edns);

    // Record query type if we have questions
    if let Some(question) = query.queries().first() {
        metrics.record_query_type(question.query_type());
    }

    tracing::debug!(
        "Received query from {}: id={} questions={}",
        addr,
        query.id(),
        query.queries().len()
    );

    // Process the query
    let response = match processor.process_query(&query).await {
        Ok(resp) => resp,
        Err(e) => {
            metrics.record_error();
            metrics.record_latency(start.elapsed());
            return Err(e);
        }
    };

    // Encode the response
    let response_buf = response
        .to_bytes()
        .context("Failed to encode DNS response")?;

    // Determine max UDP packet size (EDNS0 or standard)
    let max_udp_size = if let Some(edns) = response.extensions() {
        edns.max_payload() as usize
    } else {
        MAX_DNS_PACKET_SIZE
    };

    // Check if response fits in UDP packet
    if response_buf.len() > max_udp_size {
        tracing::warn!(
            "Response too large ({} bytes, max {}), truncating",
            response_buf.len(),
            max_udp_size
        );

        // Create truncated response
        let mut truncated = response.clone();
        truncated.set_truncated(true);

        // Try removing answers first
        while !truncated.answers().is_empty() {
            truncated.take_answers();
            let buf = truncated.to_bytes()?;
            if buf.len() <= max_udp_size {
                socket.send_to(&buf, addr).await?;
                metrics.record_response(truncated.response_code());
                metrics.record_latency(start.elapsed());
                return Ok(());
            }
        }

        // If still too large, remove authority records
        while !truncated.name_servers().is_empty() {
            truncated.take_name_servers();
            let buf = truncated.to_bytes()?;
            if buf.len() <= max_udp_size {
                socket.send_to(&buf, addr).await?;
                metrics.record_response(truncated.response_code());
                metrics.record_latency(start.elapsed());
                return Ok(());
            }
        }

        // If still too large, remove additional records
        while !truncated.additionals().is_empty() {
            truncated.take_additionals();
            let buf = truncated.to_bytes()?;
            if buf.len() <= max_udp_size {
                socket.send_to(&buf, addr).await?;
                metrics.record_response(truncated.response_code());
                metrics.record_latency(start.elapsed());
                return Ok(());
            }
        }

        // If even minimal response doesn't fit, send it anyway with TC flag
        // This shouldn't happen in practice, but handles edge case
        let minimal_buf = truncated.to_bytes()?;
        socket.send_to(&minimal_buf, addr).await?;
        metrics.record_response(truncated.response_code());
        metrics.record_latency(start.elapsed());
        return Ok(());
    }

    // Send the response
    socket.send_to(&response_buf, addr).await?;

    // Record metrics
    metrics.record_response(response.response_code());
    metrics.record_latency(start.elapsed());

    tracing::debug!(
        "Sent response to {}: id={} rcode={:?} answers={}",
        addr,
        response.id(),
        response.response_code(),
        response.answers().len()
    );

    Ok(())
}

async fn handle_tcp_connection(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    processor: Arc<QueryProcessor>,
    metrics: Arc<Metrics>,
    rate_limiter: Option<Arc<RateLimiter>>,
    zones: Arc<tokio::sync::RwLock<crate::zone::ZoneStore>>,
    tcp_config: Option<TcpConfig>,
) -> Result<()> {
    use crate::metrics::Protocol;
    use std::time::Instant;

    tracing::debug!("TCP connection from {}", addr);

    // Record new TCP connection
    metrics.record_tcp_connection();

    // Get TCP configuration values with defaults
    let idle_timeout_secs = tcp_config.as_ref().map(|c| c.idle_timeout).unwrap_or(30);
    let max_queries = tcp_config
        .as_ref()
        .map(|c| c.max_queries_per_connection)
        .unwrap_or(100);

    let idle_timeout = Duration::from_secs(idle_timeout_secs);
    let mut queries_handled: u64 = 0;

    loop {
        // Check if we've hit the max queries limit
        if queries_handled >= max_queries as u64 {
            tracing::debug!(
                "TCP connection from {} reached max queries ({})",
                addr,
                max_queries
            );
            metrics.record_tcp_connection_closed(queries_handled);
            return Ok(());
        }
        let start = Instant::now();
        // Read 2-byte length prefix with timeout
        let mut len_buf = [0u8; 2];
        match tokio::time::timeout(idle_timeout, stream.read_exact(&mut len_buf)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Client closed connection
                tracing::debug!("TCP connection closed by {}", addr);
                metrics.record_tcp_connection_closed(queries_handled);
                return Ok(());
            }
            Ok(Err(e)) => {
                metrics.record_tcp_connection_closed(queries_handled);
                return Err(e).context("Failed to read length prefix");
            }
            Err(_) => {
                // Timeout
                tracing::debug!("TCP connection from {} timed out", addr);
                metrics.record_tcp_connection_timeout();
                metrics.record_tcp_connection_closed(queries_handled);
                return Ok(());
            }
        }

        let msg_len = u16::from_be_bytes(len_buf) as usize;

        if msg_len == 0 || msg_len > MAX_TCP_DNS_PACKET_SIZE {
            metrics.record_error();
            tracing::warn!("Invalid TCP DNS message length from {}: {}", addr, msg_len);
            return Ok(());
        }

        // Check rate limiting
        if let Some(ref limiter) = rate_limiter
            && !limiter.check_rate_limit(addr.ip())
        {
            metrics.record_rate_limited();
            tracing::warn!("Rate limited TCP query from {}", addr);

            // Send REFUSED response
            let mut response = Message::new();
            response.set_message_type(hickory_proto::op::MessageType::Response);
            response.set_response_code(hickory_proto::op::ResponseCode::Refused);

            metrics.record_response(hickory_proto::op::ResponseCode::Refused);

            let response_buf = response
                .to_bytes()
                .context("Failed to encode rate limit response")?;

            let len = (response_buf.len() as u16).to_be_bytes();
            stream.write_all(&len).await?;
            stream.write_all(&response_buf).await?;

            metrics.record_latency(start.elapsed());
            return Ok(());
        }

        // Read the DNS message
        let mut msg_buf = vec![0u8; msg_len];
        stream
            .read_exact(&mut msg_buf)
            .await
            .context("Failed to read DNS message")?;

        tracing::debug!("Received TCP query from {}: {} bytes", addr, msg_len);

        // Parse the DNS query
        let query = match Message::from_bytes(&msg_buf) {
            Ok(msg) => msg,
            Err(e) => {
                metrics.record_error();
                tracing::warn!("Failed to parse TCP DNS query from {}: {}", addr, e);

                // Send FORMERR response
                let mut response = Message::new();
                if msg_buf.len() >= 2 {
                    let id = u16::from_be_bytes([msg_buf[0], msg_buf[1]]);
                    response.set_id(id);
                }
                response.set_message_type(hickory_proto::op::MessageType::Response);
                response.set_response_code(hickory_proto::op::ResponseCode::FormErr);

                metrics.record_response(hickory_proto::op::ResponseCode::FormErr);

                let response_buf = response
                    .to_bytes()
                    .context("Failed to encode FORMERR response")?;

                // Send with length prefix
                let len = (response_buf.len() as u16).to_be_bytes();
                stream.write_all(&len).await?;
                stream.write_all(&response_buf).await?;

                metrics.record_latency(start.elapsed());
                return Ok(());
            }
        };

        // Record query metrics
        let has_edns = query.extensions().is_some();
        metrics.record_query(Protocol::Tcp, has_edns);

        // Record query type if we have questions
        if let Some(question) = query.queries().first() {
            metrics.record_query_type(question.query_type());
        }

        // Check if this is an AXFR query
        let is_axfr = query
            .queries()
            .first()
            .map(|q| q.query_type() == hickory_proto::rr::RecordType::AXFR)
            .unwrap_or(false);

        if is_axfr {
            // Handle AXFR zone transfer
            tracing::info!(
                "AXFR request from {} for {:?}",
                addr,
                query.queries().first().map(|q| q.name())
            );

            // Get the zone
            let zone_store = zones.read().await;
            if let Some(question) = query.queries().first() {
                if let Some(zone) = zone_store.find_zone(question.name()) {
                    // Get all records in the zone
                    let all_records = zone.get_all_records();

                    tracing::debug!("AXFR: Streaming {} records to {}", all_records.len(), addr);

                    // Stream each record as a separate DNS message
                    for record in all_records {
                        let mut axfr_msg = Message::new();
                        axfr_msg.set_id(query.id());
                        axfr_msg.set_message_type(hickory_proto::op::MessageType::Response);
                        axfr_msg.set_op_code(hickory_proto::op::OpCode::Query);
                        axfr_msg.set_authoritative(true);
                        axfr_msg.add_query(question.clone());
                        axfr_msg.add_answer(record);

                        let msg_buf = axfr_msg
                            .to_bytes()
                            .context("Failed to encode AXFR message")?;

                        let len = (msg_buf.len() as u16).to_be_bytes();
                        stream.write_all(&len).await?;
                        stream.write_all(&msg_buf).await?;
                    }

                    metrics.record_response(hickory_proto::op::ResponseCode::NoError);
                    metrics.record_latency(start.elapsed());
                    tracing::info!("AXFR completed for {}", addr);
                    return Ok(());
                } else {
                    // Not authoritative for this zone
                    tracing::warn!("AXFR refused for non-authoritative zone from {}", addr);
                    let mut refused_response = Message::new();
                    refused_response.set_id(query.id());
                    refused_response.set_message_type(hickory_proto::op::MessageType::Response);
                    refused_response.set_response_code(hickory_proto::op::ResponseCode::Refused);
                    refused_response.add_query(question.clone());

                    let response_buf = refused_response
                        .to_bytes()
                        .context("Failed to encode refused response")?;
                    let len = (response_buf.len() as u16).to_be_bytes();
                    stream.write_all(&len).await?;
                    stream.write_all(&response_buf).await?;

                    metrics.record_response(hickory_proto::op::ResponseCode::Refused);
                    metrics.record_latency(start.elapsed());
                    return Ok(());
                }
            }
        }

        // Process the query (normal, non-AXFR)
        let response = match processor.process_query(&query).await {
            Ok(resp) => resp,
            Err(e) => {
                metrics.record_error();
                metrics.record_latency(start.elapsed());
                return Err(e);
            }
        };

        // Encode the response
        let response_buf = response
            .to_bytes()
            .context("Failed to encode DNS response")?;

        tracing::debug!(
            "Sending TCP response to {}: id={} rcode={:?} answers={} ({} bytes)",
            addr,
            response.id(),
            response.response_code(),
            response.answers().len(),
            response_buf.len()
        );

        // Send with length prefix
        let len = (response_buf.len() as u16).to_be_bytes();
        stream.write_all(&len).await?;
        stream.write_all(&response_buf).await?;

        // Record metrics
        metrics.record_response(response.response_code());
        metrics.record_latency(start.elapsed());

        // Increment query counter for this connection
        queries_handled += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::{SoaRecord, Zone, ZoneStore};
    use hickory_proto::op::{OpCode, Query};
    use hickory_proto::rr::{Name, RData, Record, RecordType};
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use tokio::sync::RwLock;

    fn create_test_processor() -> QueryProcessor {
        let origin = Name::from_str("test.local.").unwrap();
        let soa = SoaRecord {
            mname: Name::from_str("ns1.test.local.").unwrap(),
            rname: Name::from_str("admin.test.local.").unwrap(),
            serial: 1,
            refresh: 7200,
            retry: 3600,
            expire: 1209600,
            minimum: 86400,
        };

        let mut zone = Zone::new(origin.clone(), soa);

        let a_record = Record::from_rdata(
            Name::from_str("www.test.local.").unwrap(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(127, 0, 0, 1))),
        );
        zone.add_record(a_record);

        let mut store = ZoneStore::new();
        store.add_zone(zone);

        QueryProcessor::new(Arc::new(RwLock::new(store)))
    }

    #[tokio::test]
    async fn test_query_processing() {
        let processor = create_test_processor();

        let mut query = Message::new();
        query.set_id(1234);
        query.set_message_type(hickory_proto::op::MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.add_query(Query::query(
            Name::from_str("www.test.local.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.id(), 1234);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 1);
    }

    #[test]
    fn test_udp_truncation_with_large_authority_section() {
        // This test demonstrates Bug #3: UDP truncation fallback sends oversized packet
        // When a response exceeds UDP size limit and removing all answers still doesn't fit
        // (due to large authority/additional sections), the code sends the original oversized packet

        use hickory_proto::op::{Message, MessageType};
        use std::net::Ipv4Addr;

        // Create a response with many NS records in authority section
        let mut response = Message::new();
        response.set_id(1234);
        response.set_message_type(MessageType::Response);
        response.set_authoritative(true);

        // Add one answer
        let answer = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 1))),
        );
        response.add_answer(answer);

        // Add many NS records to authority section to make it large
        // Each NS record is about 20-30 bytes, so we need many to exceed 512 bytes
        for i in 0..50 {
            let ns_record = Record::from_rdata(
                Name::from_utf8("example.com.").unwrap(),
                300,
                RData::NS(hickory_proto::rr::rdata::NS(
                    Name::from_utf8(format!("ns{}.example.com.", i)).unwrap(),
                )),
            );
            response.add_name_server(ns_record);
        }

        // Serialize the response
        let response_buf = response.to_bytes().unwrap();

        // Verify that the response is larger than MAX_DNS_PACKET_SIZE (512)
        assert!(
            response_buf.len() > MAX_DNS_PACKET_SIZE,
            "Test setup error: response should be larger than {} bytes, got {}",
            MAX_DNS_PACKET_SIZE,
            response_buf.len()
        );

        // Simulate truncation logic
        let max_udp_size = MAX_DNS_PACKET_SIZE;

        let mut truncated = response.clone();
        truncated.set_truncated(true);

        // Try removing answers to make it fit
        while !truncated.answers().is_empty() {
            truncated.take_answers();
            let buf = truncated.to_bytes().unwrap();
            if buf.len() <= max_udp_size {
                // Successfully truncated to fit
                assert!(buf.len() <= max_udp_size, "Truncated response should fit");
                assert!(truncated.truncated(), "TC flag should be set");
                return;
            }
        }

        // If still too large, remove authority records
        while !truncated.name_servers().is_empty() {
            truncated.take_name_servers();
            let buf = truncated.to_bytes().unwrap();
            if buf.len() <= max_udp_size {
                // Successfully truncated to fit
                assert!(buf.len() <= max_udp_size, "Truncated response should fit");
                assert!(truncated.truncated(), "TC flag should be set");
                assert!(truncated.answers().is_empty(), "Answers should be removed");
                assert!(
                    truncated.name_servers().is_empty(),
                    "Authority records should be removed"
                );
                return;
            }
        }

        // If still too large, remove additional records
        while !truncated.additionals().is_empty() {
            truncated.take_additionals();
            let buf = truncated.to_bytes().unwrap();
            if buf.len() <= max_udp_size {
                // Successfully truncated to fit
                assert!(buf.len() <= max_udp_size, "Truncated response should fit");
                assert!(truncated.truncated(), "TC flag should be set");
                return;
            }
        }

        // Final check - send minimal response with just header and TC flag
        let final_buf = truncated.to_bytes().unwrap();
        assert!(
            final_buf.len() <= max_udp_size,
            "Even minimal truncated response should fit within {} bytes, got {}",
            max_udp_size,
            final_buf.len()
        );
    }
}
