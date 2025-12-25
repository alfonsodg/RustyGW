// Watches the main config and API key files for changes and reloads them

use std::{fs, path::PathBuf, sync::Arc};

use notify::{ Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use crate::config::{ApiKeyStore, GatewayConfig};

pub async fn watch_config_files(
    config_path: PathBuf,
    gateway_config: Arc<RwLock<GatewayConfig>>,
    api_key_store: Arc<RwLock<ApiKeyStore>>
) {

    info!("Starting Configuration file watcher...");

    let  api_key_store_path_rel = {
        let config_guard = gateway_config.read().await;
         PathBuf::from(config_guard.identity.api_key_store_path.clone())
        
    };

    let gateway_config_path = match fs::canonicalize(&config_path) {
        Ok(path) => path,
        Err(e) => {
            error!(path = ?config_path, "Failed to get absolute path for gateway config: {}", e);
            return;
        }
    };
    let api_key_store_path = match fs::canonicalize(&api_key_store_path_rel) {
        Ok(path) => path,
        Err(e) => {
            error!(path = ?api_key_store_path_rel, "Failed to get absolute path for API key store: {}", e);
            return;
        }
    };
    
    info!(gateway_config_path = ?gateway_config_path);
    info!(api_key_store_path = ?api_key_store_path);

    let gateway_config_clone = gateway_config.clone();
    let api_key_store_clone = api_key_store.clone();

    let (tx, mut rx) = mpsc::channel(1);

    let mut watcher: RecommendedWatcher = 
        match Watcher::new(move |res:Result<Event, notify::Error>| {
            if let Ok(event) = res
                && (event.kind.is_modify() || event.kind.is_create()) {
                    tx.blocking_send(event).expect("Failed to send file change event");
                }
        }, notify::Config::default()
    ) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create file watcher: {}", e);
            return;
        }
    };

    // Watch both files
    if let Err(e) = watcher.watch(&gateway_config_path, RecursiveMode::NonRecursive) {
        error!(path = ?gateway_config_path, "Failed to watch gateway config file: {}", e);
    }
    if let Err(e) = watcher.watch(&api_key_store_path, RecursiveMode::NonRecursive) {
        error!(path = ?api_key_store_path, "Failed to watch API key store file: {}", e);
    }

    //Process file change events
    while let Some(event) = rx.recv().await {
        info!("Detected change in config files: {:?}", event.paths);

        if event.paths.contains(&gateway_config_path) {
            match GatewayConfig::load(&gateway_config_path) {
                Ok(new_config) => {
                    let mut config_writer = gateway_config_clone.write().await;
                    *config_writer = new_config;
                    info!("Successfully reloaded gateway_config.yaml");

                }
                Err(e) => {
                    error!("Failed to reload gateway_config.yaml: {}. Keeping old config.", e);
                }
            }
        }
        if event.paths.contains(&api_key_store_path) {
            match ApiKeyStore::load(&api_key_store_path) {
                Ok(new_store) => {
                    let mut store_writer = api_key_store_clone.write().await;
                    *store_writer = new_store;
                    info!("Successfully reloaded api_keys.yaml");
                }
                Err(e) => {
                    error!("Failed to reload api_keys.yaml: {}. Keeping old config.", e);
                }
            }
        }
    }


}