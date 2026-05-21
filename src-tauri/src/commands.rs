use crate::state::AppState;
use crate::types::*;
use crate::ytdlp;
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type AppStateRef<'a> = State<'a, Arc<AppState>>;

fn err_to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Reject system-sensitive download targets to prevent users (or a compromised
/// WebView) from writing to OS directories. Allow ~/Downloads, ~/Movies,
/// ~/Music, ~/Documents, ~/Desktop and anywhere under the user's home dir.
fn validate_output_dir(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("下载目录必须是绝对路径".into());
    }
    // Forbid traversal segments like "..".
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("下载目录不能包含 .. 段".into());
    }
    let home = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
    if !p.starts_with(&home) {
        return Err(format!(
            "下载目录必须位于用户目录下（{}）",
            home.display()
        ));
    }
    // Forbid critical Apple-managed subdirs.
    let forbidden = [
        home.join("Library").join("Application Support"),
        home.join("Library").join("LaunchAgents"),
        home.join("Library").join("Keychains"),
    ];
    for f in &forbidden {
        if p.starts_with(f) {
            return Err("不允许写入系统受管目录".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn check_ytdlp(state: AppStateRef<'_>) -> Result<YtDlpStatus, String> {
    let (ytdlp_hint, ffmpeg_hint) = {
        let s = state.settings.lock();
        (s.ytdlp_path.clone(), s.ffmpeg_path.clone())
    };
    Ok(ytdlp::check_status(ytdlp_hint.as_deref(), ffmpeg_hint.as_deref()).await)
}

#[tauri::command]
pub async fn probe_url(url: String, state: AppStateRef<'_>) -> Result<VideoInfo, String> {
    let (ytdlp_hint, proxy, cookies) = {
        let s = state.settings.lock();
        (
            s.ytdlp_path.clone(),
            s.proxy.clone(),
            s.cookies_from_browser.clone(),
        )
    };
    ytdlp::probe(
        &url,
        ytdlp_hint.as_deref(),
        proxy.as_deref(),
        cookies.as_deref(),
    )
    .await
    .map_err(err_to_string)
}

#[tauri::command]
pub async fn start_download(
    request: DownloadRequest,
    app: AppHandle,
    state: AppStateRef<'_>,
) -> Result<String, String> {
    validate_output_dir(&request.output_dir)?;
    if let Err(e) = std::fs::create_dir_all(&request.output_dir) {
        return Err(format!("无法创建下载目录: {}", e));
    }

    let settings = { state.settings.lock().clone() };
    let app_state: Arc<AppState> = state.inner().clone();

    let task_id = uuid::Uuid::new_v4().to_string();
    let task = DownloadTask {
        id: task_id.clone(),
        url: request.url.clone(),
        title: request
            .title
            .clone()
            .unwrap_or_else(|| request.url.clone()),
        thumbnail: request.thumbnail.clone(),
        status: TaskStatus::Pending,
        progress: 0.0,
        speed: None,
        eta: None,
        size_total: None,
        size_downloaded: None,
        output_path: None,
        error: None,
        created_at: Utc::now().timestamp(),
        finished_at: None,
    };

    app_state
        .tasks
        .lock()
        .insert(task_id.clone(), task.clone());

    // Throttle concurrent downloads via a semaphore tied to user settings.
    let semaphore = app_state.semaphore_handle();
    let app_state_spawn = app_state.clone();
    let app_spawn = app.clone();
    let request_spawn = request.clone();
    let task_id_spawn = task_id.clone();
    let settings_spawn = settings.clone();
    tokio::spawn(async move {
        let _permit = match semaphore.acquire_owned().await {
            Ok(p) => p,
            Err(_) => return,
        };
        run_download(
            app_state_spawn,
            app_spawn,
            task_id_spawn,
            request_spawn,
            settings_spawn,
        )
        .await;
    });

    Ok(task_id)
}

async fn run_download(
    app_state: Arc<AppState>,
    app: AppHandle,
    task_id: String,
    request: DownloadRequest,
    settings: AppSettings,
) {
    let mut spawned = match ytdlp::spawn_download(&request, &settings).await {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            mutate_task(&app_state, &task_id, |t| {
                t.status = TaskStatus::Failed;
                t.error = Some(msg.clone());
                t.finished_at = Some(Utc::now().timestamp());
            });
            emit_for(&app, &app_state, &task_id, Some(msg));
            return;
        }
    };

    mutate_task(&app_state, &task_id, |t| t.status = TaskStatus::Running);
    emit_for(&app, &app_state, &task_id, Some("开始下载".into()));

    let (stdout, stderr) = ytdlp::make_reader(&mut spawned.child);

    app_state
        .processes
        .lock()
        .await
        .insert(task_id.clone(), spawned.child);

    monitor_process(app_state, app, task_id, request, stdout, stderr).await;
}

async fn monitor_process(
    state: Arc<AppState>,
    app: AppHandle,
    task_id: String,
    request: DownloadRequest,
    stdout: Option<tokio::io::BufReader<tokio::process::ChildStdout>>,
    stderr: Option<tokio::io::BufReader<tokio::process::ChildStderr>>,
) {
    // Read stdout and stderr concurrently to avoid pipe-buffer deadlock.
    let state_so = state.clone();
    let app_so = app.clone();
    let tid_so = task_id.clone();
    let stdout_fut = async move {
        if let Some(mut so) = stdout {
            ytdlp::read_lines(&mut so, move |line| {
                let parsed = ytdlp::parse_progress_line(&line);
                mutate_task(&state_so, &tid_so, |t| {
                    if let Some(p) = parsed.percent {
                        t.progress = p;
                    }
                    if let Some(ref s) = parsed.speed {
                        t.speed = Some(s.clone());
                    }
                    if let Some(ref e) = parsed.eta {
                        t.eta = Some(e.clone());
                    }
                    if let Some(ref tot) = parsed.total {
                        t.size_total = Some(tot.clone());
                    }
                    if let Some(ref op) = parsed.output_path {
                        t.output_path = Some(op.clone());
                    }
                    if let Some(ref cs) = parsed.completed_size {
                        t.size_downloaded = Some(cs.clone());
                    }
                    if parsed.postprocessing && t.status == TaskStatus::Running {
                        t.status = TaskStatus::Postprocessing;
                    }
                });
                emit_for(&app_so, &state_so, &tid_so, Some(line));
            })
            .await;
        }
    };

    let stderr_fut = async {
        let mut buf: Vec<String> = Vec::new();
        if let Some(mut se) = stderr {
            ytdlp::read_lines(&mut se, |line| {
                buf.push(line);
            })
            .await;
        }
        buf
    };

    let (_, stderr_lines) = tokio::join!(stdout_fut, stderr_fut);

    let exit_status = {
        let mut procs = state.processes.lock().await;
        if let Some(mut child) = procs.remove(&task_id) {
            child.wait().await.ok()
        } else {
            None
        }
    };
    let success = exit_status.map(|s| s.success()).unwrap_or(false);

    let cancelled = state
        .tasks
        .lock()
        .get(&task_id)
        .map(|t| t.status == TaskStatus::Cancelled)
        .unwrap_or(false);

    let final_status = if success {
        TaskStatus::Completed
    } else if cancelled {
        TaskStatus::Cancelled
    } else {
        TaskStatus::Failed
    };

    let err_msg = if final_status == TaskStatus::Failed {
        let joined = stderr_lines.join("\n");
        if joined.trim().is_empty() {
            Some("下载失败，请检查链接或网络".to_string())
        } else {
            Some(ytdlp::humanize_error(&joined, &request.url))
        }
    } else {
        None
    };

    let snapshot = {
        let mut t = state.tasks.lock();
        if let Some(task) = t.get_mut(&task_id) {
            task.status = final_status.clone();
            task.finished_at = Some(Utc::now().timestamp());
            if final_status == TaskStatus::Completed {
                task.progress = 100.0;
            }
            if let Some(ref m) = err_msg {
                task.error = Some(m.clone());
            }
            Some(task.clone())
        } else {
            None
        }
    };

    if let Some(task) = snapshot {
        emit_for(&app, &state, &task_id, task.error.clone());

        let mode_str = match &request.mode {
            DownloadMode::Video { quality, container } => {
                format!("视频 · {} · {}", quality, container)
            }
            DownloadMode::Audio { format, .. } => format!("音频 · {}", format),
            DownloadMode::Custom { format_selector } => {
                format!("自定义 · {}", format_selector)
            }
        };

        state.push_history(HistoryItem {
            id: task.id.clone(),
            url: task.url.clone(),
            title: task.title.clone(),
            thumbnail: task.thumbnail.clone(),
            output_path: task.output_path.clone(),
            status: task.status.clone(),
            mode: mode_str,
            created_at: task.created_at,
            finished_at: task.finished_at,
        });
    }
}

fn mutate_task<F: FnOnce(&mut DownloadTask)>(
    state: &Arc<AppState>,
    task_id: &str,
    f: F,
) {
    if let Some(t) = state.tasks.lock().get_mut(task_id) {
        f(t);
    }
}

fn emit_for(
    app: &AppHandle,
    state: &Arc<AppState>,
    task_id: &str,
    message: Option<String>,
) {
    let task = state.tasks.lock().get(task_id).cloned();
    if let Some(t) = task {
        let payload = ProgressUpdate {
            task_id: t.id,
            percent: t.progress,
            speed: t.speed,
            eta: t.eta,
            size_total: t.size_total,
            size_downloaded: t.size_downloaded,
            status: t.status,
            message,
        };
        let _ = app.emit("download://progress", payload);
    }
}

#[tauri::command]
pub async fn cancel_download(task_id: String, state: AppStateRef<'_>) -> Result<(), String> {
    let mut procs = state.processes.lock().await;
    if let Some(mut child) = procs.remove(&task_id) {
        let _ = child.kill().await;
    }
    if let Some(t) = state.tasks.lock().get_mut(&task_id) {
        t.status = TaskStatus::Cancelled;
        t.finished_at = Some(Utc::now().timestamp());
    }
    Ok(())
}

#[tauri::command]
pub fn list_tasks(state: AppStateRef<'_>) -> Result<Vec<DownloadTask>, String> {
    let tasks = state.tasks.lock();
    let mut v: Vec<DownloadTask> = tasks.values().cloned().collect();
    v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(v)
}

#[tauri::command]
pub fn get_history(state: AppStateRef<'_>) -> Result<Vec<HistoryItem>, String> {
    Ok(state.history.lock().clone())
}

#[tauri::command]
pub fn clear_history(state: AppStateRef<'_>) -> Result<(), String> {
    state.history.lock().clear();
    state.save_history().map_err(err_to_string)
}

#[tauri::command]
pub fn delete_history_item(id: String, state: AppStateRef<'_>) -> Result<(), String> {
    state.history.lock().retain(|h| h.id != id);
    state.save_history().map_err(err_to_string)
}

#[tauri::command]
pub fn get_settings(state: AppStateRef<'_>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().clone())
}

#[tauri::command]
pub fn save_settings(settings: AppSettings, state: AppStateRef<'_>) -> Result<(), String> {
    *state.settings.lock() = settings;
    state.save_settings().map_err(err_to_string)
}

/// Verify that yt-dlp can extract cookies from the given browser.
/// Returns the number of extracted cookies, or an error message.
#[tauri::command]
pub async fn test_cookies(
    browser: String,
    state: AppStateRef<'_>,
) -> Result<u32, String> {
    let ytdlp_hint = { state.settings.lock().ytdlp_path.clone() };
    let bin = ytdlp::find_binary("yt-dlp", ytdlp_hint.as_deref())
        .ok_or_else(|| "未找到 yt-dlp".to_string())?;
    let output = tokio::process::Command::new(&bin)
        .arg("-v")
        .arg("--cookies-from-browser")
        .arg(&browser)
        .arg("--simulate")
        .arg("--skip-download")
        .arg("--print")
        .arg("nothing")
        .arg("--no-warnings")
        .arg("https://www.youtube.com/")
        .env("PATH", ytdlp::enriched_path_public())
        .output()
        .await
        .map_err(|e| format!("调用 yt-dlp 失败: {}", e))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let count_re = regex::Regex::new(r"Extracted (\d+) cookies").unwrap();
    if let Some(caps) = count_re.captures(&combined) {
        if let Ok(n) = caps[1].parse::<u32>() {
            return Ok(n);
        }
    }
    // Error scenarios
    let low = combined.to_ascii_lowercase();
    if low.contains("could not find") && low.contains("browser") {
        return Err(format!("未检测到 {} 浏览器或其数据目录", browser));
    }
    if low.contains("permission denied") || low.contains("operation not permitted") {
        return Err(format!(
            "macOS 阻止读取 {} 数据。请在「系统设置 → 隐私与安全性 → App 管理 / 完全磁盘访问」中允许「飞鸟下载器」。",
            browser
        ));
    }
    if low.contains("could not decrypt") {
        return Err(format!(
            "无法解密 {} cookies。这通常因为浏览器正在运行或主密码访问被拒绝。请尝试关闭 {} 后重试。",
            browser, browser
        ));
    }
    Err(format!("未能从 {} 读取 cookies。建议改用其他已登录的浏览器。", browser))
}

/// Detect proxies running on the local machine.
#[tauri::command]
pub async fn detect_proxy() -> Result<Vec<crate::proxy::ProxyCandidate>, String> {
    Ok(crate::proxy::detect_all().await)
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path.map(|p| p.to_string()));
    });
    rx.await.map_err(err_to_string)
}

#[tauri::command]
pub fn reveal_in_finder(path: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let p = PathBuf::from(&path);
    let target = if p.is_file() {
        p.parent().map(|x| x.to_path_buf()).unwrap_or(p)
    } else {
        p
    };
    app.opener()
        .reveal_item_in_dir(target)
        .map_err(err_to_string)
}

/// Open a file with macOS default application (like double-clicking in Finder).
#[tauri::command]
pub fn open_file(path: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !std::path::Path::new(&path).exists() {
        return Err(format!("文件不存在: {}", path));
    }
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(err_to_string)
}

#[tauri::command]
pub fn open_external(url: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let parsed = url::Url::parse(&url).map_err(|e| format!("无效链接: {}", e))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("仅允许 http/https 链接（当前: {}）", scheme));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(err_to_string)
}

#[tauri::command]
pub fn default_download_dir() -> Result<String, String> {
    let p = dirs::home_dir()
        .map(|p| p.join("Downloads").join("FeiNiao"))
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    let _ = std::fs::create_dir_all(&p);
    Ok(p.to_string_lossy().to_string())
}

/// Download the official yt-dlp nightly binary to ~/.feiniao/bin/yt-dlp_macos.
/// Emits "install://progress" events with `{percent: f32}` while downloading.
#[tauri::command]
pub async fn install_ytdlp(app: AppHandle) -> Result<String, String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let target = crate::ytdlp::install_target_path()
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建安装目录失败: {}", e))?;
    }

    // OS-specific nightly binary URL.
    let url = crate::ytdlp::ytdlp_download_url();
    let url = url.as_str();
    let client = reqwest::Client::builder()
        .user_agent("FeiNiao-Downloader/1.0")
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载 yt-dlp 失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载 yt-dlp 失败（HTTP {}）。请检查网络或代理。", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    // Write to .tmp then rename for atomicity.
    let tmp = target.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("写入文件失败: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("下载中断: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("写入失败: {}", e))?;
        downloaded += bytes.len() as u64;
        if total > 0 {
            let percent = (downloaded as f32 / total as f32) * 100.0;
            let _ = app.emit("install://progress", serde_json::json!({
                "percent": percent,
                "downloaded": downloaded,
                "total": total,
            }));
        }
    }
    file.flush().map_err(|e| format!("写入完成失败: {}", e))?;
    drop(file);

    // chmod +x on Unix; Windows .exe is executable as-is.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)
            .map_err(|e| format!("无法获取文件权限: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)
            .map_err(|e| format!("设置可执行权限失败: {}", e))?;
    }

    // Atomic move into final location
    std::fs::rename(&tmp, &target).map_err(|e| format!("移动文件失败: {}", e))?;

    let _ = app.emit("install://progress", serde_json::json!({
        "percent": 100.0_f32,
        "downloaded": downloaded,
        "total": total,
        "done": true,
    }));

    Ok(target.to_string_lossy().to_string())
}
