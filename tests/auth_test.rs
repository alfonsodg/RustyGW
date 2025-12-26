use rustway::config::GatewayConfig;

#[tokio::test]
async fn test_config_loading() {
    // Test that we can load a basic config
    let config_str = r#"
server:
  addr: "0.0.0.0:3000"
routes: []
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    
    let config: Result<GatewayConfig, _> = serde_yaml::from_str(config_str);
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.server.addr, "0.0.0.0:3000");
}

#[tokio::test]
async fn test_config_with_routes() {
    let config_str = r#"
server:
  addr: "0.0.0.0:8080"
routes:
  - name: "test_route"
    path: "/api/test"
    destination: "http://localhost:8094"
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    
    let config: Result<GatewayConfig, _> = serde_yaml::from_str(config_str);
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.server.addr, "0.0.0.0:8080");
    assert_eq!(config.routes.len(), 1);
    assert_eq!(config.routes[0].path, "/api/test");
}

#[tokio::test]
async fn test_config_basic_structure() {
    let config_str = r#"
server:
  addr: "0.0.0.0:3000"
routes: []
identity:
  api_key_store_path: "./api_keys.yaml"
"#;
    
    let config: Result<GatewayConfig, _> = serde_yaml::from_str(config_str);
    assert!(config.is_ok());
    
    let config = config.unwrap();
    // Test that basic structure is present
    assert!(config.routes.is_empty());
    assert_eq!(config.identity.api_key_store_path, "./api_keys.yaml");
}
