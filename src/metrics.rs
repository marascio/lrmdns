use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Metrics {
    // Query counts
    pub total_queries: AtomicU64,
    pub udp_queries: AtomicU64,
    pub tcp_queries: AtomicU64,
    pub edns_queries: AtomicU64,

    // Response codes
    pub noerror_responses: AtomicU64,
    pub nxdomain_responses: AtomicU64,
    pub servfail_responses: AtomicU64,
    pub refused_responses: AtomicU64,
    pub formerr_responses: AtomicU64,

    // Query types
    query_types: RwLock<HashMap<RecordType, u64>>,

    // Performance metrics
    pub total_latency_us: AtomicU64,
    pub min_latency_us: AtomicU64,
    pub max_latency_us: AtomicU64,

    // Rate limiting
    pub rate_limited: AtomicU64,

    // Errors
    pub errors: AtomicU64,

    // Start time
    start_time: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Metrics {
            total_queries: AtomicU64::new(0),
            udp_queries: AtomicU64::new(0),
            tcp_queries: AtomicU64::new(0),
            edns_queries: AtomicU64::new(0),
            noerror_responses: AtomicU64::new(0),
            nxdomain_responses: AtomicU64::new(0),
            servfail_responses: AtomicU64::new(0),
            refused_responses: AtomicU64::new(0),
            formerr_responses: AtomicU64::new(0),
            query_types: RwLock::new(HashMap::new()),
            total_latency_us: AtomicU64::new(0),
            min_latency_us: AtomicU64::new(u64::MAX),
            max_latency_us: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn record_query(&self, protocol: Protocol, edns: bool) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);

        match protocol {
            Protocol::Udp => self.udp_queries.fetch_add(1, Ordering::Relaxed),
            Protocol::Tcp => self.tcp_queries.fetch_add(1, Ordering::Relaxed),
        };

        if edns {
            self.edns_queries.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_response(&self, response_code: ResponseCode) {
        match response_code {
            ResponseCode::NoError => {
                self.noerror_responses.fetch_add(1, Ordering::Relaxed);
            }
            ResponseCode::NXDomain => {
                self.nxdomain_responses.fetch_add(1, Ordering::Relaxed);
            }
            ResponseCode::ServFail => {
                self.servfail_responses.fetch_add(1, Ordering::Relaxed);
            }
            ResponseCode::Refused => {
                self.refused_responses.fetch_add(1, Ordering::Relaxed);
            }
            ResponseCode::FormErr => {
                self.formerr_responses.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub fn record_query_type(&self, qtype: RecordType) {
        let mut types = self.query_types.write().unwrap();
        *types.entry(qtype).or_insert(0) += 1;
    }

    pub fn record_latency(&self, latency: Duration) {
        let latency_us = latency.as_micros() as u64;

        self.total_latency_us
            .fetch_add(latency_us, Ordering::Relaxed);

        // Update min latency
        self.min_latency_us.fetch_min(latency_us, Ordering::Relaxed);

        // Update max latency
        self.max_latency_us.fetch_max(latency_us, Ordering::Relaxed);
    }

    pub fn record_rate_limited(&self) {
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_snapshot(&self) -> MetricsSnapshot {
        let total = self.total_queries.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);

        let avg_latency_us = if total > 0 {
            total_latency / total
        } else {
            0
        };

        let min_latency = self.min_latency_us.load(Ordering::Relaxed);
        let min_latency_us = if min_latency == u64::MAX {
            0
        } else {
            min_latency
        };

        MetricsSnapshot {
            total_queries: total,
            udp_queries: self.udp_queries.load(Ordering::Relaxed),
            tcp_queries: self.tcp_queries.load(Ordering::Relaxed),
            edns_queries: self.edns_queries.load(Ordering::Relaxed),
            noerror_responses: self.noerror_responses.load(Ordering::Relaxed),
            nxdomain_responses: self.nxdomain_responses.load(Ordering::Relaxed),
            servfail_responses: self.servfail_responses.load(Ordering::Relaxed),
            refused_responses: self.refused_responses.load(Ordering::Relaxed),
            formerr_responses: self.formerr_responses.load(Ordering::Relaxed),
            query_types: self.query_types.read().unwrap().clone(),
            avg_latency_us,
            min_latency_us,
            max_latency_us: self.max_latency_us.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed(),
        }
    }

    pub fn log_summary(&self) {
        let snapshot = self.get_snapshot();
        snapshot.log();
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_queries: u64,
    pub udp_queries: u64,
    pub tcp_queries: u64,
    pub edns_queries: u64,
    pub noerror_responses: u64,
    pub nxdomain_responses: u64,
    pub servfail_responses: u64,
    pub refused_responses: u64,
    pub formerr_responses: u64,
    pub query_types: HashMap<RecordType, u64>,
    pub avg_latency_us: u64,
    pub min_latency_us: u64,
    pub max_latency_us: u64,
    pub rate_limited: u64,
    pub errors: u64,
    pub uptime: Duration,
}

impl MetricsSnapshot {
    pub fn log(&self) {
        tracing::info!("=== DNS Server Metrics ===");
        tracing::info!("Uptime: {:?}", self.uptime);
        tracing::info!("Total queries: {}", self.total_queries);
        tracing::info!(
            "Protocol: UDP={} TCP={} EDNS={}",
            self.udp_queries,
            self.tcp_queries,
            self.edns_queries
        );
        tracing::info!(
            "Responses: NOERROR={} NXDOMAIN={} SERVFAIL={} REFUSED={} FORMERR={}",
            self.noerror_responses,
            self.nxdomain_responses,
            self.servfail_responses,
            self.refused_responses,
            self.formerr_responses
        );

        if !self.query_types.is_empty() {
            tracing::info!("Query types:");
            let mut types: Vec<_> = self.query_types.iter().collect();
            types.sort_by(|a, b| b.1.cmp(a.1));
            for (qtype, count) in types.iter().take(10) {
                tracing::info!("  {:?}: {}", qtype, count);
            }
        }

        if self.total_queries > 0 {
            let qps = self.total_queries as f64 / self.uptime.as_secs_f64();
            tracing::info!(
                "Performance: avg={:.2}ms min={:.2}ms max={:.2}ms QPS={:.0}",
                self.avg_latency_us as f64 / 1000.0,
                self.min_latency_us as f64 / 1000.0,
                self.max_latency_us as f64 / 1000.0,
                qps
            );
        }

        if self.rate_limited > 0 {
            tracing::info!("Rate limited: {}", self.rate_limited);
        }

        if self.errors > 0 {
            tracing::info!("Errors: {}", self.errors);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Udp,
    Tcp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = Metrics::new();

        metrics.record_query(Protocol::Udp, false);
        metrics.record_query(Protocol::Tcp, true);
        metrics.record_response(ResponseCode::NoError);
        metrics.record_response(ResponseCode::NXDomain);
        metrics.record_query_type(RecordType::A);
        metrics.record_latency(Duration::from_micros(1500));

        let snapshot = metrics.get_snapshot();

        assert_eq!(snapshot.total_queries, 2);
        assert_eq!(snapshot.udp_queries, 1);
        assert_eq!(snapshot.tcp_queries, 1);
        assert_eq!(snapshot.edns_queries, 1);
        assert_eq!(snapshot.noerror_responses, 1);
        assert_eq!(snapshot.nxdomain_responses, 1);
        assert_eq!(snapshot.avg_latency_us, 750);
    }
}
