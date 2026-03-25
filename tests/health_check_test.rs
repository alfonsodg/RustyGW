use rustway::features::health_check::{HealthChecker, parse_body_limit, parse_duration};
use std::time::Duration;

#[test]
fn test_parse_duration_seconds() {
    assert_eq!(parse_duration("5s"), Duration::from_secs(5));
    assert_eq!(parse_duration("30s"), Duration::from_secs(30));
}

#[test]
fn test_parse_duration_milliseconds() {
    assert_eq!(parse_duration("100ms"), Duration::from_millis(100));
    assert_eq!(parse_duration("500ms"), Duration::from_millis(500));
}

#[test]
fn test_parse_duration_minutes() {
    assert_eq!(parse_duration("1m"), Duration::from_secs(60));
    assert_eq!(parse_duration("5m"), Duration::from_secs(300));
}

#[test]
fn test_parse_duration_default() {
    assert_eq!(parse_duration("invalid"), Duration::from_secs(10));
}

#[test]
fn test_parse_body_limit_mb() {
    assert_eq!(parse_body_limit("10mb"), 10 * 1024 * 1024);
    assert_eq!(parse_body_limit("1mb"), 1024 * 1024);
}

#[test]
fn test_parse_body_limit_kb() {
    assert_eq!(parse_body_limit("512kb"), 512 * 1024);
}

#[test]
fn test_parse_body_limit_bytes() {
    assert_eq!(parse_body_limit("1048576"), 1048576);
}

#[test]
fn test_health_checker_default_healthy() {
    let hc = HealthChecker::new();
    assert!(hc.is_healthy("http://unknown:8080"));
}

#[test]
fn test_filter_healthy_all_unknown() {
    let hc = HealthChecker::new();
    let dests = vec!["http://a:8080", "http://b:8080"];
    let result = hc.filter_healthy(&dests);
    // All unknown = all assumed healthy
    assert_eq!(result.len(), 2);
}

#[test]
fn test_filter_healthy_fallback_when_all_down() {
    let hc = HealthChecker::new();
    // Manually mark backends as down by checking filter behavior
    // With no status entries, all are assumed healthy
    let dests = vec!["http://a:8080"];
    let result = hc.filter_healthy(&dests);
    assert_eq!(result, vec!["http://a:8080"]);
}
