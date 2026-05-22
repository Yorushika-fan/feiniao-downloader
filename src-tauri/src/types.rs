use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub release_name: Option<String>,
    pub release_notes: Option<String>,
    pub release_url: Option<String>,
    pub asset_url: Option<String>,
    pub asset_name: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub ffmpeg_installed: bool,
    pub ffmpeg_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcodec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acodec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesize: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tbr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_note: Option<String>,
    pub has_video: bool,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploader: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webpage_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,
    pub is_playlist: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist_count: Option<u32>,
    pub formats: Vec<VideoFormat>,
    pub subtitles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub mode: DownloadMode,
    pub output_dir: String,
    pub filename_template: Option<String>,
    pub subtitles: SubtitlesOption,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub write_thumbnail: bool,
    pub speed_limit: Option<String>,
    pub proxy: Option<String>,
    pub cookies_from_browser: Option<String>,
    pub playlist_items: Option<String>,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DownloadMode {
    Video {
        quality: String,
        container: String,
    },
    Audio {
        format: String,
        quality: String,
    },
    Custom {
        format_selector: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubtitlesOption {
    pub enabled: bool,
    pub auto_generated: bool,
    pub embed: bool,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub status: TaskStatus,
    pub progress: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub size_total: Option<String>,
    pub size_downloaded: Option<String>,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Postprocessing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub task_id: String,
    pub percent: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub size_total: Option<String>,
    pub size_downloaded: Option<String>,
    pub status: TaskStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub output_path: Option<String>,
    pub status: TaskStatus,
    pub mode: String,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub output_dir: String,
    pub filename_template: String,
    pub default_quality: String,
    pub default_container: String,
    pub default_audio_format: String,
    pub theme: String,
    pub language: String,
    pub speed_limit: Option<String>,
    pub proxy: Option<String>,
    pub cookies_from_browser: Option<String>,
    pub max_concurrent: u32,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub write_thumbnail: bool,
    pub ytdlp_path: Option<String>,
    pub ffmpeg_path: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.join("Downloads").join("FeiNiao"))
            .unwrap_or_else(|| std::path::PathBuf::from("./Downloads"));
        Self {
            output_dir: home.to_string_lossy().to_string(),
            filename_template: "%(title)s.%(ext)s".to_string(),
            default_quality: "1080".to_string(),
            default_container: "mp4".to_string(),
            default_audio_format: "mp3".to_string(),
            theme: "system".to_string(),
            language: "zh-CN".to_string(),
            speed_limit: None,
            proxy: None,
            cookies_from_browser: None,
            max_concurrent: 2,
            embed_metadata: true,
            embed_thumbnail: false,
            write_thumbnail: false,
            ytdlp_path: None,
            ffmpeg_path: None,
        }
    }
}
