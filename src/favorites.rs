use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteFolder {
    pub path: String,
    pub name: String,
    pub last_used: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoritesManager {
    pub favorites: Vec<FavoriteFolder>,
    pub recent_folders: Vec<FavoriteFolder>,
}

impl FavoritesManager {
    pub fn new() -> Self {
        Self {
            favorites: Vec::new(),
            recent_folders: Vec::new(),
        }
    }

    pub fn load() -> Self {
        let config_path = Self::get_config_path();

        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(manager) = serde_json::from_str(&content) {
                return manager;
            }
        }

        Self::new()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        // Create config directory on first save.
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, json)?;

        Ok(())
    }

    fn get_config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("quick-findr");
        path.push("favorites.json");
        path
    }

    pub fn add_favorite(&mut self, path: String, name: String) {
        // Do not add duplicates.
        if !self.favorites.iter().any(|f| f.path == path) {
            self.favorites.push(FavoriteFolder {
                path,
                name,
                last_used: Self::current_timestamp(),
            });
            let _ = self.save();
        }
    }

    pub fn remove_favorite(&mut self, path: &str) {
        self.favorites.retain(|f| f.path != path);
        let _ = self.save();
    }

    pub fn add_recent(&mut self, path: String) {
        let timestamp = Self::current_timestamp();

        // Keep a unique list.
        self.recent_folders.retain(|f| f.path != path);

        // Insert at the front (most recent first).
        let name = PathBuf::from(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        self.recent_folders.insert(
            0,
            FavoriteFolder {
                path,
                name,
                last_used: timestamp,
            },
        );

        // Keep only the last 10.
        if self.recent_folders.len() > 10 {
            self.recent_folders.truncate(10);
        }

        let _ = self.save();
    }

    pub fn update_last_used(&mut self, path: &str) {
        let timestamp = Self::current_timestamp();

        // Update in favorites.
        if let Some(fav) = self.favorites.iter_mut().find(|f| f.path == path) {
            fav.last_used = timestamp;
        }

        // Update in recents.
        if let Some(recent) = self.recent_folders.iter_mut().find(|f| f.path == path) {
            recent.last_used = timestamp;
        }

        let _ = self.save();
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_favorite() {
        let mut manager = FavoritesManager::new();
        manager.add_favorite("/test/path".to_string(), "Test".to_string());

        assert_eq!(manager.favorites.len(), 1);
        assert_eq!(manager.favorites[0].path, "/test/path");
        assert_eq!(manager.favorites[0].name, "Test");
    }

    #[test]
    fn test_no_duplicate_favorites() {
        let mut manager = FavoritesManager::new();
        manager.add_favorite("/test/path".to_string(), "Test".to_string());
        manager.add_favorite("/test/path".to_string(), "Test".to_string());

        assert_eq!(manager.favorites.len(), 1);
    }

    #[test]
    fn test_remove_favorite() {
        let mut manager = FavoritesManager::new();
        manager.add_favorite("/test/path".to_string(), "Test".to_string());
        manager.remove_favorite("/test/path");

        assert_eq!(manager.favorites.len(), 0);
    }

    #[test]
    fn test_add_recent() {
        let mut manager = FavoritesManager::new();
        manager.add_recent("/test/path1".to_string());
        manager.add_recent("/test/path2".to_string());

        assert_eq!(manager.recent_folders.len(), 2);
        assert_eq!(manager.recent_folders[0].path, "/test/path2"); // Most recent first
    }

    #[test]
    fn test_recent_limit() {
        let mut manager = FavoritesManager::new();

        for i in 0..15 {
            manager.add_recent(format!("/test/path{}", i));
        }

        assert_eq!(manager.recent_folders.len(), 10); // Max 10
    }

    #[test]
    fn test_recent_no_duplicate() {
        let mut manager = FavoritesManager::new();
        manager.add_recent("/test/path".to_string());
        manager.add_recent("/test/path".to_string());

        assert_eq!(manager.recent_folders.len(), 1);
    }
}
