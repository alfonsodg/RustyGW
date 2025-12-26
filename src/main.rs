use std::path::PathBuf;
use rustway::run;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config_path = PathBuf::from("gateway.yaml");
    run(config_path).await
}
