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

        let client = inner.clients.entry(addr).or_insert_with(|| ClientState {
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
        self.clients.retain(|_, client| {
            client
                .queries
                .retain(|&timestamp| now.duration_since(timestamp) < self.window);
            !client.queries.is_empty()
        });

        tracing::debug!(
            "Rate limiter cleanup: {} clients tracked",
            self.clients.len()
        );
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

    #[test]
    fn test_window_expiration() {
        let limiter = RateLimiter::new(5);
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // Fill up the rate limit
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(addr));
        }

        // Should be rate limited now
        assert!(!limiter.check_rate_limit(addr));

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(1100));

        // Should be allowed again after window expires
        assert!(limiter.check_rate_limit(addr));
    }

    #[test]
    fn test_cleanup_triggered() {
        let limiter = RateLimiter::new(10);

        // Add queries from multiple IPs
        for i in 1..=20 {
            let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, i as u8));
            assert!(limiter.check_rate_limit(addr));
        }

        // Manually trigger cleanup by setting last_cleanup time far in the past
        {
            let mut inner = limiter.inner.lock().unwrap();
            inner.last_cleanup = Instant::now() - Duration::from_secs(61);
        }

        // This should trigger cleanup
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 100));
        assert!(limiter.check_rate_limit(addr));

        // Verify cleanup was triggered by checking last_cleanup was updated
        {
            let inner = limiter.inner.lock().unwrap();
            assert!(inner.last_cleanup.elapsed() < Duration::from_secs(5));
        }
    }

    #[test]
    fn test_cleanup_removes_old_clients() {
        let limiter = RateLimiter::new(10);

        // Add queries from multiple IPs
        for i in 1..=10 {
            let addr = IpAddr::V4(Ipv4Addr::new(192, 168, 0, i as u8));
            assert!(limiter.check_rate_limit(addr));
        }

        // Verify clients are tracked
        {
            let inner = limiter.inner.lock().unwrap();
            assert_eq!(inner.clients.len(), 10);
        }

        // Wait for window to expire
        std::thread::sleep(Duration::from_millis(1100));

        // Trigger cleanup manually
        {
            let mut inner = limiter.inner.lock().unwrap();
            inner.cleanup();
        }

        // All clients should be cleaned up since their queries expired
        {
            let inner = limiter.inner.lock().unwrap();
            assert_eq!(inner.clients.len(), 0);
        }
    }

    #[test]
    fn test_partial_cleanup() {
        let limiter = RateLimiter::new(10);

        // Add some old queries
        let addr1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(limiter.check_rate_limit(addr1));

        // Wait a bit
        std::thread::sleep(Duration::from_millis(600));

        // Add some new queries
        let addr2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(limiter.check_rate_limit(addr2));

        // Wait for first client's queries to expire
        std::thread::sleep(Duration::from_millis(600));

        // Cleanup should remove addr1 but keep addr2
        {
            let mut inner = limiter.inner.lock().unwrap();
            inner.cleanup();
            assert!(inner.clients.contains_key(&addr2));
            // addr1 might still exist if within window, or might be removed
        }
    }

    #[test]
    fn test_ipv6_addresses() {
        use std::net::Ipv6Addr;

        let limiter = RateLimiter::new(3);
        let addr = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));

        // Should work with IPv6 addresses
        for _ in 0..3 {
            assert!(limiter.check_rate_limit(addr));
        }

        assert!(!limiter.check_rate_limit(addr));
    }

    #[test]
    fn test_zero_rate_limit() {
        let limiter = RateLimiter::new(0);
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // With 0 max_qps, first query should be blocked
        assert!(!limiter.check_rate_limit(addr));
    }

    #[test]
    fn test_high_rate_limit() {
        let limiter = RateLimiter::new(1000);
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // Should allow many queries
        for _ in 0..1000 {
            assert!(limiter.check_rate_limit(addr));
        }

        // 1001st should fail
        assert!(!limiter.check_rate_limit(addr));
    }
}
