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

/// Strip macOS Gatekeeper quarantine xattr from a downloaded binary.
/// No-op on non-macOS or when the attr is absent.
#[cfg(target_os = "macos")]
fn clear_quarantine(path: &std::path::Path) {
    let _ = std::process::Command::new("xattr")
        .arg("-d")
        .arg("com.apple.quarantine")
        .arg(path)
        .output();
}
#[cfg(not(target_os = "macos"))]
fn clear_quarantine(_path: &std::path::Path) {}

/// Build an HTTP client tuned for large binary downloads from GitHub.
/// Forces HTTP/1.1 because reqwest's HTTP/2 streaming + rustls has known
/// flow-control issues that surface as "error decoding response body"
/// partway through ~40 MB+ downloads.
fn download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("FeiNiao-Downloader/1.0")
        .http1_only()
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))
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

/// Streams a binary download into `tmp_path` with HTTP/1.1 + retry on
/// transient errors. Emits progress on `progress_event`. Returns
/// (downloaded_bytes, total_bytes_or_zero).
async fn stream_download_with_retry(
    url: &str,
    tmp_path: &std::path::Path,
    app: &AppHandle,
    progress_event: &str,
    label: &str,
) -> Result<(u64, u64), String> {
    use futures_util::StreamExt;
    use std::io::Write;

    const MAX_RETRIES: u32 = 3;
    let client = download_client()?;

    let mut last_err: Option<String> = None;
    for attempt in 0..MAX_RETRIES {
        // Recreate the file on every attempt so partial bytes don't accumulate.
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(format!("下载 {} 失败: {}", label, e));
                continue;
            }
        };
        if !resp.status().is_success() {
            return Err(format!(
                "下载 {} 失败（HTTP {}）。请检查网络或代理。",
                label,
                resp.status()
            ));
        }
        let total = resp.content_length().unwrap_or(0);

        let mut file = std::fs::File::create(tmp_path)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        let mut downloaded: u64 = 0;
        let mut stream = resp.bytes_stream();
        let mut stream_err: Option<String> = None;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Err(e) = file.write_all(&bytes) {
                        return Err(format!("写入失败: {}", e));
                    }
                    downloaded += bytes.len() as u64;
                    if total > 0 {
                        let percent = (downloaded as f32 / total as f32) * 100.0;
                        let _ = app.emit(
                            progress_event,
                            serde_json::json!({
                                "percent": percent,
                                "downloaded": downloaded,
                                "total": total,
                            }),
                        );
                    }
                }
                Err(e) => {
                    stream_err = Some(format!("下载中断: {}", e));
                    break;
                }
            }
        }
        drop(file);
        if let Some(e) = stream_err {
            last_err = Some(e);
            // Backoff before retry.
            let wait = std::time::Duration::from_millis(800 * (attempt as u64 + 1));
            log::warn!("download retry {}/{} after {:?}", attempt + 1, MAX_RETRIES, wait);
            tokio::time::sleep(wait).await;
            continue;
        }
        // Sanity: if Content-Length was provided, ensure we got the full thing.
        if total > 0 && downloaded < total {
            last_err = Some(format!(
                "下载不完整（收到 {} / 总计 {}），将重试",
                downloaded, total
            ));
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            continue;
        }
        return Ok((downloaded, total));
    }
    Err(last_err.unwrap_or_else(|| "下载失败（已重试多次）".to_string()))
}

/// Download the official yt-dlp nightly binary to ~/.feiniao/bin/yt-dlp_macos.
/// Emits "install://progress" events with `{percent: f32}` while downloading.
#[tauri::command]
pub async fn install_ytdlp(app: AppHandle) -> Result<String, String> {
    let target = crate::ytdlp::install_target_path()
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建安装目录失败: {}", e))?;
    }

    // OS-specific nightly binary URL.
    let url = crate::ytdlp::ytdlp_download_url();
    let tmp = target.with_extension("tmp");
    let (downloaded, total) = stream_download_with_retry(
        url.as_str(),
        &tmp,
        &app,
        "install://progress",
        "yt-dlp",
    )
    .await?;

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
    clear_quarantine(&target);

    let _ = app.emit("install://progress", serde_json::json!({
        "percent": 100.0_f32,
        "downloaded": downloaded,
        "total": total,
        "done": true,
    }));

    Ok(target.to_string_lossy().to_string())
}

/// Download a static ffmpeg binary to ~/.feiniao/bin/ffmpeg(.exe).
/// Emits "ffmpeg-install://progress" events with `{percent: f32}` while downloading.
#[tauri::command]
pub async fn install_ffmpeg(app: AppHandle) -> Result<String, String> {
    let url = crate::ytdlp::ffmpeg_download_url()
        .ok_or_else(|| "当前平台暂不支持自动安装 ffmpeg，请手动安装。".to_string())?;
    let target = crate::ytdlp::ffmpeg_install_target_path()
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建安装目录失败: {}", e))?;
    }

    let tmp = target.with_extension("tmp");
    let (downloaded, total) = stream_download_with_retry(
        url,
        &tmp,
        &app,
        "ffmpeg-install://progress",
        "ffmpeg",
    )
    .await?;

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

    std::fs::rename(&tmp, &target).map_err(|e| format!("移动文件失败: {}", e))?;
    clear_quarantine(&target);

    let _ = app.emit(
        "ffmpeg-install://progress",
        serde_json::json!({
            "percent": 100.0_f32,
            "downloaded": downloaded,
            "total": total,
            "done": true,
        }),
    );

    Ok(target.to_string_lossy().to_string())
}

/// Compare two semver-ish version strings like "1.2.1" vs "1.3.0".
/// Returns Ordering::Less if `a < b`. Tags like "v1.2.1" are tolerated.
fn cmp_version(a: &str, b: &str) -> std::cmp::Ordering {
    let strip = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .trim_start_matches('V')
            .split('.')
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let av = strip(a);
    let bv = strip(b);
    let n = av.len().max(bv.len());
    for i in 0..n {
        let av_i = av.get(i).copied().unwrap_or(0);
        let bv_i = bv.get(i).copied().unwrap_or(0);
        match av_i.cmp(&bv_i) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Choose the platform-appropriate release asset by name.
fn platform_asset_keywords() -> Vec<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return vec!["AppleSilicon", "aarch64", "arm64", ".dmg"];
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return vec!["Intel", "x64", "x86_64", ".dmg"];
    #[cfg(target_os = "windows")]
    return vec![".exe", "Windows", "x64"];
    #[cfg(target_os = "linux")]
    return vec![".AppImage", "Linux", "x86_64"];
}

#[allow(dead_code)]
fn platform_asset_ext() -> &'static str {
    #[cfg(target_os = "macos")]
    return ".dmg";
    #[cfg(target_os = "windows")]
    return ".exe";
    #[cfg(target_os = "linux")]
    return ".AppImage";
}

/// Check GitHub for a newer release of 飞鸟下载器.
#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let api = "https://api.github.com/repos/Yorushika-fan/feiniao-downloader/releases/latest";
    let client = reqwest::Client::builder()
        .user_agent("FeiNiao-Downloader/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP 客户端初始化失败: {}", e))?;
    let resp = client.get(api).send().await.map_err(|e| format!("检查更新失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub 返回错误：HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 GitHub 响应失败: {}", e))?;

    let tag = json.get("tag_name").and_then(|x| x.as_str()).map(String::from);
    let release_name = json.get("name").and_then(|x| x.as_str()).map(String::from);
    let release_notes = json.get("body").and_then(|x| x.as_str()).map(String::from);
    let release_url = json.get("html_url").and_then(|x| x.as_str()).map(String::from);
    let published_at = json
        .get("published_at")
        .and_then(|x| x.as_str())
        .map(String::from);

    let mut asset_url: Option<String> = None;
    let mut asset_name: Option<String> = None;
    if let Some(assets) = json.get("assets").and_then(|x| x.as_array()) {
        let keywords = platform_asset_keywords();
        let primary_ext = keywords
            .iter()
            .find(|k| k.starts_with('.'))
            .copied()
            .unwrap_or("");
        for a in assets {
            let name = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let url = a
                .get("browser_download_url")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if name.is_empty() || url.is_empty() {
                continue;
            }
            if !primary_ext.is_empty() && !name.to_ascii_lowercase().ends_with(primary_ext) {
                continue;
            }
            let lc = name.to_ascii_lowercase();
            let score = keywords
                .iter()
                .filter(|k| lc.contains(&k.to_ascii_lowercase()))
                .count();
            if score >= 1 || asset_url.is_none() {
                asset_url = Some(url.to_string());
                asset_name = Some(name.to_string());
                if score >= 2 {
                    break;
                }
            }
        }
    }

    let has_update = match tag.as_deref() {
        Some(t) => cmp_version(&current, t) == std::cmp::Ordering::Less,
        None => false,
    };

    Ok(UpdateInfo {
        current_version: current,
        latest_version: tag,
        has_update,
        release_name,
        release_notes,
        release_url,
        asset_url,
        asset_name,
        published_at,
    })
}

/// Download the new release asset and open it with the OS handler so the user
/// can install it (mount DMG, run EXE, launch AppImage). Emits
/// "update-install://progress" events.
#[tauri::command]
pub async fn install_update(url: String, app: AppHandle) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;

    let parsed = url::Url::parse(&url).map_err(|e| format!("无效链接: {}", e))?;
    if parsed.scheme() != "https" {
        return Err("仅允许 https 链接".into());
    }
    let host = parsed.host_str().unwrap_or("");
    if !(host == "github.com" || host.ends_with(".github.com") || host == "objects.githubusercontent.com") {
        return Err("只允许下载 github.com 上的发行版".into());
    }

    let raw_name = parsed
        .path_segments()
        .and_then(|s| s.last())
        .filter(|s| !s.is_empty())
        .unwrap_or("FeiNiao-update")
        .to_string();
    // Some tauri-action releases produce stripped names like "_1.3.1_aarch64.dmg"
    // (the CJK product name was removed). Add a stable prefix so OS prompts and
    // file managers still show a meaningful filename.
    let filename = if raw_name.starts_with('_') {
        format!("FeiNiao-Downloader{}", raw_name)
    } else {
        raw_name
    };
    let temp_dir = std::env::temp_dir().join("feiniao-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let target = temp_dir.join(&filename);
    let tmp = target.with_extension("part");

    let (downloaded, total) = stream_download_with_retry(
        &url,
        &tmp,
        &app,
        "update-install://progress",
        "update",
    )
    .await?;

    #[cfg(unix)]
    if filename.to_ascii_lowercase().ends_with(".appimage") {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)
            .map_err(|e| format!("无法获取文件权限: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&tmp, perms);
    }

    std::fs::rename(&tmp, &target).map_err(|e| format!("移动文件失败: {}", e))?;

    let _ = app.emit(
        "update-install://progress",
        serde_json::json!({
            "percent": 100.0_f32,
            "downloaded": downloaded,
            "total": total,
            "done": true,
        }),
    );

    let target_str = target.to_string_lossy().to_string();
    // Hand off to OS — mounts DMG / runs EXE / launches AppImage. If the OS
    // handler rejects it (e.g. Gatekeeper on an unsigned DMG), fall back to
    // revealing the installer in the file manager so the user can run it.
    if let Err(open_err) = app.opener().open_path(&target_str, None::<&str>) {
        log::warn!("opener.open_path 失败 ({open_err})，改为在 Finder/Explorer 中显示");
        if let Err(reveal_err) = app.opener().reveal_item_in_dir(&target_str) {
            return Err(format!(
                "下载完成但无法自动打开（{}）。安装包已保存到：{}",
                reveal_err, target_str
            ));
        }
    }

    Ok(target_str)
}
