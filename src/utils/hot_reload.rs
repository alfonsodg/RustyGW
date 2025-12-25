// Watches the main config and API key files for changes and reloads them

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant}
};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{config::{ApiKeyStore, GatewayConfig}, errors::AppError};

/// Time to wait before processing file change events (debouncing)
const DEBOUNCE_DELAY: Duration = Duration::from_millis(100);

/// Maximum retry attempts for watcher creation
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Retry delay for watcher recreation
const RETRY_DELAY: Duration = Duration::from_secs(1);

/// Custom error type for hot reload operations
#[derive(Debug, thiserror::Error)]
pub enum HotReloadError {
    #[error("Path resolution failed: {0}")]
    PathResolution(String),
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Watcher creation failed: {0}")]
    WatcherCreation(String),
    
    #[error("File watching failed: {0}")]
    FileWatching(String),
    
    #[error("Config reload failed: {0}")]
    ConfigReload(String),
}

impl From<HotReloadError> for AppError {
    fn from(error: HotReloadError) -> Self {
        AppError::HotReloadError(error.to_string())
    }
}

/// Helper function to resolve and verify file paths with proper error handling
fn resolve_and_verify_path(path: PathBuf) -> Result<PathBuf, HotReloadError> {
    match fs::canonicalize(&path) {
        Ok(resolved_path) => {
            // Verify file exists
            if !resolved_path.exists() {
                return Err(HotReloadError::FileNotFound(resolved_path.clone()));
            }

            // Verify it's a file (not directory)
            if !resolved_path.is_file() {
                return Err(HotReloadError::FileNotFound(resolved_path));
            }

            Ok(resolved_path)
        }
        Err(e) => Err(HotReloadError::PathResolution(format!(
            "Failed to resolve path {:?}: {}",
            path, e
        ))),
    }
}

/// Helper function to safely reload configuration with error handling
async fn safe_config_reload(
    config_path: &PathBuf,
    gateway_config: Arc<RwLock<GatewayConfig>>,
) -> Result<(), HotReloadError> {
    match GatewayConfig::load(config_path) {
        Ok(new_config) => {
            let mut config_writer = gateway_config.write().await;
            *config_writer = new_config;
            info!("Successfully reloaded gateway_config.yaml");
            Ok(())
        }
        Err(e) => {
            Err(HotReloadError::ConfigReload(format!(
                "Failed to parse gateway_config.yaml: {}",
                e
            )))
        }
    }
}

/// Helper function to safely reload API key store with error handling
async fn safe_api_key_reload(
    api_key_path: &PathBuf,
    api_key_store: Arc<RwLock<ApiKeyStore>>,
) -> Result<(), HotReloadError> {
    match ApiKeyStore::load(api_key_path) {
        Ok(new_store) => {
            let mut store_writer = api_key_store.write().await;
            *store_writer = new_store;
            info!("Successfully reloaded api_keys.yaml");
            Ok(())
        }
        Err(e) => {
            Err(HotReloadError::ConfigReload(format!(
                "Failed to parse api_keys.yaml: {}",
                e
            )))
        }
    }
}

/// Main function to watch configuration files with improved error handling
pub async fn watch_config_files(
    config_path: PathBuf,
    gateway_config: Arc<RwLock<GatewayConfig>>,
    api_key_store: Arc<RwLock<ApiKeyStore>>,
) -> Result<(), AppError> {
    info!("Starting Configuration file watcher...");

    // Get API key store path from config
    let api_key_store_path_rel = {
        let config_guard = gateway_config.read().await;
        PathBuf::from(config_guard.identity.api_key_store_path.clone())
    };

    // Resolve and verify both paths with error handling
    let (gateway_config_path, api_key_store_path) = match (
        resolve_and_verify_path(config_path),
        resolve_and_verify_path(api_key_store_path_rel),
    ) {
        (Ok(gateway_path), Ok(api_key_path)) => (gateway_path, api_key_path),
        (Err(e), _) | (_, Err(e)) => {
            error!("Failed to resolve configuration paths: {}", e);
            return Err(e.into());
        }
    };

    info!(gateway_config_path = ?gateway_config_path);
    info!(api_key_store_path = ?api_key_store_path);

    // Create the watcher with retry mechanism
    let mut watcher = create_watcher_with_retry().await?;
    
    // Watch both files with error handling
    watch_file(&mut watcher, &gateway_config_path, "gateway config")?;
    watch_file(&mut watcher, &api_key_store_path, "API key store")?;

    // Clone for event processing
    let gateway_config_clone = gateway_config.clone();
    let api_key_store_clone = api_key_store.clone();

    let (tx, mut rx) = mpsc::channel(crate::constants::hot_reload::CHANNEL_BUFFER_SIZE);

    // Set up the watcher callback
    let watcher_tx = tx.clone();
    let watcher_result = Watcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    // Use non-blocking send to avoid deadlocks
                    if let Err(e) = watcher_tx.try_send(event) {
                        warn!("Failed to send file change event: {}", e);
                    }
                }
            }
        },
        notify::Config::default(),
    );

    let mut watcher: RecommendedWatcher = match watcher_result {
        Ok(w) => w,
        Err(e) => {
            return Err(HotReloadError::WatcherCreation(format!(
                "Failed to create file watcher: {}",
                e
            ))
            .into());
        }
    };

    // Start watching with error handling
    if let Err(e) = watcher.watch(&gateway_config_path, RecursiveMode::NonRecursive) {
        return Err(HotReloadError::FileWatching(format!(
            "Failed to watch gateway config file: {}",
            e
        ))
        .into());
    }
    if let Err(e) = watcher.watch(&api_key_store_path, RecursiveMode::NonRecursive) {
        return Err(HotReloadError::FileWatching(format!(
            "Failed to watch API key store file: {}",
            e
        ))
        .into());
    }

    info!("File watcher successfully started");

    // Process file change events with debouncing
    let mut last_event_time = Instant::now();
    let mut pending_event: Option<Event> = None;

    while let Some(event) = rx.recv().await {
        let now = Instant::now();
        
        // Debounce rapid file changes
        if now.duration_since(last_event_time) < DEBOUNCE_DELAY {
            pending_event = Some(event);
            continue;
        }

        // Process any pending event first
        if let Some(pending) = pending_event.take() {
            process_config_event(
                &pending,
                &gateway_config_path,
                &api_key_store_path,
                gateway_config_clone.clone(),
                api_key_store_clone.clone(),
            ).await;
        }

        // Process current event
        process_config_event(
            &event,
            &gateway_config_path,
            &api_key_store_path,
            gateway_config_clone.clone(),
            api_key_store_clone.clone(),
        ).await;

        last_event_time = now;
    }

    Ok(())
}

/// Create watcher with retry mechanism
async fn create_watcher_with_retry() -> Result<RecommendedWatcher, HotReloadError> {
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        match Watcher::new(
            |_res: Result<Event, notify::Error>| {
                // This closure will be overridden in the main function
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => {
                if attempt > 1 {
                    info!("Watcher created successfully on attempt {}", attempt);
                }
                return Ok(watcher);
            }
            Err(e) => {
                if attempt == MAX_RETRY_ATTEMPTS {
                    return Err(HotReloadError::WatcherCreation(format!(
                        "Failed to create watcher after {} attempts: {}",
                        MAX_RETRY_ATTEMPTS, e
                    )));
                }
                warn!(
                    "Watcher creation failed on attempt {}: {}. Retrying...",
                    attempt, e
                );
                sleep(RETRY_DELAY).await;
            }
        }
    }
    
    unreachable!()
}

/// Watch a specific file with error handling
fn watch_file(
    watcher: &mut RecommendedWatcher,
    path: &PathBuf,
    file_type: &str,
) -> Result<(), HotReloadError> {
    match watcher.watch(path, RecursiveMode::NonRecursive) {
        Ok(_) => {
            info!("Successfully watching {} file: {:?}", file_type, path);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to watch {} file: {}", file_type, e);
            error!("{}", error_msg);
            Err(HotReloadError::FileWatching(error_msg))
        }
    }
}

/// Process configuration file change events
async fn process_config_event(
    event: &Event,
    gateway_config_path: &PathBuf,
    api_key_store_path: &PathBuf,
    gateway_config: Arc<RwLock<GatewayConfig>>,
    api_key_store: Arc<RwLock<ApiKeyStore>>,
) {
    info!("Detected change in config files: {:?}", event.paths);

    // Process gateway config changes
    if event.paths.contains(gateway_config_path) {
        match safe_config_reload(gateway_config_path, gateway_config).await {
            Ok(_) => info!("Gateway config reloaded successfully"),
            Err(e) => {
                error!("Failed to reload gateway config: {}. Keeping old config.", e);
            }
        }
    }

    // Process API key store changes
    if event.paths.contains(api_key_store_path) {
        match safe_api_key_reload(api_key_store_path, api_key_store).await {
            Ok(_) => info!("API key store reloaded successfully"),
            Err(e) => {
                error!("Failed to reload API key store: {}. Keeping old config.", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::sync::RwLock;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_resolve_and_verify_path() {
        // Test with existing file
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        let result = resolve_and_verify_path(path);
        assert!(result.is_ok());
        
        // Test with non-existing file
        let non_existing = PathBuf::from("/non/existent/file");
        let result = resolve_and_verify_path(non_existing);
        assert!(result.is_err());
        
        if let Err(e) = result {
            // Check that it's either a path resolution error or file not found error
            let error_msg = e.to_string();
            assert!(error_msg.contains("Path resolution failed") || error_msg.contains("File not found"), 
                   "Expected path resolution or file not found error, got: {}", error_msg);
        }
    }

    #[tokio::test]
    async fn test_hot_reload_error_creation() {
        // Test creating different types of hot reload errors
        let path = PathBuf::from("/test/path");
        
        let path_error = HotReloadError::PathResolution("test error".to_string());
        assert!(path_error.to_string().contains("Path resolution failed"));
        
        let file_error = HotReloadError::FileNotFound(path.clone());
        assert!(file_error.to_string().contains("File not found"));
        
        let watcher_error = HotReloadError::WatcherCreation("watcher failed".to_string());
        assert!(watcher_error.to_string().contains("Watcher creation failed"));
        
        let config_error = HotReloadError::ConfigReload("config failed".to_string());
        assert!(config_error.to_string().contains("Config reload failed"));
    }

    #[tokio::test]
    async fn test_app_error_from_hot_reload_error() {
        let hot_reload_error = HotReloadError::PathResolution("test error".to_string());
        let app_error: AppError = hot_reload_error.into();
        
        // Verify that the conversion works and the error is properly formatted
        assert_eq!(app_error.to_string(), "Hot reload error: Path resolution failed: test error");
    }
}
