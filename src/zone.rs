use anyhow::{Context, Result};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Zone {
    pub origin: Name,
    pub soa: SoaRecord,
    pub records: HashMap<Name, HashMap<RecordType, Vec<Record>>>,
}

#[derive(Debug, Clone)]
pub struct SoaRecord {
    pub mname: Name,
    pub rname: Name,
    pub serial: u32,
    pub refresh: i32,
    pub retry: i32,
    pub expire: i32,
    pub minimum: u32,
}

impl Zone {
    pub fn new(origin: Name, soa: SoaRecord) -> Self {
        Zone {
            origin,
            soa,
            records: HashMap::new(),
        }
    }

    pub fn add_record(&mut self, record: Record) {
        let name = record.name().clone();
        let rtype = record.record_type();

        self.records
            .entry(name)
            .or_insert_with(HashMap::new)
            .entry(rtype)
            .or_insert_with(Vec::new)
            .push(record);
    }

    pub fn lookup(&self, name: &Name, rtype: RecordType) -> Option<&Vec<Record>> {
        self.records.get(name)?.get(&rtype)
    }

    pub fn contains_name(&self, name: &Name) -> bool {
        self.records.contains_key(name)
    }

    pub fn get_soa_record(&self) -> Record {
        let rdata = RData::SOA(hickory_proto::rr::rdata::SOA::new(
            self.soa.mname.clone(),
            self.soa.rname.clone(),
            self.soa.serial,
            self.soa.refresh,
            self.soa.retry,
            self.soa.expire,
            self.soa.minimum,
        ));

        Record::from_rdata(
            self.origin.clone(),
            self.soa.minimum,
            rdata,
        )
    }
}

#[derive(Debug)]
pub struct ZoneStore {
    zones: HashMap<Name, Zone>,
}

impl ZoneStore {
    pub fn new() -> Self {
        ZoneStore {
            zones: HashMap::new(),
        }
    }

    pub fn add_zone(&mut self, zone: Zone) {
        self.zones.insert(zone.origin.clone(), zone);
    }

    pub fn find_zone(&self, name: &Name) -> Option<&Zone> {
        // Try exact match first
        if let Some(zone) = self.zones.get(name) {
            return Some(zone);
        }

        // Find the zone with the longest matching suffix
        let mut best_match: Option<&Zone> = None;
        let mut best_match_labels = 0;

        for zone in self.zones.values() {
            // Check if this name is in the zone (zone.origin is a parent of name)
            if zone.origin.zone_of(name) {
                let labels = zone.origin.num_labels();
                if labels > best_match_labels {
                    best_match = Some(zone);
                    best_match_labels = labels;
                }
            }
        }

        best_match
    }
}

pub fn parse_zone_file<P: AsRef<Path>>(path: P, origin_name: &str) -> Result<Zone> {
    let content = std::fs::read_to_string(path.as_ref())
        .context("Failed to read zone file")?;

    let origin = Name::from_str(origin_name)
        .context("Invalid origin name")?;

    let mut zone: Option<Zone> = None;
    let mut default_ttl: u32 = 3600;
    let mut current_origin = origin.clone();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        // Handle directives
        if line.starts_with('$') {
            if line.starts_with("$ORIGIN") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    current_origin = Name::from_str(parts[1])
                        .context(format!("Invalid $ORIGIN on line {}", line_num + 1))?;
                }
            } else if line.starts_with("$TTL") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    default_ttl = parts[1].parse()
                        .context(format!("Invalid $TTL on line {}", line_num + 1))?;
                }
            }
            continue;
        }

        // Parse resource record
        if let Some(record) = parse_resource_record(line, &current_origin, default_ttl, line_num)? {
            // If this is SOA and we don't have a zone yet, create it
            if record.record_type() == RecordType::SOA && zone.is_none() {
                if let Some(soa_data) = extract_soa_data(&record) {
                    zone = Some(Zone::new(origin.clone(), soa_data));
                }
            }

            if let Some(ref mut z) = zone {
                z.add_record(record);
            }
        }
    }

    zone.ok_or_else(|| anyhow::anyhow!("Zone file must contain an SOA record"))
}

fn parse_resource_record(
    line: &str,
    origin: &Name,
    default_ttl: u32,
    line_num: usize,
) -> Result<Option<Record>> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return Ok(None);
    }

    let mut idx = 0;

    // Parse name
    let name = if parts[idx] == "@" {
        origin.clone()
    } else if parts[idx].ends_with('.') {
        Name::from_str(parts[idx])
            .context(format!("Invalid name on line {}", line_num + 1))?
    } else {
        Name::from_str(&format!("{}.{}", parts[idx], origin))
            .context(format!("Invalid name on line {}", line_num + 1))?
    };
    idx += 1;

    // Parse optional TTL or class
    let mut ttl = default_ttl;
    if parts[idx].parse::<u32>().is_ok() {
        ttl = parts[idx].parse().unwrap();
        idx += 1;
    }

    // Skip class if present (we only support IN)
    if parts[idx] == "IN" {
        idx += 1;
    }

    // Parse record type
    let rtype = parts[idx];
    idx += 1;

    // Parse RDATA
    let rdata = match rtype {
        "A" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let addr = parts[idx].parse::<Ipv4Addr>()
                .context(format!("Invalid A record on line {}", line_num + 1))?;
            RData::A(hickory_proto::rr::rdata::A(addr))
        }
        "AAAA" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let addr = parts[idx].parse::<Ipv6Addr>()
                .context(format!("Invalid AAAA record on line {}", line_num + 1))?;
            RData::AAAA(hickory_proto::rr::rdata::AAAA(addr))
        }
        "NS" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let nsdname = parse_domain_name(parts[idx], origin)
                .context(format!("Invalid NS record on line {}", line_num + 1))?;
            RData::NS(hickory_proto::rr::rdata::NS(nsdname))
        }
        "SOA" => {
            if parts.len() < idx + 7 {
                return Ok(None);
            }
            let mname = parse_domain_name(parts[idx], origin)?;
            let rname = parse_domain_name(parts[idx + 1], origin)?;
            let serial = parts[idx + 2].parse()
                .context(format!("Invalid SOA serial on line {}", line_num + 1))?;
            let refresh = parts[idx + 3].parse()
                .context(format!("Invalid SOA refresh on line {}", line_num + 1))?;
            let retry = parts[idx + 4].parse()
                .context(format!("Invalid SOA retry on line {}", line_num + 1))?;
            let expire = parts[idx + 5].parse()
                .context(format!("Invalid SOA expire on line {}", line_num + 1))?;
            let minimum = parts[idx + 6].parse()
                .context(format!("Invalid SOA minimum on line {}", line_num + 1))?;

            RData::SOA(hickory_proto::rr::rdata::SOA::new(
                mname, rname, serial, refresh, retry, expire, minimum,
            ))
        }
        "CNAME" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let cname = parse_domain_name(parts[idx], origin)
                .context(format!("Invalid CNAME record on line {}", line_num + 1))?;
            RData::CNAME(hickory_proto::rr::rdata::CNAME(cname))
        }
        "MX" => {
            if parts.len() < idx + 2 {
                return Ok(None);
            }
            let preference = parts[idx].parse::<u16>()
                .context(format!("Invalid MX preference on line {}", line_num + 1))?;
            let exchange = parse_domain_name(parts[idx + 1], origin)
                .context(format!("Invalid MX exchange on line {}", line_num + 1))?;
            RData::MX(hickory_proto::rr::rdata::MX::new(preference, exchange))
        }
        "TXT" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            // Join all remaining parts as the TXT data (handles quoted strings)
            let txt_data = parts[idx..].join(" ");
            // Remove quotes if present
            let txt_data = txt_data.trim_matches('"');
            RData::TXT(hickory_proto::rr::rdata::TXT::new(vec![txt_data.to_string()]))
        }
        _ => {
            tracing::warn!("Unsupported record type {} on line {}", rtype, line_num + 1);
            return Ok(None);
        }
    };

    Ok(Some(Record::from_rdata(name, ttl, rdata)))
}

fn parse_domain_name(s: &str, origin: &Name) -> Result<Name> {
    if s.ends_with('.') {
        Ok(Name::from_str(s)?)
    } else {
        Ok(Name::from_str(&format!("{}.{}", s, origin))?)
    }
}

fn extract_soa_data(record: &Record) -> Option<SoaRecord> {
    if let Some(RData::SOA(soa)) = record.data() {
        Some(SoaRecord {
            mname: soa.mname().clone(),
            rname: soa.rname().clone(),
            serial: soa.serial(),
            refresh: soa.refresh(),
            retry: soa.retry(),
            expire: soa.expire(),
            minimum: soa.minimum(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_store_find_zone() {
        let mut store = ZoneStore::new();

        let origin = Name::from_str("example.com.").unwrap();
        let soa = SoaRecord {
            mname: Name::from_str("ns1.example.com.").unwrap(),
            rname: Name::from_str("admin.example.com.").unwrap(),
            serial: 1,
            refresh: 7200,
            retry: 3600,
            expire: 1209600,
            minimum: 86400,
        };

        let zone = Zone::new(origin, soa);
        store.add_zone(zone);

        let query = Name::from_str("www.example.com.").unwrap();
        assert!(store.find_zone(&query).is_some());

        let query = Name::from_str("example.org.").unwrap();
        assert!(store.find_zone(&query).is_none());
    }
}
