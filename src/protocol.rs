use crate::zone::ZoneStore;
use anyhow::Result;
use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::RecordType;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QueryProcessor {
    zones: Arc<RwLock<ZoneStore>>,
}

impl QueryProcessor {
    pub fn new(zones: Arc<RwLock<ZoneStore>>) -> Self {
        QueryProcessor { zones }
    }

    pub async fn process_query(&self, query: &Message) -> Result<Message> {
        let mut response = Message::new();

        // Copy query ID and set response flags
        response.set_id(query.id());
        response.set_message_type(MessageType::Response);
        response.set_op_code(OpCode::Query);
        response.set_recursion_desired(query.recursion_desired());
        response.set_recursion_available(false);

        // We only handle standard queries
        if query.op_code() != OpCode::Query {
            response.set_response_code(ResponseCode::NotImp);
            return Ok(response);
        }

        // Get the first question
        let question = match query.queries().first() {
            Some(q) => q,
            None => {
                response.set_response_code(ResponseCode::FormErr);
                return Ok(response);
            }
        };

        // Add the question to the response
        response.add_query(question.clone());

        let qname = question.name();
        let qtype = question.query_type();

        // Check for EDNS0 support
        let edns = query.extensions();
        let client_udp_size = if let Some(edns) = edns {
            edns.max_payload()
        } else {
            512 // Default DNS UDP packet size
        };

        tracing::debug!(
            "Query: name={} type={:?} edns_size={} from={}",
            qname,
            qtype,
            client_udp_size,
            "unknown" // Will be filled in by server
        );

        // Find the authoritative zone
        let zones = self.zones.read().await;
        let zone = match zones.find_zone(qname) {
            Some(z) => z,
            None => {
                // Not authoritative for this zone
                response.set_response_code(ResponseCode::Refused);
                tracing::debug!("Not authoritative for zone: {}", qname);
                return Ok(response);
            }
        };

        // Set authoritative answer flag
        response.set_authoritative(true);

        // Check if the name exists in the zone
        let name_exists = zone.contains_name(qname);

        // Lookup the requested record type
        let lookup_result = if name_exists {
            zone.lookup(qname, qtype)
        } else {
            // Try wildcard lookup if exact name doesn't exist
            zone.lookup_wildcard(qname, qtype)
        };

        match lookup_result {
            Some(records) => {
                // Found records of the requested type
                for record in records {
                    response.add_answer(record.clone());
                }
                response.set_response_code(ResponseCode::NoError);
                tracing::debug!("Found {} records for {} {:?}", records.len(), qname, qtype);
            }
            None => {
                // Check if there's a CNAME record for this name (exact or wildcard)
                let cname_result = if name_exists {
                    zone.lookup(qname, RecordType::CNAME)
                } else {
                    zone.lookup_wildcard(qname, RecordType::CNAME)
                };

                if let Some(cname_records) = cname_result {
                    // Add CNAME record(s) to answer
                    for cname_record in cname_records {
                        response.add_answer(cname_record.clone());

                        // Chase the CNAME to find the target records
                        if let Some(rdata) = cname_record.data() {
                            if let hickory_proto::rr::RData::CNAME(cname) = rdata {
                                let target = cname.0.clone();

                                // Try to find the target record of the requested type
                                if let Some(target_records) = zone.lookup(&target, qtype) {
                                    for target_record in target_records {
                                        response.add_answer(target_record.clone());
                                    }
                                    tracing::debug!(
                                        "CNAME {} -> {}, found {} {:?} records",
                                        qname,
                                        target,
                                        target_records.len(),
                                        qtype
                                    );
                                }
                            }
                        }
                    }
                    response.set_response_code(ResponseCode::NoError);
                } else if name_exists {
                    // Name exists but no record of this type and no CNAME
                    response.set_response_code(ResponseCode::NoError);

                    // Add SOA in authority section
                    response.add_name_server(zone.get_soa_record());

                    tracing::debug!("Name exists but no {:?} record: {}", qtype, qname);
                } else {
                    // Name doesn't exist and no wildcard match - NXDOMAIN
                    response.set_response_code(ResponseCode::NXDomain);

                    // Add SOA record in authority section for negative caching
                    response.add_name_server(zone.get_soa_record());

                    tracing::debug!("Name not found (no wildcard match): {}", qname);
                }
            }
        }

        // Add NS records in authority section for positive responses
        if response.response_code() == ResponseCode::NoError && !response.answers().is_empty() {
            if let Some(ns_records) = zone.lookup(&zone.origin, RecordType::NS) {
                for record in ns_records {
                    response.add_name_server(record.clone());
                }
            }
        }

        // Add EDNS0 support if client requested it
        if query.extensions().is_some() {
            let mut edns = hickory_proto::op::Edns::new();
            // Advertise our supported UDP payload size (4096 bytes)
            edns.set_max_payload(4096);
            edns.set_version(0);
            response.set_edns(edns);
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::{SoaRecord, Zone};
    use hickory_proto::op::Query;
    use hickory_proto::rr::{Name, RData, Record};
    use std::net::Ipv4Addr;
    use std::str::FromStr;

    fn create_test_zone() -> Zone {
        let origin = Name::from_str("example.com.").unwrap();
        let soa = SoaRecord {
            mname: Name::from_str("ns1.example.com.").unwrap(),
            rname: Name::from_str("admin.example.com.").unwrap(),
            serial: 2025120601,
            refresh: 7200,
            retry: 3600,
            expire: 1209600,
            minimum: 86400,
        };

        let mut zone = Zone::new(origin.clone(), soa);

        // Add A record
        let a_record = Record::from_rdata(
            Name::from_str("www.example.com.").unwrap(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 1))),
        );
        zone.add_record(a_record);

        // Add NS record
        let ns_record = Record::from_rdata(
            origin.clone(),
            3600,
            RData::NS(hickory_proto::rr::rdata::NS(
                Name::from_str("ns1.example.com.").unwrap(),
            )),
        );
        zone.add_record(ns_record);

        zone
    }

    #[tokio::test]
    async fn test_successful_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(RwLock::new(store)));

        let mut query = Message::new();
        query.set_id(1234);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.add_query(Query::query(
            Name::from_str("www.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.id(), 1234);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 1);
    }

    #[tokio::test]
    async fn test_nxdomain_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(RwLock::new(store)));

        let mut query = Message::new();
        query.set_id(5678);
        query.add_query(Query::query(
            Name::from_str("nonexistent.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.response_code(), ResponseCode::NXDomain);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 0);
    }

    #[tokio::test]
    async fn test_refused_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(RwLock::new(store)));

        let mut query = Message::new();
        query.add_query(Query::query(
            Name::from_str("example.org.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.response_code(), ResponseCode::Refused);
    }

    #[tokio::test]
    async fn test_wildcard_query() {
        let origin = Name::from_str("example.com.").unwrap();
        let soa = SoaRecord {
            mname: Name::from_str("ns1.example.com.").unwrap(),
            rname: Name::from_str("admin.example.com.").unwrap(),
            serial: 2025120601,
            refresh: 7200,
            retry: 3600,
            expire: 1209600,
            minimum: 86400,
        };

        let mut zone = Zone::new(origin.clone(), soa);

        // Add wildcard A record
        let wildcard_record = Record::from_rdata(
            Name::from_str("*.example.com.").unwrap(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 100))),
        );
        zone.add_record(wildcard_record);

        // Add specific record that should override wildcard
        let www_record = Record::from_rdata(
            Name::from_str("www.example.com.").unwrap(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 10))),
        );
        zone.add_record(www_record);

        // Add NS record for authority section
        let ns_record = Record::from_rdata(
            origin.clone(),
            3600,
            RData::NS(hickory_proto::rr::rdata::NS(
                Name::from_str("ns1.example.com.").unwrap(),
            )),
        );
        zone.add_record(ns_record);

        let mut store = ZoneStore::new();
        store.add_zone(zone);
        let processor = QueryProcessor::new(Arc::new(RwLock::new(store)));

        // Test wildcard match for non-existent name
        let mut query = Message::new();
        query.set_id(1111);
        query.add_query(Query::query(
            Name::from_str("random.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.id(), 1111);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 1);

        // Verify the answer is the wildcard IP
        if let Some(RData::A(a)) = response.answers()[0].data() {
            assert_eq!(a.0, Ipv4Addr::new(192, 0, 2, 100));
        }

        // Test that specific record overrides wildcard
        let mut query = Message::new();
        query.set_id(2222);
        query.add_query(Query::query(
            Name::from_str("www.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).await.unwrap();

        assert_eq!(response.id(), 2222);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert_eq!(response.answers().len(), 1);

        // Verify the answer is the specific IP, not wildcard
        if let Some(RData::A(a)) = response.answers()[0].data() {
            assert_eq!(a.0, Ipv4Addr::new(192, 0, 2, 10));
        }
    }
}
