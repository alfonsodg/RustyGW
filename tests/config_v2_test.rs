use rustway::config::GatewayConfig;

fn parse_config(yaml: &str) -> GatewayConfig {
    serde_yaml::from_str(yaml).unwrap()
}

#[test]
fn test_route_single_destination() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let route = &cfg.routes[0];
    assert_eq!(route.all_destinations(), vec!["http://localhost:8080"]);
}

#[test]
fn test_route_multiple_destinations() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://fallback:8080
    destinations:
      - http://svc1:8080
      - http://svc2:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let route = &cfg.routes[0];
    let dests = route.all_destinations();
    assert_eq!(dests.len(), 2);
    assert_eq!(dests[0], "http://svc1:8080");
    assert_eq!(dests[1], "http://svc2:8080");
}

#[test]
fn test_route_load_balance_default() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    // Default should be RoundRobin
    match cfg.routes[0].load_balance {
        rustway::features::load_balancer::LoadBalanceStrategy::RoundRobin => {},
        _ => panic!("Expected RoundRobin default"),
    }
}

#[test]
fn test_route_retry_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
    retry:
      count: 3
      backoff: 200ms
      retry_on: [502, 503]
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let retry = cfg.routes[0].retry.as_ref().unwrap();
    assert_eq!(retry.count, 3);
    assert_eq!(retry.backoff, "200ms");
    assert_eq!(retry.retry_on, vec![502, 503]);
}

#[test]
fn test_route_timeout_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
    timeout: 5s
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.routes[0].timeout.as_ref().unwrap(), "5s");
}

#[test]
fn test_route_transform_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
    transform:
      rewrite_path: /v2{path}
      request_headers:
        x-custom: value
      remove_request_headers: [cookie]
      response_headers:
        x-powered-by: RustyGW
      remove_response_headers: [server]
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let t = cfg.routes[0].transform.as_ref().unwrap();
    assert_eq!(t.rewrite_path.as_ref().unwrap(), "/v2{path}");
    assert_eq!(t.request_headers.get("x-custom").unwrap(), "value");
    assert_eq!(t.remove_request_headers, vec!["cookie"]);
    assert_eq!(t.response_headers.get("x-powered-by").unwrap(), "RustyGW");
    assert_eq!(t.remove_response_headers, vec!["server"]);
}

#[test]
fn test_route_health_check_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
    health_check:
      interval: 10s
      path: /healthz
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let hc = cfg.routes[0].health_check.as_ref().unwrap();
    assert_eq!(hc.interval, "10s");
    assert_eq!(hc.path, "/healthz");
}

#[test]
fn test_route_tls_skip_verify() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: https://self-signed:8443
    tls_skip_verify: true
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert!(cfg.routes[0].tls_skip_verify);
}

#[test]
fn test_route_aggregate_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: dashboard
    path: /api/dashboard
    destination: http://localhost
    aggregate:
      - service: users
        path: http://users:8091/me
        field: user
        timeout: 3s
      - service: orders
        path: http://orders:8093/recent
        field: orders
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let agg = cfg.routes[0].aggregate.as_ref().unwrap();
    assert_eq!(agg.len(), 2);
    assert_eq!(agg[0].field, "user");
    assert_eq!(agg[0].timeout.as_ref().unwrap(), "3s");
    assert_eq!(agg[1].field, "orders");
}

#[test]
fn test_cors_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes: []
cors:
  enabled: true
  origins: ["https://app.example.com", "http://localhost:3000"]
  methods: [GET, POST]
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert!(cfg.cors.enabled);
    assert_eq!(cfg.cors.origins.len(), 2);
    assert_eq!(cfg.cors.methods, vec!["GET", "POST"]);
}

#[test]
fn test_cors_disabled_by_default() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes: []
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert!(!cfg.cors.enabled);
}

#[test]
fn test_pool_config() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
  pool:
    idle_timeout: 120s
    max_idle_per_host: 64
    connect_timeout: 3s
    request_timeout: 15s
    body_limit: 5mb
routes: []
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.server.pool.idle_timeout, "120s");
    assert_eq!(cfg.server.pool.max_idle_per_host, 64);
    assert_eq!(cfg.server.pool.connect_timeout, "3s");
    assert_eq!(cfg.server.pool.request_timeout, "15s");
    assert_eq!(cfg.server.pool.body_limit, "5mb");
}

#[test]
fn test_pool_config_defaults() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes: []
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert_eq!(cfg.server.pool.idle_timeout, "90s");
    assert_eq!(cfg.server.pool.max_idle_per_host, 32);
    assert_eq!(cfg.server.pool.body_limit, "10mb");
}

#[test]
fn test_find_route_longest_match() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: general
    path: /api
    destination: http://general:8080
  - name: specific
    path: /api/users
    destination: http://users:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    let route = cfg.find_route_for_path("/api/users/123").unwrap();
    assert_eq!(route.name, "specific");
}

#[test]
fn test_find_route_no_match() {
    let cfg = parse_config(r#"
server:
  addr: "0.0.0.0:8094"
routes:
  - name: test
    path: /api
    destination: http://localhost:8080
identity:
  api_key_store_path: ./api_keys.yaml
"#);
    assert!(cfg.find_route_for_path("/other").is_none());
}
