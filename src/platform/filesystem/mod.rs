use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use tokio::fs;

#[cfg(target_os = "windows")]
pub mod windows;

/// File system manager trait for cross-platform file operations
#[async_trait::async_trait]
pub trait FileSystemManager: Send + Sync {
    /// Scan a media directory and return all media files
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError>;
    
    /// Normalize a path for the current platform
    fn normalize_path(&self, path: &Path) -> PathBuf;
    
    /// Check if a path is accessible with current permissions
    async fn is_accessible(&self, path: &Path) -> bool;
    
    /// Get detailed file information
    async fn get_file_info(&self, path: &Path) -> Result<FileInfo, FileSystemError>;
    
    /// Check if two paths refer to the same file (handles case sensitivity)
    fn paths_equal(&self, path1: &Path, path2: &Path) -> bool;
    
    /// Validate that a path is safe to access (security check)
    fn validate_path(&self, path: &Path) -> Result<(), FileSystemError>;
    
    /// Get the canonical form of a path
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, FileSystemError>;
    
    /// Check if a file matches the given extensions (case-insensitive on Windows)
    fn matches_extension(&self, path: &Path, extensions: &[String]) -> bool;
}

/// File system specific errors
#[derive(Error, Debug)]
pub enum FileSystemError {
    #[error("Path not found: {path}")]
    PathNotFound { path: String },
    
    #[error("Access denied: {path} - {reason}")]
    AccessDenied { path: String, reason: String },
    
    #[error("Invalid path: {path} - {reason}")]
    InvalidPath { path: String, reason: String },
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Permission error: {path} - {details}")]
    Permission { path: String, details: String },
    
    #[error("Encoding error: {path} - {details}")]
    Encoding { path: String, details: String },
    
    #[error("Platform-specific error: {0}")]
    Platform(String),
}

/// Detailed file information
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File size in bytes
    pub size: u64,
    
    /// Last modified time
    pub modified: SystemTime,
    
    /// File permissions information
    pub permissions: FilePermissions,
    
    /// MIME type based on file extension
    pub mime_type: String,
    
    /// Whether the file is hidden
    pub is_hidden: bool,
    
    /// Platform-specific metadata
    pub metadata: HashMap<String, String>,
}

/// Cross-platform file permissions representation
#[derive(Debug, Clone)]
pub struct FilePermissions {
    /// Whether the file is readable
    pub readable: bool,
    
    /// Whether the file is writable
    pub writable: bool,
    
    /// Whether the file is executable
    pub executable: bool,
    
    /// Platform-specific permission details
    pub platform_details: HashMap<String, String>,
}

/// Media file representation with platform-aware handling
#[derive(Debug, Clone)]
pub struct MediaFile {
    /// Unique identifier for the file
    pub id: Option<i64>,
    
    /// Full path to the file
    pub path: PathBuf,
    
    /// File name only
    pub filename: String,
    
    /// File size in bytes
    pub size: u64,
    
    /// Last modified time
    pub modified: SystemTime,
    
    /// MIME type
    pub mime_type: String,
    
    /// Media duration (for audio/video files)
    pub duration: Option<std::time::Duration>,
    
    /// Media title metadata
    pub title: Option<String>,
    
    /// Media artist metadata
    pub artist: Option<String>,
    
    /// Media album metadata
    pub album: Option<String>,
    
    /// When this record was created
    pub created_at: SystemTime,
    
    /// When this record was last updated
    pub updated_at: SystemTime,
}

/// Supported media file extensions and their MIME types
pub const SUPPORTED_MEDIA_TYPES: &[(&str, &str)] = &[
    // Video formats
    ("mkv", "video/x-matroska"),
    ("mp4", "video/mp4"),
    ("avi", "video/x-msvideo"),
    ("mov", "video/quicktime"),
    ("wmv", "video/x-ms-wmv"),
    ("flv", "video/x-flv"),
    ("webm", "video/webm"),
    ("m4v", "video/x-m4v"),
    ("3gp", "video/3gpp"),
    ("mpg", "video/mpeg"),
    ("mpeg", "video/mpeg"),
    // Audio formats
    ("mp3", "audio/mpeg"),
    ("flac", "audio/flac"),
    ("wav", "audio/wav"),
    ("aac", "audio/aac"),
    ("ogg", "audio/ogg"),
    ("wma", "audio/x-ms-wma"),
    ("m4a", "audio/mp4"),
    ("opus", "audio/opus"),
    ("aiff", "audio/aiff"),
    // Image formats
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("gif", "image/gif"),
    ("bmp", "image/bmp"),
    ("tiff", "image/tiff"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

/// Get MIME type for a file based on its extension
pub fn get_mime_type_for_extension(extension: &str) -> String {
    let ext_lower = extension.to_lowercase();
    SUPPORTED_MEDIA_TYPES
        .iter()
        .find(|(ext, _)| *ext == ext_lower)
        .map(|(_, mime)| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

/// Check if a file extension is supported for media serving
pub fn is_supported_media_extension(extension: &str) -> bool {
    let ext_lower = extension.to_lowercase();
    SUPPORTED_MEDIA_TYPES
        .iter()
        .any(|(ext, _)| *ext == ext_lower)
}

/// Base implementation of FileSystemManager with common functionality
pub struct BaseFileSystemManager {
    /// Whether the file system is case-sensitive
    pub case_sensitive: bool,
}

impl BaseFileSystemManager {
    /// Create a new base file system manager
    pub fn new(case_sensitive: bool) -> Self {
        Self { case_sensitive }
    }
    
    /// Common path validation logic
    pub fn validate_path_common(&self, path: &Path) -> Result<(), FileSystemError> {
        // Check for null bytes
        if path.to_string_lossy().contains('\0') {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path contains null bytes".to_string(),
            });
        }
        
        // Check for excessively long paths
        if path.to_string_lossy().len() > 4096 {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path is too long".to_string(),
            });
        }
        
        // Check for directory traversal attempts
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path contains directory traversal".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Common file info extraction
    pub async fn get_file_info_common(&self, path: &Path) -> Result<FileInfo, FileSystemError> {
        let metadata = fs::metadata(path).await?;
        
        let permissions = FilePermissions {
            readable: !metadata.permissions().readonly(),
            writable: !metadata.permissions().readonly(),
            executable: false, // Will be overridden by platform-specific implementations
            platform_details: HashMap::new(),
        };
        
        let mime_type = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(get_mime_type_for_extension)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        
        let is_hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false);
        
        Ok(FileInfo {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            permissions,
            mime_type,
            is_hidden,
            metadata: HashMap::new(),
        })
    }
    
    /// Common media file scanning logic
    pub async fn scan_directory_common(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError> {
        let mut media_files = Vec::new();
        let mut entries = fs::read_dir(path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            
            // Skip directories
            if entry_path.is_dir() {
                continue;
            }
            
            // Check if it's a supported media file
            if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                if !is_supported_media_extension(extension) {
                    continue;
                }
                
                // Get file metadata
                let metadata = entry.metadata().await?;
                let filename = entry_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                let mime_type = get_mime_type_for_extension(extension);
                let now = SystemTime::now();
                
                media_files.push(MediaFile {
                    id: None,
                    path: entry_path,
                    filename,
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    mime_type,
                    duration: None, // TODO: Extract from metadata
                    title: None,    // TODO: Extract from metadata
                    artist: None,   // TODO: Extract from metadata
                    album: None,    // TODO: Extract from metadata
                    created_at: now,
                    updated_at: now,
                });
            }
        }
        
        Ok(media_files)
    }
}

#[async_trait::async_trait]
impl FileSystemManager for BaseFileSystemManager {
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError> {
        self.validate_path_common(path)?;
        
        if !self.is_accessible(path).await {
            return Err(FileSystemError::AccessDenied {
                path: path.display().to_string(),
                reason: "Directory is not accessible".to_string(),
            });
        }
        
        self.scan_directory_common(path).await
    }
    
    fn normalize_path(&self, path: &Path) -> PathBuf {
        // Basic normalization - platform-specific implementations will override
        path.to_path_buf()
    }
    
    async fn is_accessible(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok()
    }
    
    async fn get_file_info(&self, path: &Path) -> Result<FileInfo, FileSystemError> {
        self.get_file_info_common(path).await
    }
    
    fn paths_equal(&self, path1: &Path, path2: &Path) -> bool {
        if self.case_sensitive {
            path1 == path2
        } else {
            path1.to_string_lossy().to_lowercase() == path2.to_string_lossy().to_lowercase()
        }
    }
    
    fn validate_path(&self, path: &Path) -> Result<(), FileSystemError> {
        self.validate_path_common(path)
    }
    
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, FileSystemError> {
        fs::canonicalize(path).await.map_err(FileSystemError::from)
    }
    
    fn matches_extension(&self, path: &Path, extensions: &[String]) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_to_check = if self.case_sensitive {
                ext.to_string()
            } else {
                ext.to_lowercase()
            };
            
            extensions.iter().any(|allowed| {
                let allowed_ext = if self.case_sensitive {
                    allowed.clone()
                } else {
                    allowed.to_lowercase()
                };
                ext_to_check == allowed_ext
            })
        } else {
            false
        }
    }
}

/// Create a platform-specific file system manager
pub fn create_platform_filesystem_manager() -> Box<dyn FileSystemManager> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsFileSystemManager::new())
    }
    
    #[cfg(target_os = "macos")]
    {
        Box::new(BaseFileSystemManager::new(true)) // macOS is case-sensitive
    }
    
    #[cfg(target_os = "linux")]
    {
        Box::new(BaseFileSystemManager::new(true)) // Linux is case-sensitive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_mime_type_detection() {
        assert_eq!(get_mime_type_for_extension("mp4"), "video/mp4");
        assert_eq!(get_mime_type_for_extension("MP4"), "video/mp4");
        assert_eq!(get_mime_type_for_extension("mp3"), "audio/mpeg");
        assert_eq!(get_mime_type_for_extension("unknown"), "application/octet-stream");
    }
    
    #[test]
    fn test_supported_extension_check() {
        assert!(is_supported_media_extension("mp4"));
        assert!(is_supported_media_extension("MP4"));
        assert!(is_supported_media_extension("mp3"));
        assert!(!is_supported_media_extension("txt"));
        assert!(!is_supported_media_extension("unknown"));
    }
    
    #[test]
    fn test_path_validation() {
        let manager = BaseFileSystemManager::new(true);
        
        // Valid paths
        assert!(manager.validate_path_common(Path::new("/valid/path")).is_ok());
        assert!(manager.validate_path_common(Path::new("relative/path")).is_ok());
        
        // Invalid paths
        assert!(manager.validate_path_common(Path::new("path/with/\0/null")).is_err());
        assert!(manager.validate_path_common(Path::new("path/../traversal")).is_err());
    }
    
    #[test]
    fn test_case_sensitivity() {
        let case_sensitive = BaseFileSystemManager::new(true);
        let case_insensitive = BaseFileSystemManager::new(false);
        
        let path1 = Path::new("/Test/Path");
        let path2 = Path::new("/test/path");
        
        assert!(!case_sensitive.paths_equal(path1, path2));
        assert!(case_insensitive.paths_equal(path1, path2));
    }
    
    #[test]
    fn test_extension_matching() {
        let case_sensitive = BaseFileSystemManager::new(true);
        let case_insensitive = BaseFileSystemManager::new(false);
        
        let path = Path::new("test.MP4");
        let extensions = vec!["mp4".to_string(), "avi".to_string()];
        
        assert!(!case_sensitive.matches_extension(path, &extensions));
        assert!(case_insensitive.matches_extension(path, &extensions));
    }
}