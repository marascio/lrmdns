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
            .or_default()
            .entry(rtype)
            .or_default()
            .push(record);
    }

    pub fn lookup(&self, name: &Name, rtype: RecordType) -> Option<&Vec<Record>> {
        self.records.get(name)?.get(&rtype)
    }

    pub fn contains_name(&self, name: &Name) -> bool {
        self.records.contains_key(name)
    }

    /// Lookup a wildcard record by finding the best matching wildcard
    /// Returns None if no wildcard matches
    pub fn lookup_wildcard(&self, name: &Name, rtype: RecordType) -> Option<&Vec<Record>> {
        // Try to find a wildcard match by constructing potential wildcard names
        // For "foo.bar.example.com", try "*.bar.example.com", then "*.example.com"
        let labels = name.iter().collect::<Vec<_>>();

        // Start from the second label (skip the leftmost label)
        for skip in 1..labels.len() {
            let mut wildcard_labels = vec![b"*".as_ref()];
            wildcard_labels.extend_from_slice(&labels[skip..]);

            if let Ok(wildcard_name) = Name::from_labels(wildcard_labels)
                && let Some(records) = self.lookup(&wildcard_name, rtype)
            {
                return Some(records);
            }
        }

        None
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

        Record::from_rdata(self.origin.clone(), self.soa.minimum, rdata)
    }

    /// Get all records in the zone for AXFR
    /// Returns records in canonical order: SOA, other records, SOA
    pub fn get_all_records(&self) -> Vec<Record> {
        let mut records = Vec::new();

        // Start with SOA
        records.push(self.get_soa_record());

        // Add all other records
        for record_map in self.records.values() {
            for record_vec in record_map.values() {
                for record in record_vec {
                    records.push(record.clone());
                }
            }
        }

        // End with SOA
        records.push(self.get_soa_record());

        records
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
    let content = std::fs::read_to_string(path.as_ref()).context("Failed to read zone file")?;

    let origin = Name::from_str(origin_name).context("Invalid origin name")?;

    let mut zone: Option<Zone> = None;
    let mut default_ttl: u32 = 3600;
    let mut current_origin = origin.clone();

    // Preprocess the content to handle multi-line records with parentheses
    let processed_lines = preprocess_zone_content(&content);

    for (line_num, line) in processed_lines.iter().enumerate() {
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
                    default_ttl = parts[1]
                        .parse()
                        .context(format!("Invalid $TTL on line {}", line_num + 1))?;
                }
            }
            continue;
        }

        // Parse resource record
        if let Some(record) = parse_resource_record(line, &current_origin, default_ttl, line_num)? {
            // If this is SOA and we don't have a zone yet, create it
            if record.record_type() == RecordType::SOA
                && zone.is_none()
                && let Some(soa_data) = extract_soa_data(&record)
            {
                zone = Some(Zone::new(origin.clone(), soa_data));
            }

            if let Some(ref mut z) = zone {
                z.add_record(record);
            }
        }
    }

    zone.ok_or_else(|| anyhow::anyhow!("Zone file must contain an SOA record"))
}

/// Preprocesses zone file content to handle multi-line records with parentheses
/// and strip inline comments
fn preprocess_zone_content(content: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut in_parens = false;

    for line in content.lines() {
        // Strip inline comments (but preserve the rest of the line)
        let line_without_comment = if let Some(pos) = line.find(';') {
            // Check if the semicolon is inside quotes (for TXT records, etc.)
            let before_semicolon = &line[..pos];
            let quote_count = before_semicolon.matches('"').count();

            // If odd number of quotes, semicolon is inside a string, keep it
            if quote_count % 2 == 1 {
                line
            } else {
                before_semicolon
            }
        } else {
            line
        };

        let trimmed = line_without_comment.trim();

        // Check for opening parenthesis
        if trimmed.contains('(') {
            in_parens = true;
            // Add everything before and including the paren to current line
            current_line.push_str(trimmed.replace('(', "").trim());
            current_line.push(' ');
            continue;
        }

        // Check for closing parenthesis
        if trimmed.contains(')') {
            in_parens = false;
            // Add everything before the closing paren and finalize the line
            current_line.push_str(trimmed.replace(')', "").trim());
            result.push(current_line.trim().to_string());
            current_line = String::new();
            continue;
        }

        // If we're inside parentheses, accumulate
        if in_parens {
            current_line.push_str(trimmed);
            current_line.push(' ');
        } else {
            // Normal line - add it directly (unless it's empty)
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
        }
    }

    // Handle case where parentheses weren't closed (add remaining content)
    if !current_line.is_empty() {
        result.push(current_line.trim().to_string());
    }

    result
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
    } else if parts[idx] == "*" {
        // Wildcard at zone apex: *.example.com.
        Name::from_str(&format!("*.{}", origin))
            .context(format!("Invalid wildcard name on line {}", line_num + 1))?
    } else if parts[idx].starts_with("*.") {
        // Wildcard with subdomain: *.sub.example.com
        if parts[idx].ends_with('.') {
            Name::from_str(parts[idx])
                .context(format!("Invalid wildcard name on line {}", line_num + 1))?
        } else {
            Name::from_str(&format!("{}.{}", parts[idx], origin))
                .context(format!("Invalid wildcard name on line {}", line_num + 1))?
        }
    } else if parts[idx].ends_with('.') {
        Name::from_str(parts[idx]).context(format!("Invalid name on line {}", line_num + 1))?
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
            let addr = parts[idx]
                .parse::<Ipv4Addr>()
                .context(format!("Invalid A record on line {}", line_num + 1))?;
            RData::A(hickory_proto::rr::rdata::A(addr))
        }
        "AAAA" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let addr = parts[idx]
                .parse::<Ipv6Addr>()
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
            let serial = parts[idx + 2]
                .parse()
                .context(format!("Invalid SOA serial on line {}", line_num + 1))?;
            let refresh = parts[idx + 3]
                .parse()
                .context(format!("Invalid SOA refresh on line {}", line_num + 1))?;
            let retry = parts[idx + 4]
                .parse()
                .context(format!("Invalid SOA retry on line {}", line_num + 1))?;
            let expire = parts[idx + 5]
                .parse()
                .context(format!("Invalid SOA expire on line {}", line_num + 1))?;
            let minimum = parts[idx + 6]
                .parse()
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
            let preference = parts[idx]
                .parse::<u16>()
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
            RData::TXT(hickory_proto::rr::rdata::TXT::new(vec![
                txt_data.to_string(),
            ]))
        }
        "PTR" => {
            if parts.len() <= idx {
                return Ok(None);
            }
            let ptrdname = parse_domain_name(parts[idx], origin)
                .context(format!("Invalid PTR record on line {}", line_num + 1))?;
            RData::PTR(hickory_proto::rr::rdata::PTR(ptrdname))
        }
        "SRV" => {
            if parts.len() < idx + 4 {
                return Ok(None);
            }
            let priority = parts[idx]
                .parse::<u16>()
                .context(format!("Invalid SRV priority on line {}", line_num + 1))?;
            let weight = parts[idx + 1]
                .parse::<u16>()
                .context(format!("Invalid SRV weight on line {}", line_num + 1))?;
            let port = parts[idx + 2]
                .parse::<u16>()
                .context(format!("Invalid SRV port on line {}", line_num + 1))?;
            let target = parse_domain_name(parts[idx + 3], origin)
                .context(format!("Invalid SRV target on line {}", line_num + 1))?;
            RData::SRV(hickory_proto::rr::rdata::SRV::new(
                priority, weight, port, target,
            ))
        }
        "CAA" => {
            if parts.len() < idx + 3 {
                return Ok(None);
            }
            let flags = parts[idx]
                .parse::<u8>()
                .context(format!("Invalid CAA flags on line {}", line_num + 1))?;
            let tag = parts[idx + 1].to_string();
            // Join remaining parts and remove quotes
            let value = parts[idx + 2..].join(" ");
            let value = value.trim_matches('"');

            // Create CAA record - for now, only support "issue" tag properly
            let caa = if tag == "issue" || tag == "issuewild" {
                if value.is_empty() || value == ";" {
                    hickory_proto::rr::rdata::CAA::new_issue(flags & 0x80 != 0, None, vec![])
                } else {
                    hickory_proto::rr::rdata::CAA::new_issue(
                        flags & 0x80 != 0,
                        Some(
                            hickory_proto::rr::Name::from_str(value)
                                .unwrap_or_else(|_| hickory_proto::rr::Name::root()),
                        ),
                        vec![],
                    )
                }
            } else {
                // For other tags, use a simple issue record
                hickory_proto::rr::rdata::CAA::new_issue(flags & 0x80 != 0, None, vec![])
            };
            RData::CAA(caa)
        }
        "DNSKEY" => {
            if parts.len() < idx + 4 {
                return Ok(None);
            }
            let flags = parts[idx]
                .parse::<u16>()
                .context(format!("Invalid DNSKEY flags on line {}", line_num + 1))?;
            let _protocol = parts[idx + 1]
                .parse::<u8>()
                .context(format!("Invalid DNSKEY protocol on line {}", line_num + 1))?;
            let algorithm = parts[idx + 2]
                .parse::<u8>()
                .context(format!("Invalid DNSKEY algorithm on line {}", line_num + 1))?;

            // Public key is base64 encoded, join remaining parts
            let public_key_b64 = parts[idx + 3..].join("");
            let public_key = match base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &public_key_b64,
            ) {
                Ok(key) => key,
                Err(_) => {
                    tracing::warn!("Invalid base64 in DNSKEY on line {}", line_num + 1);
                    return Ok(None);
                }
            };

            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                hickory_proto::rr::dnssec::rdata::DNSKEY::new(
                    flags & 0x0100 != 0, // zone key flag
                    flags & 0x0001 != 0, // secure entry point flag
                    flags & 0x8000 != 0, // revoke flag
                    hickory_proto::rr::dnssec::Algorithm::from_u8(algorithm),
                    public_key,
                ),
            ))
        }
        "RRSIG" => {
            // RRSIG: type_covered algorithm labels original_ttl sig_expiration sig_inception key_tag signer_name signature
            if parts.len() < idx + 9 {
                return Ok(None);
            }

            let type_covered = RecordType::from_str(parts[idx]).context(format!(
                "Invalid RRSIG type_covered on line {}",
                line_num + 1
            ))?;
            let algorithm = parts[idx + 1]
                .parse::<u8>()
                .context(format!("Invalid RRSIG algorithm on line {}", line_num + 1))?;
            let labels = parts[idx + 2]
                .parse::<u8>()
                .context(format!("Invalid RRSIG labels on line {}", line_num + 1))?;
            let original_ttl = parts[idx + 3].parse::<u32>().context(format!(
                "Invalid RRSIG original_ttl on line {}",
                line_num + 1
            ))?;
            let sig_expiration = parts[idx + 4].parse::<u32>().context(format!(
                "Invalid RRSIG sig_expiration on line {}",
                line_num + 1
            ))?;
            let sig_inception = parts[idx + 5].parse::<u32>().context(format!(
                "Invalid RRSIG sig_inception on line {}",
                line_num + 1
            ))?;
            let key_tag = parts[idx + 6]
                .parse::<u16>()
                .context(format!("Invalid RRSIG key_tag on line {}", line_num + 1))?;
            let signer_name = parse_domain_name(parts[idx + 7], origin).context(format!(
                "Invalid RRSIG signer_name on line {}",
                line_num + 1
            ))?;

            // Signature is base64 encoded, join remaining parts
            let signature_b64 = parts[idx + 8..].join("");
            let signature = match base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &signature_b64,
            ) {
                Ok(sig) => sig,
                Err(_) => {
                    tracing::warn!("Invalid base64 in RRSIG on line {}", line_num + 1);
                    return Ok(None);
                }
            };

            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(
                hickory_proto::rr::dnssec::rdata::SIG::new(
                    type_covered,
                    hickory_proto::rr::dnssec::Algorithm::from_u8(algorithm),
                    labels,
                    original_ttl,
                    sig_expiration,
                    sig_inception,
                    key_tag,
                    signer_name,
                    signature,
                ),
            ))
        }
        "NSEC" => {
            // NSEC: next_domain_name type_bit_maps
            if parts.len() < idx + 2 {
                return Ok(None);
            }

            let next_domain_name = parse_domain_name(parts[idx], origin).context(format!(
                "Invalid NSEC next_domain_name on line {}",
                line_num + 1
            ))?;

            // Parse type bit maps - simplified version, just parse the record types
            let mut type_bit_maps = Vec::new();
            for part in &parts[idx + 1..] {
                if let Ok(rtype) = RecordType::from_str(part) {
                    type_bit_maps.push(rtype);
                }
            }

            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::NSEC(
                hickory_proto::rr::dnssec::rdata::NSEC::new(next_domain_name, type_bit_maps),
            ))
        }
        "DS" => {
            // DS: key_tag algorithm digest_type digest
            if parts.len() < idx + 4 {
                return Ok(None);
            }

            let key_tag = parts[idx]
                .parse::<u16>()
                .context(format!("Invalid DS key_tag on line {}", line_num + 1))?;
            let algorithm = parts[idx + 1]
                .parse::<u8>()
                .context(format!("Invalid DS algorithm on line {}", line_num + 1))?;
            let digest_type = parts[idx + 2]
                .parse::<u8>()
                .context(format!("Invalid DS digest_type on line {}", line_num + 1))?;

            // Digest is hex encoded
            let digest_hex = parts[idx + 3..].join("");
            let digest = match hex::decode(&digest_hex) {
                Ok(d) => d,
                Err(_) => {
                    tracing::warn!("Invalid hex in DS on line {}", line_num + 1);
                    return Ok(None);
                }
            };

            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DS(
                hickory_proto::rr::dnssec::rdata::DS::new(
                    key_tag,
                    hickory_proto::rr::dnssec::Algorithm::from_u8(algorithm),
                    hickory_proto::rr::dnssec::DigestType::from_u8(digest_type)?,
                    digest,
                ),
            ))
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

    #[test]
    fn test_wildcard_lookup() {
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

        let mut zone = Zone::new(origin.clone(), soa);

        // Add a wildcard A record
        let wildcard_name = Name::from_str("*.example.com.").unwrap();
        let wildcard_record = Record::from_rdata(
            wildcard_name.clone(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 100))),
        );
        zone.add_record(wildcard_record);

        // Add a specific record that should override wildcard
        let www_name = Name::from_str("www.example.com.").unwrap();
        let www_record = Record::from_rdata(
            www_name.clone(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 10))),
        );
        zone.add_record(www_record);

        // Test direct wildcard lookup
        let wildcard_result = zone.lookup(&wildcard_name, RecordType::A);
        assert!(wildcard_result.is_some());
        assert_eq!(wildcard_result.unwrap().len(), 1);

        // Test wildcard match for non-existent name
        let random_name = Name::from_str("random.example.com.").unwrap();
        let wildcard_match = zone.lookup_wildcard(&random_name, RecordType::A);
        assert!(wildcard_match.is_some());
        assert_eq!(wildcard_match.unwrap().len(), 1);

        // Test that specific record exists (should NOT use wildcard)
        let www_result = zone.lookup(&www_name, RecordType::A);
        assert!(www_result.is_some());
        assert_eq!(www_result.unwrap().len(), 1);

        // Verify www returns different IP than wildcard
        if let Some(RData::A(a)) = www_result.unwrap()[0].data() {
            assert_eq!(a.0, Ipv4Addr::new(192, 0, 2, 10));
        }
    }

    #[test]
    fn test_dnskey_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary zone file with DNSKEY record
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1").unwrap();
        // DNSKEY with simple base64 key for testing
        writeln!(
            temp_file,
            "@ IN DNSKEY 256 3 8 AwEAAaetidLzsKWUt4swWR8yu0wPHPiUi8LU"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Verify DNSKEY was parsed
        let dnskey_records =
            zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::DNSKEY);
        assert!(dnskey_records.is_some(), "DNSKEY record should be parsed");
        assert_eq!(
            dnskey_records.unwrap().len(),
            1,
            "Should have one DNSKEY record"
        );

        // Verify it's actually a DNSSEC record
        if let Some(rdata) = dnskey_records.unwrap()[0].data() {
            assert!(matches!(rdata, RData::DNSSEC(_)), "Should be DNSSEC RData");
        }
    }

    #[test]
    fn test_rrsig_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary zone file with RRSIG record
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1").unwrap();
        // RRSIG: type_covered algorithm labels original_ttl sig_expiration sig_inception key_tag signer_name signature
        // sig_expiration and sig_inception are Unix timestamps (u32)
        writeln!(
            temp_file,
            "@ IN RRSIG A 8 2 3600 1767139200 1764547200 12345 example.com. AwEAAaetidLzsKWU"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Verify RRSIG was parsed (stored as SIG in hickory-proto)
        let rrsig_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::SIG);
        assert!(rrsig_records.is_some(), "RRSIG record should be parsed");
        assert_eq!(
            rrsig_records.unwrap().len(),
            1,
            "Should have one RRSIG record"
        );

        // Verify it's actually a DNSSEC record
        if let Some(rdata) = rrsig_records.unwrap()[0].data() {
            assert!(matches!(rdata, RData::DNSSEC(_)), "Should be DNSSEC RData");
        }
    }

    #[test]
    fn test_nsec_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary zone file with NSEC record
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1").unwrap();
        // NSEC: next_domain_name type_bit_maps...
        writeln!(
            temp_file,
            "@ IN NSEC www.example.com. A NS SOA RRSIG NSEC DNSKEY"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Verify NSEC was parsed
        let nsec_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::NSEC);
        assert!(nsec_records.is_some(), "NSEC record should be parsed");
        assert_eq!(
            nsec_records.unwrap().len(),
            1,
            "Should have one NSEC record"
        );

        // Verify it's actually a DNSSEC record
        if let Some(rdata) = nsec_records.unwrap()[0].data() {
            assert!(matches!(rdata, RData::DNSSEC(_)), "Should be DNSSEC RData");
        }
    }

    #[test]
    fn test_ds_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary zone file with DS record
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1").unwrap();
        // DS: key_tag algorithm digest_type digest_hex
        writeln!(
            temp_file,
            "@ IN DS 12345 8 2 A8B1C2D3E4F506172839405A6B7C8D9E0F1A2B3C4D5E6F70"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Verify DS was parsed
        let ds_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::DS);
        assert!(ds_records.is_some(), "DS record should be parsed");
        assert_eq!(ds_records.unwrap().len(), 1, "Should have one DS record");

        // Verify it's actually a DNSSEC record
        if let Some(rdata) = ds_records.unwrap()[0].data() {
            assert!(matches!(rdata, RData::DNSSEC(_)), "Should be DNSSEC RData");
        }
    }

    #[test]
    fn test_malformed_zone_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Missing SOA record
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let result = parse_zone_file(temp_file.path(), "example.com.");
        assert!(result.is_err(), "Should fail without SOA record");
    }

    #[test]
    fn test_invalid_ttl() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL invalid").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        temp_file.flush().unwrap();

        let result = parse_zone_file(temp_file.path(), "example.com.");
        assert!(result.is_err(), "Should fail with invalid TTL");
    }

    #[test]
    fn test_invalid_record_data() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN A invalid.ip.address").unwrap();
        temp_file.flush().unwrap();

        let result = parse_zone_file(temp_file.path(), "example.com.");
        // Invalid A record causes parse error
        assert!(result.is_err(), "Should fail with invalid A record");
    }

    #[test]
    fn test_malformed_soa() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        // SOA with insufficient fields
        writeln!(temp_file, "@ IN SOA ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let result = parse_zone_file(temp_file.path(), "example.com.");
        assert!(result.is_err(), "Should fail with malformed SOA");
    }

    #[test]
    fn test_empty_zone_file() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let result = parse_zone_file(temp_file.path(), "example.com.");
        assert!(result.is_err(), "Should fail with empty zone file");
    }

    #[test]
    fn test_invalid_base64_in_dnskey() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        // Invalid base64 in DNSKEY
        writeln!(temp_file, "@ IN DNSKEY 256 3 8 !!!INVALID_BASE64!!!").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        // Invalid DNSKEY should be skipped
        let dnskey_records =
            zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::DNSKEY);
        assert!(dnskey_records.is_none(), "Invalid DNSKEY should be skipped");
    }

    #[test]
    fn test_invalid_hex_in_ds() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        // Invalid hex in DS digest
        writeln!(temp_file, "@ IN DS 12345 8 2 ZZZZZZ").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        // Invalid DS should be skipped
        let ds_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::DS);
        assert!(ds_records.is_none(), "Invalid DS should be skipped");
    }

    #[test]
    fn test_very_long_domain_name() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        // Very long label (>63 characters, which violates DNS spec)
        let long_label = "a".repeat(70);
        writeln!(temp_file, "{} IN A 192.0.2.1", long_label).unwrap();
        temp_file.flush().unwrap();

        let result = parse_zone_file(temp_file.path(), "example.com.");
        // Very long labels cause parse errors
        assert!(result.is_err(), "Should fail with very long domain label");
    }

    #[test]
    fn test_wildcard_with_specific_override() {
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

        let mut zone = Zone::new(origin.clone(), soa);

        // Add wildcard
        let wildcard_name = Name::from_str("*.example.com.").unwrap();
        let wildcard_record = Record::from_rdata(
            wildcard_name.clone(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 100))),
        );
        zone.add_record(wildcard_record);

        // Add specific record
        let specific_name = Name::from_str("www.example.com.").unwrap();
        let specific_record = Record::from_rdata(
            specific_name.clone(),
            3600,
            RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(192, 0, 2, 10))),
        );
        zone.add_record(specific_record);

        // Test that specific overrides wildcard
        let specific_result = zone.lookup(&specific_name, RecordType::A);
        assert!(specific_result.is_some());
        if let Some(RData::A(a)) = specific_result.unwrap()[0].data() {
            assert_eq!(
                a.0,
                Ipv4Addr::new(192, 0, 2, 10),
                "Specific record should override wildcard"
            );
        }

        // Test wildcard match
        let random_name = Name::from_str("random.example.com.").unwrap();
        let wildcard_result = zone.lookup_wildcard(&random_name, RecordType::A);
        assert!(wildcard_result.is_some());
        if let Some(RData::A(a)) = wildcard_result.unwrap()[0].data() {
            assert_eq!(a.0, Ipv4Addr::new(192, 0, 2, 100), "Should match wildcard");
        }
    }

    #[test]
    fn test_nonexistent_record_type() {
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

        let zone = Zone::new(origin.clone(), soa);

        // Query for record type that doesn't exist
        let result = zone.lookup(&origin, RecordType::MX);
        assert!(
            result.is_none(),
            "Should return None for non-existent record type"
        );
    }

    #[test]
    fn test_caa_record_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN CAA 0 issue \"letsencrypt.org\"").unwrap();
        writeln!(temp_file, "@ IN CAA 0 issuewild \"letsencrypt.org\"").unwrap();
        writeln!(temp_file, "@ IN CAA 0 iodef \"mailto:admin@example.com\"").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        let caa_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::CAA);
        assert!(caa_records.is_some());
        assert_eq!(caa_records.unwrap().len(), 3);
    }

    #[test]
    fn test_ptr_record_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN 2.0.192.in-addr.arpa.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "1 IN PTR www.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "2.0.192.in-addr.arpa.").unwrap();
        let ptr_records = zone.lookup(
            &Name::from_str("1.2.0.192.in-addr.arpa.").unwrap(),
            RecordType::PTR,
        );
        assert!(ptr_records.is_some());
    }

    #[test]
    fn test_srv_record_parsing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "_http._tcp IN SRV 10 60 80 www.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        let srv_records = zone.lookup(
            &Name::from_str("_http._tcp.example.com.").unwrap(),
            RecordType::SRV,
        );
        assert!(srv_records.is_some());
    }

    #[test]
    fn test_multiline_soa_with_parentheses() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    2024010101  ; Serial").unwrap();
        writeln!(temp_file, "    7200        ; Refresh").unwrap();
        writeln!(temp_file, "    3600        ; Retry").unwrap();
        writeln!(temp_file, "    1209600     ; Expire").unwrap();
        writeln!(temp_file, "    86400       ; Minimum TTL").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Verify SOA was parsed correctly
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
        assert_eq!(zone.soa.retry, 3600);
        assert_eq!(zone.soa.expire, 1209600);
        assert_eq!(zone.soa.minimum, 86400);

        // Verify NS record was also parsed
        let ns_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::NS);
        assert!(ns_records.is_some());
    }

    #[test]
    fn test_multiline_soa_without_inline_comments() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    2024010101").unwrap();
        writeln!(temp_file, "    7200").unwrap();
        writeln!(temp_file, "    3600").unwrap();
        writeln!(temp_file, "    1209600").unwrap();
        writeln!(temp_file, "    86400").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
    }

    #[test]
    fn test_inline_comments_stripped() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600 ; default TTL").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400 ; SOA record"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com. ; nameserver").unwrap();
        writeln!(temp_file, "ns1 IN A 192.0.2.1 ; nameserver IP").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();

        // Should parse successfully despite comments
        let ns_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::NS);
        assert!(ns_records.is_some());
    }

    #[test]
    fn test_preprocess_zone_content() {
        let content = r#"$ORIGIN example.com.
$TTL 3600
@ IN SOA ns1.example.com. admin.example.com. (
    2024010101  ; Serial
    7200        ; Refresh
    3600        ; Retry
    1209600     ; Expire
    86400       ; Minimum TTL
)
@ IN NS ns1.example.com.
"#;

        let processed = preprocess_zone_content(content);

        // Should have 4 lines: $ORIGIN, $TTL, SOA (merged), NS
        assert_eq!(processed.len(), 4);

        // SOA line should be merged
        assert!(processed[2].contains("SOA"));
        assert!(processed[2].contains("2024010101"));
        assert!(processed[2].contains("86400"));

        // Comments should be stripped
        assert!(!processed[2].contains(";"));
    }

    #[test]
    fn test_soa_single_line_format() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 2024010101 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
        assert_eq!(zone.soa.retry, 3600);
        assert_eq!(zone.soa.expire, 1209600);
        assert_eq!(zone.soa.minimum, 86400);
    }

    #[test]
    fn test_soa_multiline_compact() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // All values on one line within parentheses
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    2024010101 7200 3600 1209600 86400").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
    }

    #[test]
    fn test_soa_multiline_each_field_separate_line() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Each field on its own line
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    2024010101").unwrap();
        writeln!(temp_file, "    7200").unwrap();
        writeln!(temp_file, "    3600").unwrap();
        writeln!(temp_file, "    1209600").unwrap();
        writeln!(temp_file, "    86400").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.minimum, 86400);
    }

    #[test]
    fn test_soa_multiline_mixed_grouping() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Some fields grouped, some separate
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    2024010101 7200").unwrap();
        writeln!(temp_file, "    3600").unwrap();
        writeln!(temp_file, "    1209600 86400").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
        assert_eq!(zone.soa.retry, 3600);
        assert_eq!(zone.soa.expire, 1209600);
        assert_eq!(zone.soa.minimum, 86400);
    }

    #[test]
    fn test_soa_with_inline_comments() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. ( ; Start SOA"
        )
        .unwrap();
        writeln!(temp_file, "    2024010101  ; Serial number").unwrap();
        writeln!(temp_file, "    7200        ; Refresh - 2 hours").unwrap();
        writeln!(temp_file, "    3600        ; Retry - 1 hour").unwrap();
        writeln!(temp_file, "    1209600     ; Expire - 2 weeks").unwrap();
        writeln!(temp_file, "    86400       ; Minimum TTL - 1 day").unwrap();
        writeln!(temp_file, ") ; End SOA").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com. ; Primary NS").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
        assert_eq!(zone.soa.retry, 3600);
        assert_eq!(zone.soa.expire, 1209600);
        assert_eq!(zone.soa.minimum, 86400);
    }

    #[test]
    fn test_soa_single_line_with_comment() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 2024010101 7200 3600 1209600 86400 ; Main SOA"
        )
        .unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
    }

    #[test]
    fn test_soa_with_tabs_and_spaces() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@\tIN\tSOA\tns1.example.com.\tadmin.example.com.\t("
        )
        .unwrap();
        writeln!(temp_file, "\t\t2024010101\t\t; Serial").unwrap();
        writeln!(temp_file, "        7200          ; Refresh").unwrap();
        writeln!(temp_file, "\t3600\t\t; Retry").unwrap();
        writeln!(temp_file, "    1209600    ; Expire").unwrap();
        writeln!(temp_file, "\t86400\t; Minimum").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
        assert_eq!(zone.soa.retry, 3600);
    }

    #[test]
    fn test_soa_opening_paren_on_same_line_as_name() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Opening paren on same line as mname and rname
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. ( 2024010101"
        )
        .unwrap();
        writeln!(temp_file, "    7200 3600").unwrap();
        writeln!(temp_file, "    1209600 86400 )").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.minimum, 86400);
    }

    #[test]
    fn test_soa_multiple_blank_lines_in_parens() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "    2024010101").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "    7200 3600").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "    1209600 86400").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
    }

    #[test]
    fn test_soa_comment_only_lines_in_parens() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(temp_file, "@ IN SOA ns1.example.com. admin.example.com. (").unwrap();
        writeln!(temp_file, "    ; This is the serial number").unwrap();
        writeln!(temp_file, "    2024010101").unwrap();
        writeln!(temp_file, "    ; Time values below").unwrap();
        writeln!(temp_file, "    7200").unwrap();
        writeln!(temp_file, "    3600").unwrap();
        writeln!(temp_file, "    1209600").unwrap();
        writeln!(temp_file, "    86400").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        assert_eq!(zone.soa.serial, 2024010101);
        assert_eq!(zone.soa.refresh, 7200);
    }

    #[test]
    fn test_multiline_mx_record() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Test that multiline format works for other record types too
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "$ORIGIN example.com.").unwrap();
        writeln!(temp_file, "$TTL 3600").unwrap();
        writeln!(
            temp_file,
            "@ IN SOA ns1.example.com. admin.example.com. 1 7200 3600 1209600 86400"
        )
        .unwrap();
        writeln!(temp_file, "@ IN MX (").unwrap();
        writeln!(temp_file, "    10").unwrap();
        writeln!(temp_file, "    mail.example.com.").unwrap();
        writeln!(temp_file, ")").unwrap();
        writeln!(temp_file, "@ IN NS ns1.example.com.").unwrap();
        temp_file.flush().unwrap();

        let zone = parse_zone_file(temp_file.path(), "example.com.").unwrap();
        let mx_records = zone.lookup(&Name::from_str("example.com.").unwrap(), RecordType::MX);
        assert!(mx_records.is_some());
        assert_eq!(mx_records.unwrap().len(), 1);
    }
}
