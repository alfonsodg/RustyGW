// Application-wide constants

/// Cache configuration
pub mod cache {
    pub const MAX_CAPACITY: u64 = 10_000;
    pub const TTL_SECONDS: u64 = 300; // 5 minutes
    pub const IDLE_TIMEOUT_SECONDS: u64 = 180; // 3 minutes
    pub const MAX_KEY_LENGTH: usize = 512;
    pub const TRUNCATED_KEY_LENGTH: usize = 508;
}

/// Rate limiter configuration
pub mod rate_limiter {
    pub const DEFAULT_TTL_SECONDS: u64 = 3600; // 1 hour
    pub const DEFAULT_PERIOD_SECONDS: u64 = 60; // 1 minute
}

/// Circuit breaker configuration
pub mod circuit_breaker {
    pub const DEFAULT_TTL_SECONDS: u64 = 3600; // 1 hour
}

/// Monitoring configuration
pub mod monitoring {
    pub const METRICS_INTERVAL_SECONDS: u64 = 60; // 1 minute
}

/// Hot reload configuration
pub mod hot_reload {
    pub const CHANNEL_BUFFER_SIZE: usize = 32;
}

/// Default configuration values
pub mod defaults {
    pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB
}

/// Time conversion helpers
pub mod time {
    pub const SECONDS_PER_MINUTE: u64 = 60;
    pub const SECONDS_PER_HOUR: u64 = 3600;
}
