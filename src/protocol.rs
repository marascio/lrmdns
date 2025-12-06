use crate::zone::ZoneStore;
use anyhow::Result;
use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::RecordType;
use std::sync::Arc;

pub struct QueryProcessor {
    zones: Arc<ZoneStore>,
}

impl QueryProcessor {
    pub fn new(zones: Arc<ZoneStore>) -> Self {
        QueryProcessor { zones }
    }

    pub fn process_query(&self, query: &Message) -> Result<Message> {
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

        tracing::debug!(
            "Query: name={} type={:?} from={}",
            qname,
            qtype,
            "unknown" // Will be filled in by server
        );

        // Find the authoritative zone
        let zone = match self.zones.find_zone(qname) {
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
        if !zone.contains_name(qname) {
            // Name doesn't exist - NXDOMAIN
            response.set_response_code(ResponseCode::NXDomain);

            // Add SOA record in authority section for negative caching
            response.add_name_server(zone.get_soa_record());

            tracing::debug!("Name not found: {}", qname);
            return Ok(response);
        }

        // Lookup the requested record type
        match zone.lookup(qname, qtype) {
            Some(records) => {
                // Found records of the requested type
                for record in records {
                    response.add_answer(record.clone());
                }
                response.set_response_code(ResponseCode::NoError);
                tracing::debug!("Found {} records for {} {:?}", records.len(), qname, qtype);
            }
            None => {
                // Name exists but no record of this type
                response.set_response_code(ResponseCode::NoError);

                // Add SOA in authority section
                response.add_name_server(zone.get_soa_record());

                tracing::debug!("Name exists but no {:?} record: {}", qtype, qname);
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

    #[test]
    fn test_successful_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(store));

        let mut query = Message::new();
        query.set_id(1234);
        query.set_message_type(MessageType::Query);
        query.set_op_code(OpCode::Query);
        query.add_query(Query::query(
            Name::from_str("www.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).unwrap();

        assert_eq!(response.id(), 1234);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 1);
    }

    #[test]
    fn test_nxdomain_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(store));

        let mut query = Message::new();
        query.set_id(5678);
        query.add_query(Query::query(
            Name::from_str("nonexistent.example.com.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).unwrap();

        assert_eq!(response.response_code(), ResponseCode::NXDomain);
        assert!(response.authoritative());
        assert_eq!(response.answers().len(), 0);
    }

    #[test]
    fn test_refused_query() {
        let mut store = ZoneStore::new();
        store.add_zone(create_test_zone());
        let processor = QueryProcessor::new(Arc::new(store));

        let mut query = Message::new();
        query.add_query(Query::query(
            Name::from_str("example.org.").unwrap(),
            RecordType::A,
        ));

        let response = processor.process_query(&query).unwrap();

        assert_eq!(response.response_code(), ResponseCode::Refused);
    }
}
