use rustway::config::GatewayConfig;
use rustway::features::load_balancer::{LoadBalanceStrategy, LoadBalancer};

// ==================== Load Balancer Negative Tests ====================

#[test]
fn test_lb_zero_backends_round_robin() {
    let lb = LoadBalancer::new();
    assert!(lb.next_index(0, &LoadBalanceStrategy::RoundRobin).is_none());
}

#[test]
fn test_lb_zero_backends_random() {
    let lb = LoadBalancer::new();
    assert!(lb.next_index(0, &LoadBalanceStrategy::Random).is_none());
}

// ==================== Config Validation Negative Tests ====================

#[test]
fn test_config_empty_yaml_fails() {
    let yaml = "";
    let result = serde_yaml::from_str::<GatewayConfig>(yaml);
    assert!(result.is_err());
}

#[test]
fn test_config_missing_server_fails() {
    let yaml = r#"
routes:
  - name: test
    path: /test
    destination: http://localhost:9001
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    let result = serde_yaml::from_str::<GatewayConfig>(yaml);
    assert!(result.is_err());
}

#[test]
fn test_config_route_no_destination_fails_validation() {
    let yaml = r#"
server:
  addr: "127.0.0.1:8094"
routes:
  - name: broken
    path: /broken
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    let mut cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    cfg.resolve_services_pub();
    cfg.apply_defaults_pub();
    let result = cfg.validate_pub();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no destination"));
}

#[test]
fn test_config_route_missing_service_fails_validation() {
    let yaml = r#"
server:
  addr: "127.0.0.1:8094"
routes:
  - name: broken
    path: /broken
    service: nonexistent
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    let mut cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    cfg.resolve_services_pub();
    cfg.apply_defaults_pub();
    let result = cfg.validate_pub();
    assert!(result.is_err());
}

// ==================== Route Matching Negative Tests ====================

#[test]
fn test_route_no_match_returns_none() {
    let yaml = r#"
server:
  addr: "127.0.0.1:8094"
routes:
  - name: users
    path: /api/users
    destination: http://localhost:9001
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    let mut cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    cfg.resolve_services_pub();
    cfg.apply_defaults_pub();
    assert!(cfg.find_route_for_path("/api/orders").is_none());
    assert!(cfg.find_route_for_path("/completely/different").is_none());
}

#[test]
fn test_route_match_with_params_returns_captures() {
    let yaml = r#"
server:
  addr: "127.0.0.1:8094"
routes:
  - name: user-detail
    path: "/api/users/{id}"
    destination: "http://localhost:9001/users/{id}"
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    let mut cfg: GatewayConfig = serde_yaml::from_str(yaml).unwrap();
    cfg.resolve_services_pub();
    cfg.apply_defaults_pub();
    cfg.build_route_tree_pub();

    let result = cfg.match_route_with_params("/api/users/42");
    assert!(result.is_some());
    let (route, params) = result.unwrap();
    assert_eq!(route.name, "user-detail");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], ("id".to_string(), "42".to_string()));
}

// ==================== Auth Negative Tests ====================

#[test]
fn test_check_roles_insufficient() {
    use rustway::features::auth::auth::check_roles;
    let user_roles = vec!["user".to_string()];
    let required = vec!["admin".to_string()];
    let result = check_roles(&user_roles, &required);
    assert!(result.is_err());
}

#[test]
fn test_check_roles_empty_user_roles() {
    use rustway::features::auth::auth::check_roles;
    let user_roles: Vec<String> = vec![];
    let required = vec!["admin".to_string()];
    let result = check_roles(&user_roles, &required);
    assert!(result.is_err());
}
