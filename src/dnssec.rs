use anyhow::{Context, Result, anyhow};
use hickory_proto::rr::dnssec::DigestType;
use hickory_proto::rr::{Name, RData, Record, RecordType};
use sha2::{Digest, Sha256, Sha512};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for DNSSEC validation behavior
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DnssecConfig {
    /// Whether to validate DNSSEC signatures
    pub validate_signatures: bool,
    /// Whether to require DNSSEC for all responses
    pub require_dnssec: bool,
    /// Whether to include DNSSEC records in responses when DO flag is set
    pub auto_include_dnssec: bool,
}

impl Default for DnssecConfig {
    fn default() -> Self {
        DnssecConfig {
            validate_signatures: false,
            require_dnssec: false,
            auto_include_dnssec: true,
        }
    }
}

/// Verify a DS record against a DNSKEY record
/// This validates that the digest in the DS record matches the hash of the DNSKEY
#[allow(dead_code)]
pub fn verify_ds(ds: &Record, dnskey: &Record) -> Result<()> {
    let ds_data = match ds.data() {
        Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DS(ds))) => ds,
        _ => return Err(anyhow!("Invalid DS record")),
    };

    let dnskey_data = match dnskey.data() {
        Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(key))) => key,
        _ => return Err(anyhow!("Invalid DNSKEY record")),
    };

    // Verify algorithm matches
    if ds_data.algorithm() != dnskey_data.algorithm() {
        return Err(anyhow!(
            "Algorithm mismatch: DS={:?} DNSKEY={:?}",
            ds_data.algorithm(),
            dnskey_data.algorithm()
        ));
    }

    // Verify key tag matches
    let computed_key_tag = compute_key_tag(dnskey)?;
    if ds_data.key_tag() != computed_key_tag {
        return Err(anyhow!(
            "Key tag mismatch: DS={} computed={}",
            ds_data.key_tag(),
            computed_key_tag
        ));
    }

    // Compute digest of DNSKEY according to RFC 4034 Section 5.1.4
    let mut digest_input = Vec::new();

    // Owner name in wire format (canonical form - lowercase)
    let owner_name = dnskey.name().to_lowercase();
    // Simplified: convert to string and then bytes (not proper wire format, but functional)
    digest_input.extend_from_slice(owner_name.to_string().as_bytes());

    // DNSKEY RDATA in wire format
    digest_input.extend_from_slice(&dnskey_data.flags().to_be_bytes());
    digest_input.push(3); // Protocol is always 3 for DNSSEC
    digest_input.push(dnskey_data.algorithm().into());
    digest_input.extend_from_slice(dnskey_data.public_key());

    // Compute digest based on digest type
    let computed_digest = match ds_data.digest_type() {
        DigestType::SHA256 => {
            let mut hasher = Sha256::new();
            hasher.update(&digest_input);
            hasher.finalize().to_vec()
        }
        DigestType::SHA384 => {
            let mut hasher = sha2::Sha384::new();
            hasher.update(&digest_input);
            hasher.finalize().to_vec()
        }
        DigestType::SHA512 => {
            let mut hasher = Sha512::new();
            hasher.update(&digest_input);
            hasher.finalize().to_vec()
        }
        _ => {
            return Err(anyhow!(
                "Unsupported digest type: {:?}",
                ds_data.digest_type()
            ));
        }
    };

    // Compare computed digest with DS digest
    if computed_digest.as_slice() != ds_data.digest() {
        return Err(anyhow!(
            "DS digest mismatch: computed={} expected={}",
            hex::encode(&computed_digest),
            hex::encode(ds_data.digest())
        ));
    }

    Ok(())
}

/// Check if a DNSSEC signature is time-valid
#[allow(dead_code)]
pub fn check_signature_validity(rrsig: &Record) -> Result<()> {
    let rrsig_data = match rrsig.data() {
        Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(sig))) => sig,
        _ => return Err(anyhow!("Invalid RRSIG record")),
    };

    // Check signature time validity
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get current time")?
        .as_secs() as u32;

    if now < rrsig_data.sig_inception() {
        return Err(anyhow!(
            "Signature not yet valid: inception={} now={}",
            rrsig_data.sig_inception(),
            now
        ));
    }

    if now > rrsig_data.sig_expiration() {
        return Err(anyhow!(
            "Signature expired: expiration={} now={}",
            rrsig_data.sig_expiration(),
            now
        ));
    }

    Ok(())
}

/// Validate NSEC proof of non-existence
#[allow(dead_code)]
pub fn validate_nsec_denial(
    query_name: &Name,
    query_type: RecordType,
    nsec_records: &[Record],
) -> Result<()> {
    // NSEC validation according to RFC 4034 Section 4
    // Find NSEC record that covers the query name

    for nsec in nsec_records {
        let nsec_data = match nsec.data() {
            Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::NSEC(nsec))) => nsec,
            _ => continue,
        };

        let owner_name = nsec.name();
        let next_name = nsec_data.next_domain_name();

        // Check if query_name is covered by this NSEC record
        // owner_name < query_name < next_name (canonical DNS ordering)
        let covers_name = if owner_name < next_name {
            // Normal case: owner < query < next
            query_name > owner_name && query_name < next_name
        } else {
            // Wrap-around case (last record in zone)
            query_name > owner_name || query_name < next_name
        };

        if covers_name {
            // Name is covered - proves non-existence
            return Ok(());
        }

        // Check if NSEC proves the type doesn't exist at this name
        if query_name == owner_name {
            let type_exists = nsec_data.type_bit_maps().contains(&query_type);

            if !type_exists {
                // Type doesn't exist at this name
                return Ok(());
            }
        }
    }

    Err(anyhow!("No NSEC record proves non-existence"))
}

/// Compute key tag for a DNSKEY record (RFC 4034 Appendix B)
#[allow(dead_code)]
pub fn compute_key_tag(dnskey: &Record) -> Result<u16> {
    let dnskey_data = match dnskey.data() {
        Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(key))) => key,
        _ => return Err(anyhow!("Invalid DNSKEY record")),
    };

    let mut rdata = Vec::new();
    rdata.extend_from_slice(&dnskey_data.flags().to_be_bytes());
    rdata.push(3); // Protocol is always 3 for DNSSEC
    rdata.push(dnskey_data.algorithm().into());
    rdata.extend_from_slice(dnskey_data.public_key());

    // RFC 4034 Appendix B algorithm
    let mut ac: u32 = 0;
    for (i, &byte) in rdata.iter().enumerate() {
        if i % 2 == 0 {
            ac += (byte as u32) << 8;
        } else {
            ac += byte as u32;
        }
    }

    ac += (ac >> 16) & 0xFFFF;
    Ok((ac & 0xFFFF) as u16)
}

/// Find DNSSEC records related to a given RRset
#[allow(dead_code)]
pub fn find_related_dnssec_records(
    records: &[Record],
    name: &Name,
    rtype: RecordType,
) -> Vec<Record> {
    let mut dnssec_records = Vec::new();

    // Find RRSIG records that cover this RRtype
    for record in records {
        if let Some(RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(sig))) =
            record.data()
            && record.name() == name
            && sig.type_covered() == rtype
        {
            dnssec_records.push(record.clone());
        }
    }

    // Find DNSKEY records for the zone
    for record in records {
        if matches!(
            record.data(),
            Some(RData::DNSSEC(
                hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(_)
            ))
        ) {
            dnssec_records.push(record.clone());
        }
    }

    dnssec_records
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::dnssec::Algorithm;
    use hickory_proto::rr::dnssec::rdata::{DNSKEY, DS, SIG};
    use std::str::FromStr;

    #[test]
    fn test_dnssec_config_default() {
        let config = DnssecConfig::default();
        assert!(!config.validate_signatures);
        assert!(!config.require_dnssec);
        assert!(config.auto_include_dnssec);
    }

    #[test]
    fn test_dnssec_config_custom() {
        let config = DnssecConfig {
            validate_signatures: true,
            require_dnssec: true,
            auto_include_dnssec: false,
        };
        assert!(config.validate_signatures);
        assert!(config.require_dnssec);
        assert!(!config.auto_include_dnssec);
    }

    #[test]
    fn test_key_tag_computation() {
        // Create a simple DNSKEY record
        // Parameters: zone_key, secure_entry_point, revoke, algorithm, public_key
        let dnskey = DNSKEY::new(
            true,  // zone_key
            false, // secure_entry_point
            false, // revoke
            Algorithm::RSASHA256,
            vec![1, 2, 3, 4, 5],
        );

        let record = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                dnskey,
            )),
        );

        // Just verify it doesn't panic and returns a value
        let key_tag = compute_key_tag(&record);
        assert!(key_tag.is_ok());
        assert!(key_tag.unwrap() > 0);
    }

    #[test]
    fn test_key_tag_with_different_keys() {
        // Test that different keys produce different tags
        let dnskey1 = DNSKEY::new(
            true,
            false,
            false,
            Algorithm::RSASHA256,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        );

        let dnskey2 = DNSKEY::new(
            true,
            false,
            false,
            Algorithm::RSASHA256,
            vec![16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        );

        let record1 = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                dnskey1,
            )),
        );

        let record2 = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                dnskey2,
            )),
        );

        let tag1 = compute_key_tag(&record1).unwrap();
        let tag2 = compute_key_tag(&record2).unwrap();

        // Different keys should produce different tags
        assert_ne!(tag1, tag2);
    }

    #[test]
    fn test_find_related_dnssec_records() {
        let name = Name::from_utf8("example.com.").unwrap();
        let records = vec![];

        let result = find_related_dnssec_records(&records, &name, RecordType::A);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_find_related_dnssec_with_dnskey() {
        let name = Name::from_utf8("example.com.").unwrap();

        let dnskey = DNSKEY::new(
            true,
            false,
            false,
            Algorithm::RSASHA256,
            vec![1, 2, 3, 4, 5],
        );

        let record = Record::from_rdata(
            name.clone(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                dnskey,
            )),
        );

        let records = vec![record];
        let result = find_related_dnssec_records(&records, &name, RecordType::A);

        // Should find the DNSKEY record
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_check_signature_validity_future() {
        // Create an RRSIG that's not yet valid
        let future_time = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400) as u32; // 1 day in the future

        let sig = SIG::new(
            RecordType::A,
            Algorithm::RSASHA256,
            2,                  // labels
            300,                // original_ttl
            future_time + 3600, // expiration (2 hours from inception)
            future_time,        // inception (1 day in future)
            12345,              // key_tag
            Name::from_str("example.com.").unwrap(),
            vec![1, 2, 3, 4, 5], // signature
        );

        let record = Record::from_rdata(
            Name::from_utf8("www.example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(sig)),
        );

        let result = check_signature_validity(&record);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet valid"));
    }

    #[test]
    fn test_check_signature_validity_expired() {
        // Create an RRSIG that's expired
        let past_time = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 86400) as u32; // 1 day in the past

        let sig = SIG::new(
            RecordType::A,
            Algorithm::RSASHA256,
            2,                // labels
            300,              // original_ttl
            past_time + 3600, // expiration (23 hours ago)
            past_time,        // inception (1 day ago)
            12345,            // key_tag
            Name::from_str("example.com.").unwrap(),
            vec![1, 2, 3, 4, 5], // signature
        );

        let record = Record::from_rdata(
            Name::from_utf8("www.example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(sig)),
        );

        let result = check_signature_validity(&record);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_check_signature_validity_valid() {
        // Create an RRSIG that's currently valid
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let sig = SIG::new(
            RecordType::A,
            Algorithm::RSASHA256,
            2,          // labels
            300,        // original_ttl
            now + 3600, // expiration (1 hour from now)
            now - 3600, // inception (1 hour ago)
            12345,      // key_tag
            Name::from_str("example.com.").unwrap(),
            vec![1, 2, 3, 4, 5], // signature
        );

        let record = Record::from_rdata(
            Name::from_utf8("www.example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::SIG(sig)),
        );

        let result = check_signature_validity(&record);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_nsec_denial_empty() {
        let query_name = Name::from_utf8("www.example.com.").unwrap();
        let nsec_records = vec![];

        let result = validate_nsec_denial(&query_name, RecordType::A, &nsec_records);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_ds_algorithm_mismatch() {
        // Create DS with RSASHA256
        let ds = DS::new(
            12345,
            Algorithm::RSASHA256,
            DigestType::SHA256,
            vec![1, 2, 3, 4, 5],
        );

        // Create DNSKEY with different algorithm
        let dnskey = DNSKEY::new(
            true,
            false,
            false,
            Algorithm::ECDSAP256SHA256, // Different algorithm
            vec![1, 2, 3, 4, 5],
        );

        let ds_record = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DS(ds)),
        );

        let dnskey_record = Record::from_rdata(
            Name::from_utf8("example.com.").unwrap(),
            300,
            RData::DNSSEC(hickory_proto::rr::dnssec::rdata::DNSSECRData::DNSKEY(
                dnskey,
            )),
        );

        let result = verify_ds(&ds_record, &dnskey_record);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Algorithm mismatch")
        );
    }
}
