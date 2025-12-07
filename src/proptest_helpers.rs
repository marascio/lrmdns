#![allow(dead_code)]

use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use proptest::prelude::*;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

pub fn arb_dns_label() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::char::range('a', 'z'), 1..=63)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .prop_filter("Label cannot be empty", |s| !s.is_empty())
}

pub fn arb_dns_name() -> impl Strategy<Value = Name> {
    prop::collection::vec(arb_dns_label(), 1..=4)
        .prop_filter("DNS name must be <= 253 chars total", |labels| {
            let fqdn = format!("{}.", labels.join("."));
            fqdn.len() <= 253
        })
        .prop_map(|labels| {
            let fqdn = format!("{}.", labels.join("."));
            Name::from_str(&fqdn).unwrap()
        })
}

pub fn arb_record_type() -> impl Strategy<Value = RecordType> {
    prop_oneof![
        Just(RecordType::A),
        Just(RecordType::AAAA),
        Just(RecordType::NS),
        Just(RecordType::CNAME),
        Just(RecordType::MX),
        Just(RecordType::TXT),
        Just(RecordType::SOA),
        Just(RecordType::PTR),
        Just(RecordType::SRV),
        Just(RecordType::CAA),
    ]
}

pub fn arb_ipv4() -> impl Strategy<Value = Ipv4Addr> {
    any::<[u8; 4]>().prop_map(Ipv4Addr::from)
}

pub fn arb_ipv6() -> impl Strategy<Value = Ipv6Addr> {
    any::<[u8; 16]>().prop_map(Ipv6Addr::from)
}

pub fn arb_a_record(name: Name) -> impl Strategy<Value = Record> {
    arb_ipv4().prop_map(move |ip| {
        Record::from_rdata(
            name.clone(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(ip)),
        )
    })
}

pub fn arb_aaaa_record(name: Name) -> impl Strategy<Value = Record> {
    arb_ipv6().prop_map(move |ip| {
        Record::from_rdata(
            name.clone(),
            3600,
            RData::AAAA(hickory_proto::rr::rdata::AAAA(ip)),
        )
    })
}

pub fn arb_ns_record(name: Name) -> impl Strategy<Value = Record> {
    arb_dns_name().prop_map(move |ns_name| {
        Record::from_rdata(
            name.clone(),
            3600,
            RData::NS(hickory_proto::rr::rdata::NS(ns_name)),
        )
    })
}

pub fn arb_cname_record(name: Name) -> impl Strategy<Value = Record> {
    arb_dns_name().prop_map(move |target| {
        Record::from_rdata(
            name.clone(),
            3600,
            RData::CNAME(hickory_proto::rr::rdata::CNAME(target)),
        )
    })
}

pub fn arb_txt_record(name: Name) -> impl Strategy<Value = Record> {
    prop::collection::vec(prop::char::range('a', 'z'), 1..=255).prop_map(move |chars| {
        let text = chars.into_iter().collect::<String>();
        Record::from_rdata(
            name.clone(),
            3600,
            RData::TXT(hickory_proto::rr::rdata::TXT::new(vec![text])),
        )
    })
}

pub fn arb_record(name: Name) -> impl Strategy<Value = Record> {
    prop_oneof![
        arb_a_record(name.clone()),
        arb_aaaa_record(name.clone()),
        arb_ns_record(name.clone()),
        arb_cname_record(name.clone()),
        arb_txt_record(name),
    ]
}

pub fn arb_query_message() -> impl Strategy<Value = Message> {
    (any::<u16>(), arb_dns_name(), arb_record_type()).prop_map(|(id, name, rtype)| {
        let mut msg = Message::new();
        msg.set_id(id);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.add_query(Query::query(name, rtype));
        msg
    })
}

pub fn arb_recursion_desired() -> impl Strategy<Value = bool> {
    any::<bool>()
}

pub fn arb_ttl() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(60u32),
        Just(300u32),
        Just(3600u32),
        Just(86400u32),
        0u32..=2147483647u32,
    ]
}
