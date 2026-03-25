use rustway::config::GatewayConfig;

fn parse_config(yaml: &str) -> GatewayConfig {
    let interpolated = rustway::config::interpolate_env_vars_pub(yaml);
    serde_yaml::from_str(&interpolated).map(|mut cfg: GatewayConfig| {
        cfg.resolve_services_pub();
        cfg.apply_defaults_pub();
        cfg
    }).unwrap()
}

#[test]
fn test_service_abstraction_resolves_urls() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
services:
  users:
    urls: ["http://users-1:8091", "http://users-2:8091"]
    timeout: 5s
    retry:
      count: 3
routes:
  - name: users
    path: /api/users
    service: users
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let route = &cfg.routes[0];
    assert_eq!(route.destinations, vec!["http://users-1:8091", "http://users-2:8091"]);
    assert_eq!(route.timeout.as_ref().unwrap(), "5s");
    assert_eq!(route.retry.as_ref().unwrap().count, 3);
}

#[test]
fn test_service_single_url() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
services:
  payments:
    url: http://payments:8080
routes:
  - name: pay
    path: /api/pay
    service: payments
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.routes[0].destination, "http://payments:8080");
}

#[test]
fn test_route_overrides_service() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
services:
  users:
    url: http://users:8091
    timeout: 5s
routes:
  - name: users
    path: /api/users
    service: users
    timeout: 10s
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    // Route timeout should override service timeout
    assert_eq!(cfg.routes[0].timeout.as_ref().unwrap(), "10s");
}

#[test]
fn test_global_defaults_applied() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
defaults:
  timeout: 3s
  retry:
    count: 1
    backoff: 50ms
routes:
  - name: test
    path: /api/test
    destination: http://localhost:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.routes[0].timeout.as_ref().unwrap(), "3s");
    assert_eq!(cfg.routes[0].retry.as_ref().unwrap().count, 1);
}

#[test]
fn test_route_overrides_defaults() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
defaults:
  timeout: 3s
routes:
  - name: test
    path: /api/test
    destination: http://localhost:8080
    timeout: 15s
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.routes[0].timeout.as_ref().unwrap(), "15s");
}

#[test]
fn test_env_var_interpolation() {
    unsafe { std::env::set_var("TEST_GW_PORT", "9999"); }
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:${TEST_GW_PORT}"
routes: []
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.server.addr, "0.0.0.0:9999");
    unsafe { std::env::remove_var("TEST_GW_PORT"); }
}

#[test]
fn test_env_var_missing_keeps_placeholder() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:${NONEXISTENT_VAR_XYZ}"
routes: []
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.server.addr, "0.0.0.0:${NONEXISTENT_VAR_XYZ}");
}

#[test]
fn test_validation_missing_service() {
    let yaml = r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api/test
    service: nonexistent
identity:
  api_key_store_path: ./api_keys.yaml
"#;
    let mut cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    cfg.resolve_services_pub();
    let result = cfg.validate_pub();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("nonexistent"));
}

#[test]
fn test_validation_no_destination() {
    let yaml = r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api/test
identity:
  api_key_store_path: ./api_keys.yaml
"#;
    let cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    let result = cfg.validate_pub();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no destination"));
}

#[test]
fn test_validation_passes_with_aggregate() {
    let yaml = r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: dashboard
    path: /api/dashboard
    aggregate:
      - service: users
        path: http://users:8091/me
        field: user
identity:
  api_key_store_path: ./api_keys.yaml
"#;
    let cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(cfg.validate_pub().is_ok());
}
