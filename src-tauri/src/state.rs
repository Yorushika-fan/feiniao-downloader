use crate::types::{AppSettings, DownloadTask, HistoryItem};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::process::Child;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

pub struct AppState {
    #[allow(dead_code)]
    pub app_handle: AppHandle,
    pub tasks: Mutex<HashMap<String, DownloadTask>>,
    pub processes: AsyncMutex<HashMap<String, Child>>,
    pub history: Mutex<Vec<HistoryItem>>,
    pub settings: Mutex<AppSettings>,
    pub settings_path: PathBuf,
    pub history_path: PathBuf,
    pub semaphore: Mutex<Arc<Semaphore>>,
}

impl AppState {
    pub fn new(app_handle: AppHandle) -> Self {
        let base = dirs::config_dir()
            .map(|d| d.join("FeiNiaoDownloader"))
            .unwrap_or_else(|| PathBuf::from(".feiniao"));
        let _ = std::fs::create_dir_all(&base);
        let settings_path = base.join("settings.json");
        let history_path = base.join("history.json");

        let settings: AppSettings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let _ = std::fs::create_dir_all(&settings.output_dir);

        let history: Vec<HistoryItem> = std::fs::read_to_string(&history_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let permits = settings.max_concurrent.clamp(1, 16) as usize;
        Self {
            app_handle,
            tasks: Mutex::new(HashMap::new()),
            processes: AsyncMutex::new(HashMap::new()),
            history: Mutex::new(history),
            settings: Mutex::new(settings),
            settings_path,
            history_path,
            semaphore: Mutex::new(Arc::new(Semaphore::new(permits))),
        }
    }

    pub fn save_settings(&self) -> std::io::Result<()> {
        let settings = self.settings.lock().clone();
        let permits = settings.max_concurrent.clamp(1, 16) as usize;
        *self.semaphore.lock() = Arc::new(Semaphore::new(permits));
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        atomic_write(&self.settings_path, json.as_bytes())
    }

    pub fn save_history(&self) -> std::io::Result<()> {
        let history = self.history.lock().clone();
        let json = serde_json::to_string_pretty(&history)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        atomic_write(&self.history_path, json.as_bytes())
    }

    pub fn push_history(&self, item: HistoryItem) {
        let mut hist = self.history.lock();
        hist.insert(0, item);
        if hist.len() > 500 {
            hist.truncate(500);
        }
        drop(hist);
        let _ = self.save_history();
    }

    pub fn semaphore_handle(&self) -> Arc<Semaphore> {
        self.semaphore.lock().clone()
    }
}

fn atomic_write(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
