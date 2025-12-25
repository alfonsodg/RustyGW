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
            assert!(e.to_string().contains("File not found"));
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
