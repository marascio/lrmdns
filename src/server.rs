use crate::protocol::QueryProcessor;
use anyhow::{Context, Result};
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
use std::sync::Arc;
use tokio::net::UdpSocket;

const MAX_DNS_PACKET_SIZE: usize = 512;

pub struct DnsServer {
    processor: Arc<QueryProcessor>,
    listen_addr: String,
}

impl DnsServer {
    pub fn new(processor: QueryProcessor, listen_addr: String) -> Self {
        DnsServer {
            processor: Arc::new(processor),
            listen_addr,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let socket = UdpSocket::bind(&self.listen_addr)
            .await
            .context(format!("Failed to bind to {}", self.listen_addr))?;

        tracing::info!("DNS server listening on {} (UDP)", self.listen_addr);

        let socket = Arc::new(socket);
        let mut buf = vec![0u8; MAX_DNS_PACKET_SIZE];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let processor = self.processor.clone();
                    let socket = socket.clone();

                    // Spawn a task to handle the query
                    tokio::spawn(async move {
                        if let Err(e) = handle_query(data, addr, processor, socket).await {
                            tracing::error!("Error handling query from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Error receiving packet: {}", e);
                }
            }
        }
    }
}

async fn handle_query(
    data: Vec<u8>,
    addr: std::net::SocketAddr,
    processor: Arc<QueryProcessor>,
    socket: Arc<UdpSocket>,
) -> Result<()> {
    // Parse the DNS query
    let query = match Message::from_bytes(&data) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!("Failed to parse DNS query from {}: {}", addr, e);

            // Send FORMERR response
            let mut response = Message::new();
            if data.len() >= 2 {
                let id = u16::from_be_bytes([data[0], data[1]]);
                response.set_id(id);
            }
            response.set_message_type(hickory_proto::op::MessageType::Response);
            response.set_response_code(hickory_proto::op::ResponseCode::FormErr);

            let response_buf = response.to_bytes()
                .context("Failed to encode FORMERR response")?;
            socket.send_to(&response_buf, addr).await?;
            return Ok(());
        }
    };

    tracing::debug!(
        "Received query from {}: id={} questions={}",
        addr,
        query.id(),
        query.queries().len()
    );

    // Process the query
    let response = processor.process_query(&query)?;

    // Encode the response
    let response_buf = response.to_bytes()
        .context("Failed to encode DNS response")?;

    // Check if response fits in UDP packet
    if response_buf.len() > MAX_DNS_PACKET_SIZE {
        tracing::warn!(
            "Response too large ({} bytes), truncating",
            response_buf.len()
        );

        // Create truncated response
        let mut truncated = response.clone();
        truncated.set_truncated(true);
        // Remove answers to make it fit
        while !truncated.answers().is_empty() {
            truncated.take_answers();
            let buf = truncated.to_bytes()?;
            if buf.len() <= MAX_DNS_PACKET_SIZE {
                socket.send_to(&buf, addr).await?;
                return Ok(());
            }
        }
    }

    // Send the response
    socket.send_to(&response_buf, addr).await?;

    tracing::debug!(
        "Sent response to {}: id={} rcode={:?} answers={}",
        addr,
        response.id(),
        response.response_code(),
        response.answers().len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::{SoaRecord, Zone, ZoneStore};
    use hickory_proto::op::{Query, OpCode};
    use hickory_proto::rr::{Name, RData, Record, RecordType};
    use std::net::Ipv4Addr;
    use std::str::FromStr;

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

        QueryProcessor::new(Arc::new(store))
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

        let response = processor.process_query(&query).unwrap();

        assert_eq!(response.id(), 1234);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 1);
    }
}
