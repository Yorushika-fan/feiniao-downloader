import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface YtDlpStatus {
  installed: boolean;
  version: string | null;
  path: string | null;
  ffmpeg_installed: boolean;
  ffmpeg_path: string | null;
}

export interface VideoFormat {
  format_id: string;
  ext: string;
  resolution?: string;
  fps?: number;
  vcodec?: string;
  acodec?: string;
  filesize?: number;
  tbr?: number;
  format_note?: string;
  has_video: boolean;
  has_audio: boolean;
}

export interface VideoInfo {
  id: string;
  title: string;
  description?: string;
  uploader?: string;
  channel?: string;
  duration?: number;
  thumbnail?: string;
  webpage_url?: string;
  view_count?: number;
  upload_date?: string;
  extractor?: string;
  is_playlist: boolean;
  playlist_count?: number;
  formats: VideoFormat[];
  subtitles: string[];
}

export type DownloadMode =
  | { kind: "video"; quality: string; container: string }
  | { kind: "audio"; format: string; quality: string }
  | { kind: "custom"; format_selector: string };

export interface SubtitlesOption {
  enabled: boolean;
  auto_generated: boolean;
  embed: boolean;
  languages: string[];
}

export interface DownloadRequest {
  url: string;
  title: string | null;
  thumbnail: string | null;
  mode: DownloadMode;
  output_dir: string;
  filename_template: string | null;
  subtitles: SubtitlesOption;
  embed_metadata: boolean;
  embed_thumbnail: boolean;
  write_thumbnail: boolean;
  speed_limit: string | null;
  proxy: string | null;
  cookies_from_browser: string | null;
  playlist_items: string | null;
  extra_args: string[];
}

export type TaskStatus =
  | "pending"
  | "running"
  | "postprocessing"
  | "completed"
  | "failed"
  | "cancelled";

export interface DownloadTask {
  id: string;
  url: string;
  title: string;
  thumbnail: string | null;
  status: TaskStatus;
  progress: number;
  speed: string | null;
  eta: string | null;
  size_total: string | null;
  size_downloaded: string | null;
  output_path: string | null;
  error: string | null;
  created_at: number;
  finished_at: number | null;
}

export interface ProgressUpdate {
  task_id: string;
  percent: number;
  speed: string | null;
  eta: string | null;
  size_total: string | null;
  size_downloaded: string | null;
  status: TaskStatus;
  message: string | null;
}

export interface HistoryItem {
  id: string;
  url: string;
  title: string;
  thumbnail: string | null;
  output_path: string | null;
  status: TaskStatus;
  mode: string;
  created_at: number;
  finished_at: number | null;
}

export interface ProxyCandidate {
  kind: "http" | "socks5" | "system" | string;
  url: string;
  label: string;
  source: "scan" | "system" | "env" | string;
}

export interface AppSettings {
  output_dir: string;
  filename_template: string;
  default_quality: string;
  default_container: string;
  default_audio_format: string;
  theme: "light" | "dark" | "system";
  language: string;
  speed_limit: string | null;
  proxy: string | null;
  cookies_from_browser: string | null;
  max_concurrent: number;
  embed_metadata: boolean;
  embed_thumbnail: boolean;
  write_thumbnail: boolean;
  ytdlp_path: string | null;
  ffmpeg_path: string | null;
}

export interface InstallProgress {
  percent: number;
  downloaded: number;
  total: number;
  done?: boolean;
}

export interface UpdateInfo {
  current_version: string;
  latest_version: string | null;
  has_update: boolean;
  release_name: string | null;
  release_notes: string | null;
  release_url: string | null;
  asset_url: string | null;
  asset_name: string | null;
  published_at: string | null;
}

export const api = {
  checkYtDlp: () => invoke<YtDlpStatus>("check_ytdlp"),
  probeUrl: (url: string) => invoke<VideoInfo>("probe_url", { url }),
  startDownload: (request: DownloadRequest) =>
    invoke<string>("start_download", { request }),
  cancelDownload: (taskId: string) =>
    invoke<void>("cancel_download", { taskId }),
  listTasks: () => invoke<DownloadTask[]>("list_tasks"),
  getHistory: () => invoke<HistoryItem[]>("get_history"),
  clearHistory: () => invoke<void>("clear_history"),
  deleteHistoryItem: (id: string) => invoke<void>("delete_history_item", { id }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) =>
    invoke<void>("save_settings", { settings }),
  testCookies: (browser: string) =>
    invoke<number>("test_cookies", { browser }),
  detectProxy: () => invoke<ProxyCandidate[]>("detect_proxy"),
  pickDirectory: () => invoke<string | null>("pick_directory"),
  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  defaultDownloadDir: () => invoke<string>("default_download_dir"),
  installYtDlp: () => invoke<string>("install_ytdlp"),
  installFfmpeg: () => invoke<string>("install_ffmpeg"),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  installUpdate: (url: string) => invoke<string>("install_update", { url }),
};

export function onInstallProgress(
  cb: (p: InstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<InstallProgress>("install://progress", (e) => cb(e.payload));
}

export function onFfmpegInstallProgress(
  cb: (p: InstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<InstallProgress>("ffmpeg-install://progress", (e) =>
    cb(e.payload),
  );
}

export function onUpdateInstallProgress(
  cb: (p: InstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<InstallProgress>("update-install://progress", (e) =>
    cb(e.payload),
  );
}

export function onDownloadProgress(
  cb: (p: ProgressUpdate) => void,
): Promise<UnlistenFn> {
  return listen<ProgressUpdate>("download://progress", (e) => cb(e.payload));
}
