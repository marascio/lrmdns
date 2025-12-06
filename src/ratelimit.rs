use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

#[derive(Debug)]
struct RateLimiterInner {
    clients: HashMap<IpAddr, ClientState>,
    max_qps: u32,
    window: Duration,
    last_cleanup: Instant,
}

#[derive(Debug)]
struct ClientState {
    queries: Vec<Instant>,
}

impl RateLimiter {
    pub fn new(max_qps: u32) -> Self {
        RateLimiter {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                clients: HashMap::new(),
                max_qps,
                window: Duration::from_secs(1),
                last_cleanup: Instant::now(),
            })),
        }
    }

    pub fn check_rate_limit(&self, addr: IpAddr) -> bool {
        let mut inner = self.inner.lock().unwrap();

        // Cleanup old entries every 60 seconds
        if inner.last_cleanup.elapsed() > Duration::from_secs(60) {
            inner.cleanup();
            inner.last_cleanup = Instant::now();
        }

        let now = Instant::now();
        let window = inner.window;
        let max_qps = inner.max_qps;

        let client = inner
            .clients
            .entry(addr)
            .or_insert_with(|| ClientState {
                queries: Vec::new(),
            });

        // Remove queries outside the time window
        client
            .queries
            .retain(|&timestamp| now.duration_since(timestamp) < window);

        // Check if rate limit exceeded
        if client.queries.len() >= max_qps as usize {
            tracing::debug!(
                "Rate limit exceeded for {}: {} queries in {}s",
                addr,
                client.queries.len(),
                window.as_secs()
            );
            return false;
        }

        // Record this query
        client.queries.push(now);
        true
    }
}

impl RateLimiterInner {
    fn cleanup(&mut self) {
        let now = Instant::now();
        self.clients
            .retain(|_, client| {
                client
                    .queries
                    .retain(|&timestamp| now.duration_since(timestamp) < self.window);
                !client.queries.is_empty()
            });

        tracing::debug!("Rate limiter cleanup: {} clients tracked", self.clients.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_rate_limiting() {
        let limiter = RateLimiter::new(10);
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // Should allow first 10 queries
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(addr));
        }

        // 11th query should be rate limited
        assert!(!limiter.check_rate_limit(addr));
    }

    #[test]
    fn test_different_ips() {
        let limiter = RateLimiter::new(5);
        let addr1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let addr2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));

        // Each IP should have its own limit
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(addr1));
            assert!(limiter.check_rate_limit(addr2));
        }

        assert!(!limiter.check_rate_limit(addr1));
        assert!(!limiter.check_rate_limit(addr2));
    }
}
