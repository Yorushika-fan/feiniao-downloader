use crate::types::*;
use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// OS-aware search paths for `yt-dlp` / `ffmpeg` lookups.
fn os_extra_paths() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let home = dirs::home_dir();

    #[cfg(target_os = "macos")]
    {
        if let Some(h) = home.as_ref() {
            // Prefer pip-installed nightly (curl_cffi support, fresher fixes).
            for v in ["3.14", "3.13", "3.12", "3.11"] {
                out.push(h.join(format!("Library/Python/{}/bin", v)));
            }
            out.push(h.join(".local/bin"));
        }
        for p in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin"] {
            out.push(PathBuf::from(p));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(h) = home.as_ref() {
            out.push(h.join(r"AppData\Local\Programs\Python\Python313\Scripts"));
            out.push(h.join(r"AppData\Local\Programs\Python\Python312\Scripts"));
            out.push(h.join(r"AppData\Local\Programs\Python\Python311\Scripts"));
            out.push(h.join(r"AppData\Roaming\Python\Scripts"));
        }
        out.push(PathBuf::from(r"C:\Program Files\yt-dlp"));
        out.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
        out.push(PathBuf::from(r"C:\tools\yt-dlp"));
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(h) = home.as_ref() {
            out.push(h.join(".local/bin"));
            out.push(h.join("bin"));
        }
        for p in ["/usr/local/bin", "/usr/bin", "/snap/bin", "/var/lib/flatpak/exports/bin"] {
            out.push(PathBuf::from(p));
        }
    }

    out
}

/// Filename for the prebuilt yt-dlp single-file binary on this OS.
pub fn ytdlp_binary_filename() -> &'static str {
    #[cfg(target_os = "macos")]
    return "yt-dlp_macos";
    #[cfg(target_os = "windows")]
    return "yt-dlp.exe";
    #[cfg(target_os = "linux")]
    return {
        // PyInstaller standalone (musllinux) covers most distros.
        if cfg!(target_arch = "aarch64") {
            "yt-dlp_linux_aarch64"
        } else {
            "yt-dlp_linux"
        }
    };
}

/// Direct download URL for the OS-appropriate yt-dlp nightly binary.
pub fn ytdlp_download_url() -> String {
    format!(
        "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/{}",
        ytdlp_binary_filename()
    )
}

/// Local filename for the bundled ffmpeg binary.
pub fn ffmpeg_binary_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    return "ffmpeg.exe";
    #[cfg(not(target_os = "windows"))]
    return "ffmpeg";
}

/// Direct download URL for a static, single-file ffmpeg binary per platform.
/// Uses eugeneware/ffmpeg-static which publishes bare binaries — no archive
/// to extract on the user's machine.
pub fn ffmpeg_download_url() -> Option<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some("https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/darwin-arm64");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Some("https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/darwin-x64");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some("https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/linux-x64");
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Some("https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/linux-arm64");
    #[cfg(target_os = "windows")]
    return Some("https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/win32-x64.exe");
    #[cfg(not(any(
        all(target_os = "macos", any(target_arch = "aarch64", target_arch = "x86_64")),
        all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")),
        target_os = "windows"
    )))]
    return None;
}

/// Target install path for the ffmpeg auto-installer.
pub fn ffmpeg_install_target_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".feiniao").join("bin").join(ffmpeg_binary_filename()))
}

const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36";

/// Detect the host name from a URL (lowercased, no leading "www.").
fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.trim_start_matches("www.").to_string())
}

/// Per-site request-header injections to bypass common anti-scraping checks.
/// Returns extra yt-dlp args (referer, headers, etc.) for the given URL.
fn site_specific_args(url: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let host = match host_of(url) {
        Some(h) => h,
        None => return out,
    };

    // Force a real desktop UA for every site — many CDNs reject the default UA.
    out.push("--user-agent".into());
    out.push(DESKTOP_UA.into());

    if host.ends_with("bilibili.com") || host.ends_with("b23.tv") {
        out.push("--referer".into());
        out.push("https://www.bilibili.com/".into());
        out.push("--add-header".into());
        out.push("Origin:https://www.bilibili.com".into());
        out.push("--add-header".into());
        out.push("Accept-Language:zh-CN,zh;q=0.9,en;q=0.8".into());
        out.push("--add-header".into());
        out.push("Sec-Fetch-Mode:navigate".into());
        // Try the new HTML5 extractor first; falls back automatically.
        out.push("--extractor-args".into());
        out.push("bilibili:format=html5".into());
    } else if host.ends_with("pornhub.com") || host.ends_with("xvideos.com") || host.ends_with("xnxx.com") {
        // Adult sites use TLS fingerprinting (ja3) — impersonate Chrome to bypass HTTP 410.
        out.push("--impersonate".into());
        out.push("chrome".into());
        out.push("--age-limit".into());
        out.push("99".into());
        out.push("--referer".into());
        let referer = format!("https://www.{}/", host);
        out.push(referer);
        out.push("--add-header".into());
        out.push("Accept-Language:en-US,en;q=0.9".into());
    } else if host.ends_with("youtube.com") || host.ends_with("youtu.be") {
        // Force web client (avoids android player throttling on some videos).
        out.push("--extractor-args".into());
        out.push("youtube:player_client=web,web_safari".into());
    } else if host.ends_with("twitter.com") || host.ends_with("x.com") {
        out.push("--referer".into());
        out.push("https://x.com/".into());
    } else if host.ends_with("douyin.com") || host.ends_with("iesdouyin.com") {
        out.push("--referer".into());
        out.push("https://www.douyin.com/".into());
        out.push("--add-header".into());
        out.push("Accept-Language:zh-CN,zh;q=0.9".into());
    } else if host.ends_with("kuaishou.com") {
        out.push("--referer".into());
        out.push("https://www.kuaishou.com/".into());
    } else if host.ends_with("xiaohongshu.com") || host.ends_with("xhslink.com") {
        out.push("--referer".into());
        out.push("https://www.xiaohongshu.com/".into());
    }

    out
}

/// Turn a raw yt-dlp stderr into a friendly Chinese error.
pub fn humanize_error(stderr: &str, url: &str) -> String {
    let stderr_lower = stderr.to_ascii_lowercase();
    let host = host_of(url).unwrap_or_default();
    let site_label = if host.contains("bilibili") {
        "Bilibili"
    } else if host.contains("pornhub") {
        "Pornhub"
    } else if host.contains("youtube") || host.contains("youtu.be") {
        "YouTube"
    } else {
        ""
    };

    if stderr_lower.contains("412") || stderr_lower.contains("precondition failed") {
        return format!(
            "{} 已临时限流当前 IP（HTTP 412）。\n建议：\n  1. 等待数分钟后重试\n  2. 在「设置 → 网络 → 代理」配置一个 HTTP/SOCKS 代理\n  3. 在「设置 → 网络 → 浏览器 Cookies」选择已登录的浏览器导入 Cookie",
            if site_label.is_empty() { "服务器" } else { site_label }
        );
    }
    if stderr_lower.contains("429") || stderr_lower.contains("too many requests") {
        return "请求过于频繁（HTTP 429）。请稍后再试或使用代理。".into();
    }
    if stderr_lower.contains("403") || stderr_lower.contains("forbidden") {
        return "服务器拒绝访问（HTTP 403）。可能需要登录 Cookie 或代理。请前往「设置 → 网络」配置。".into();
    }
    if stderr_lower.contains("404") {
        return "视频不存在或已被删除（HTTP 404）。".into();
    }
    if stderr_lower.contains("private") && stderr_lower.contains("video") {
        return "该视频为私享内容，需登录账号后观看。请在设置中导入浏览器 Cookies。".into();
    }
    if stderr_lower.contains("age") && stderr_lower.contains("restrict") {
        return "该视频有年龄限制。请在设置中导入已登录的浏览器 Cookies。".into();
    }
    if stderr_lower.contains("members") && stderr_lower.contains("only") {
        return "该视频仅限会员观看。".into();
    }
    if stderr_lower.contains("geo") && (stderr_lower.contains("restrict") || stderr_lower.contains("block")) {
        return "该视频在你所在地区不可观看。请使用代理。".into();
    }
    if stderr_lower.contains("unsupported url") {
        return format!("yt-dlp 暂不支持此链接：\n{}", url);
    }
    if stderr_lower.contains("ffmpeg") && (stderr_lower.contains("not found") || stderr_lower.contains("install")) {
        return "缺少 ffmpeg，无法合并视频与音频。请在「设置 → 依赖」点击「一键安装 ffmpeg」。".into();
    }
    if stderr_lower.contains("requested format is not available")
        || stderr_lower.contains("no formats found")
    {
        return "选择的格式不可用。尝试在首页切换到「最佳画质」或更换容器为 mkv 后重试。".into();
    }
    if stderr_lower.contains("ssl") && stderr_lower.contains("certificate") {
        return "SSL 证书验证失败。请检查系统时间是否正确。".into();
    }
    if stderr_lower.contains("name or service not known")
        || stderr_lower.contains("nodename nor servname")
        || stderr_lower.contains("dns")
    {
        return "DNS 解析失败。请检查网络连接。".into();
    }
    // Fall back to last few lines.
    let trimmed: Vec<&str> = stderr
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if trimmed.is_empty() {
        "未知错误".into()
    } else {
        trimmed.join("\n")
    }
}

/// Public wrapper for the per-site header injection (used by subscription module).
pub fn site_specific_args_public(url: &str) -> Vec<String> {
    site_specific_args(url)
}

/// Public wrapper so commands.rs can reuse the enriched PATH.
pub fn enriched_path_public() -> String {
    enriched_path()
}

/// Build PATH ensuring we can find brew/macports/python binaries even when
/// launched from Finder/Explorer.
fn enriched_path() -> String {
    let sep = if cfg!(windows) { ';' } else { ':' };
    let mut paths: Vec<String> = std::env::var("PATH")
        .ok()
        .map(|p| p.split(sep).map(String::from).collect())
        .unwrap_or_default();
    for extra in os_extra_paths() {
        let s = extra.to_string_lossy().to_string();
        if !paths.iter().any(|p| *p == s) {
            paths.push(s);
        }
    }
    paths.join(&sep.to_string())
}

pub fn find_binary(name: &str, hint: Option<&str>) -> Option<PathBuf> {
    if let Some(h) = hint {
        let p = PathBuf::from(h);
        if p.is_file() {
            return Some(p);
        }
    }
    // 1) Look for user-installed yt-dlp / ffmpeg (downloaded by our "一键安装").
    if name == "yt-dlp" {
        if let Some(user_installed) = user_installed_ytdlp() {
            if user_installed.is_file() {
                return Some(user_installed);
            }
        }
    }
    if name == "ffmpeg" {
        if let Some(p) = ffmpeg_install_target_path() {
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 2) Look in OS-specific well-known paths.
    let exe_name = exe_name_for(name);
    for extra in os_extra_paths() {
        let candidate = extra.join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // 3) PATH lookup.
    which::which(&exe_name).ok().or_else(|| which::which(name).ok())
}

fn exe_name_for(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        if name.ends_with(".exe") { name.to_string() } else { format!("{}.exe", name) }
    }
    #[cfg(not(target_os = "windows"))]
    {
        name.to_string()
    }
}

/// Path where our installer drops yt-dlp: ~/.feiniao/bin/<os-specific-name>
pub fn user_installed_ytdlp() -> Option<PathBuf> {
    Some(install_target_path()?).filter(|p| p.is_file())
}

/// Target install path for "一键安装" (regardless of whether file exists yet).
pub fn install_target_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".feiniao").join("bin").join(ytdlp_binary_filename()))
}

pub async fn check_status(
    ytdlp_hint: Option<&str>,
    ffmpeg_hint: Option<&str>,
) -> YtDlpStatus {
    let ytdlp_path = find_binary("yt-dlp", ytdlp_hint);
    let version = if let Some(ref p) = ytdlp_path {
        Command::new(p)
            .arg("--version")
            .env("PATH", enriched_path())
            .output()
            .await
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
    } else {
        None
    };

    let ffmpeg_path = find_binary("ffmpeg", ffmpeg_hint);

    YtDlpStatus {
        installed: ytdlp_path.is_some(),
        version,
        path: ytdlp_path.map(|p| p.to_string_lossy().to_string()),
        ffmpeg_installed: ffmpeg_path.is_some(),
        ffmpeg_path: ffmpeg_path.map(|p| p.to_string_lossy().to_string()),
    }
}

pub async fn probe(
    url: &str,
    ytdlp_hint: Option<&str>,
    proxy: Option<&str>,
    cookies_from_browser: Option<&str>,
) -> Result<VideoInfo> {
    let bin = find_binary("yt-dlp", ytdlp_hint)
        .ok_or_else(|| anyhow!("未找到 yt-dlp，请前往设置安装或配置路径"))?;

    // First attempt: full site-specific args (incl. --impersonate when needed).
    let first = run_probe(&bin, url, proxy, cookies_from_browser, true).await?;
    if first.0.success() {
        return parse_probe_json(&first.1);
    }

    let stderr = String::from_utf8_lossy(&first.2);
    let stderr_lower = stderr.to_ascii_lowercase();
    let is_412 = stderr.contains("412") || stderr_lower.contains("precondition");
    let needs_curl_cffi = stderr_lower.contains("impersonate target")
        && stderr_lower.contains("not available");
    let host = host_of(url).unwrap_or_default();

    // Graceful fallback: if curl_cffi missing, retry without --impersonate.
    if needs_curl_cffi {
        log::info!("curl_cffi 不可用 — 不使用 --impersonate 重试");
        let retry = run_probe(&bin, url, proxy, cookies_from_browser, false).await?;
        if retry.0.success() {
            return parse_probe_json(&retry.1);
        }
        let retry_stderr = String::from_utf8_lossy(&retry.2);
        if retry_stderr.to_ascii_lowercase().contains("410")
            || retry_stderr.to_ascii_lowercase().contains("gone")
        {
            return Err(anyhow!(
                "下载内核缺少 curl_cffi（TLS 指纹支持），无法访问该网站。\n\n解决方法（任选）：\n  1. 终端运行：pip3 install --user curl_cffi\n  2. 或者更换使用 yt-dlp 单文件版本（已支持）",
            ));
        }
        return Err(anyhow!("{}", humanize_error(&retry_stderr, url)));
    }

    // Bilibili 412 → cycle browser cookies.
    if is_412 && host.ends_with("bilibili.com") && cookies_from_browser.is_none() {
        for browser in ["chrome", "edge", "brave", "firefox", "safari"] {
            log::info!("Bilibili 412 — 重试使用 {} cookies", browser);
            let retry = run_probe(&bin, url, proxy, Some(browser), true).await?;
            if retry.0.success() {
                return parse_probe_json(&retry.1);
            }
        }
    }

    Err(anyhow!("{}", humanize_error(&stderr, url)))
}

async fn run_probe(
    bin: &PathBuf,
    url: &str,
    proxy: Option<&str>,
    cookies_from_browser: Option<&str>,
    use_impersonate: bool,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut cmd = Command::new(bin);
    cmd.arg("-J")
        .arg("--no-warnings")
        .arg("--no-call-home")
        .arg("--skip-download")
        .arg("--no-playlist")
        .arg("--retries")
        .arg("3")
        .arg("--socket-timeout")
        .arg("15");

    let raw_args = site_specific_args(url);
    let mut skip_next = false;
    for a in raw_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if !use_impersonate && a == "--impersonate" {
            skip_next = true; // skip the value too
            continue;
        }
        cmd.arg(a);
    }

    if let Some(p) = proxy {
        if !p.is_empty() {
            cmd.arg("--proxy").arg(p);
        }
    }
    if let Some(b) = cookies_from_browser {
        if !b.is_empty() {
            cmd.arg("--cookies-from-browser").arg(b);
        }
    }

    cmd.arg(url)
        .env("PATH", enriched_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await.context("调用 yt-dlp 失败")?;
    Ok((output.status, output.stdout, output.stderr))
}

fn parse_probe_json(bytes: &[u8]) -> Result<VideoInfo> {
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).context("yt-dlp 返回 JSON 解析失败")?;
    parse_video_info(&raw)
}

fn parse_video_info(v: &serde_json::Value) -> Result<VideoInfo> {
    let is_playlist = v.get("_type").and_then(|x| x.as_str()) == Some("playlist")
        || v.get("entries").is_some();

    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("(无标题)")
        .to_string();

    let formats: Vec<VideoFormat> = v
        .get("formats")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let format_id = f.get("format_id").and_then(|x| x.as_str())?.to_string();
                    let ext = f
                        .get("ext")
                        .and_then(|x| x.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let vcodec = f
                        .get("vcodec")
                        .and_then(|x| x.as_str())
                        .filter(|s| *s != "none")
                        .map(String::from);
                    let acodec = f
                        .get("acodec")
                        .and_then(|x| x.as_str())
                        .filter(|s| *s != "none")
                        .map(String::from);
                    Some(VideoFormat {
                        format_id,
                        ext,
                        resolution: f
                            .get("resolution")
                            .and_then(|x| x.as_str())
                            .map(String::from)
                            .or_else(|| {
                                let h = f.get("height").and_then(|x| x.as_u64())?;
                                let w = f.get("width").and_then(|x| x.as_u64()).unwrap_or(0);
                                Some(format!("{}x{}", w, h))
                            }),
                        fps: f.get("fps").and_then(|x| x.as_f64()),
                        has_video: vcodec.is_some(),
                        has_audio: acodec.is_some(),
                        vcodec,
                        acodec,
                        filesize: f
                            .get("filesize")
                            .and_then(|x| x.as_u64())
                            .or_else(|| f.get("filesize_approx").and_then(|x| x.as_u64())),
                        tbr: f.get("tbr").and_then(|x| x.as_f64()),
                        format_note: f
                            .get("format_note")
                            .and_then(|x| x.as_str())
                            .map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let subtitles: Vec<String> = v
        .get("subtitles")
        .and_then(|s| s.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    Ok(VideoInfo {
        id,
        title,
        description: v
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        uploader: v
            .get("uploader")
            .and_then(|x| x.as_str())
            .map(String::from),
        channel: v.get("channel").and_then(|x| x.as_str()).map(String::from),
        duration: v.get("duration").and_then(|x| x.as_f64()),
        thumbnail: v
            .get("thumbnail")
            .and_then(|x| x.as_str())
            .map(String::from),
        webpage_url: v
            .get("webpage_url")
            .and_then(|x| x.as_str())
            .map(String::from),
        view_count: v.get("view_count").and_then(|x| x.as_u64()),
        upload_date: v
            .get("upload_date")
            .and_then(|x| x.as_str())
            .map(String::from),
        extractor: v
            .get("extractor_key")
            .and_then(|x| x.as_str())
            .map(String::from),
        is_playlist,
        playlist_count: v
            .get("playlist_count")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32),
        formats,
        subtitles,
    })
}

pub struct SpawnedDownload {
    pub child: Child,
}

pub fn build_args(req: &DownloadRequest, settings: &AppSettings) -> Vec<String> {
    let mut args: Vec<String> = vec![];
    let template = req
        .filename_template
        .clone()
        .unwrap_or_else(|| settings.filename_template.clone());

    let output = format!(
        "{}/{}",
        req.output_dir.trim_end_matches('/'),
        template
    );

    // Newline-progress output for parsing
    args.push("--newline".into());
    args.push("--no-call-home".into());
    args.push("--progress".into());
    args.push("--no-warnings".into());
    args.push("--retries".into());
    args.push("5".into());
    args.push("--fragment-retries".into());
    args.push("5".into());
    args.push("--socket-timeout".into());
    args.push("20".into());
    // Speed-ups:
    // - concurrent fragments: parallelizes HLS/DASH segment downloads
    // - HTTP chunk size 10M: optimizes range-request streaming for direct files
    args.push("--concurrent-fragments".into());
    args.push("8".into());
    args.push("--http-chunk-size".into());
    args.push("10M".into());
    // If video and audio cannot be merged into the requested container (e.g.
    // VP9 + opus into mp4), fall back to mkv instead of failing the task.
    args.push("--remux-video".into());
    args.push("mp4/mkv".into());

    // Inject per-site UA/Referer/headers so anti-scrape gates do not 403/412.
    for a in site_specific_args(&req.url) {
        args.push(a);
    }

    // Format selection
    match &req.mode {
        DownloadMode::Video { quality, container } => {
            // Selector tries best video+audio first, then single best, then
            // any best matching the height limit.
            let selector = if quality == "best" {
                "bv*+ba/b/bestvideo+bestaudio/best".to_string()
            } else {
                format!(
                    "bv*[height<={q}]+ba/b[height<={q}]/bv[height<={q}]+ba/best[height<={q}]/best",
                    q = quality
                )
            };
            args.push("-f".into());
            args.push(selector);
            args.push("--merge-output-format".into());
            args.push(container.clone());
        }
        DownloadMode::Audio { format, quality } => {
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push(format.clone());
            args.push("--audio-quality".into());
            args.push(quality.clone());
        }
        DownloadMode::Custom { format_selector } => {
            args.push("-f".into());
            args.push(format_selector.clone());
        }
    }

    args.push("-o".into());
    args.push(output);

    if req.embed_metadata {
        args.push("--embed-metadata".into());
    }
    if req.embed_thumbnail {
        args.push("--embed-thumbnail".into());
    }
    if req.write_thumbnail {
        args.push("--write-thumbnail".into());
    }
    if req.subtitles.enabled {
        args.push("--write-subs".into());
        if req.subtitles.auto_generated {
            args.push("--write-auto-subs".into());
        }
        if !req.subtitles.languages.is_empty() {
            args.push("--sub-langs".into());
            args.push(req.subtitles.languages.join(","));
        }
        if req.subtitles.embed {
            args.push("--embed-subs".into());
        }
    }
    if let Some(items) = &req.playlist_items {
        if !items.is_empty() {
            args.push("--playlist-items".into());
            args.push(items.clone());
        }
    } else {
        args.push("--no-playlist".into());
    }
    if let Some(rate) = req.speed_limit.as_ref().or(settings.speed_limit.as_ref()) {
        if !rate.is_empty() {
            args.push("--limit-rate".into());
            args.push(rate.clone());
        }
    }
    if let Some(proxy) = req.proxy.as_ref().or(settings.proxy.as_ref()) {
        if !proxy.is_empty() {
            args.push("--proxy".into());
            args.push(proxy.clone());
        }
    }
    if let Some(b) = req
        .cookies_from_browser
        .as_ref()
        .or(settings.cookies_from_browser.as_ref())
    {
        if !b.is_empty() {
            args.push("--cookies-from-browser".into());
            args.push(b.clone());
        }
    }

    // Blocklist yt-dlp flags that enable arbitrary command execution or
    // file inclusion via the GUI's extra_args escape hatch.
    const BLOCKED_PREFIXES: &[&str] = &[
        "--exec",
        "--exec-before-download",
        "--exec-after-download",
        "--external-downloader",
        "--external-downloader-args",
        "--batch-file",
        "--config-location",
        "--load-info-json",
        "--parse-metadata",
    ];
    for a in &req.extra_args {
        if a.is_empty() {
            continue;
        }
        let lower = a.to_ascii_lowercase();
        if BLOCKED_PREFIXES.iter().any(|b| lower == *b || lower.starts_with(&format!("{}=", b))) {
            log::warn!("已拦截危险参数: {}", a);
            continue;
        }
        args.push(a.clone());
    }

    args.push(req.url.clone());

    args
}

pub async fn spawn_download(
    req: &DownloadRequest,
    settings: &AppSettings,
) -> Result<SpawnedDownload> {
    let bin = find_binary("yt-dlp", settings.ytdlp_path.as_deref())
        .ok_or_else(|| anyhow!("未找到 yt-dlp"))?;

    let args = build_args(req, settings);

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .env("PATH", enriched_path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    if let Some(ff) = find_binary("ffmpeg", settings.ffmpeg_path.as_deref()) {
        if let Some(dir) = ff.parent() {
            cmd.arg("--ffmpeg-location").arg(dir);
        }
    }

    let child = cmd.spawn().context("启动 yt-dlp 子进程失败")?;
    Ok(SpawnedDownload { child })
}

// Regex parsers for yt-dlp --newline progress lines.
// Example:
// [download]   3.7% of  100.00MiB at  1.23MiB/s ETA 01:23
// [download] 100% of 100.00MiB in 00:01:23
static RE_PROGRESS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\[download\]\s+(?P<pct>[\d.]+)%\s+of\s+~?\s*(?P<total>[\d.]+\s*[KMGTP]?i?B)(?:\s+at\s+(?P<speed>[\d.]+\s*[KMGTP]?i?B/s|Unknown\s*B/s))?(?:\s+ETA\s+(?P<eta>[\d:]+|Unknown))?",
    )
    .unwrap()
});

static RE_DONE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\[download\]\s+(?P<size>[\d.]+\s*[KMGTP]?i?B)\s+in\s+(?P<elapsed>[\d:]+)",
    )
    .unwrap()
});

static RE_DESTINATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[download\] Destination:\s+(?P<path>.+)").unwrap());

static RE_MERGE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\[Merger\] Merging formats into "(?P<path>.+)""#).unwrap());

static RE_AUDIO_DEST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[ExtractAudio\] Destination:\s+(?P<path>.+)").unwrap()
});

#[derive(Debug, Default, Clone)]
pub struct ParsedProgress {
    pub percent: Option<f64>,
    pub total: Option<String>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub output_path: Option<String>,
    pub postprocessing: bool,
    pub completed_size: Option<String>,
}

pub fn parse_progress_line(line: &str) -> ParsedProgress {
    let mut out = ParsedProgress::default();
    if let Some(caps) = RE_PROGRESS.captures(line) {
        out.percent = caps
            .name("pct")
            .and_then(|m| m.as_str().parse::<f64>().ok());
        out.total = caps.name("total").map(|m| m.as_str().trim().to_string());
        out.speed = caps.name("speed").map(|m| m.as_str().trim().to_string());
        out.eta = caps.name("eta").map(|m| m.as_str().trim().to_string());
        return out;
    }
    if let Some(caps) = RE_DONE.captures(line) {
        out.percent = Some(100.0);
        out.completed_size = caps.name("size").map(|m| m.as_str().trim().to_string());
        return out;
    }
    if let Some(caps) = RE_DESTINATION.captures(line) {
        out.output_path = caps.name("path").map(|m| m.as_str().trim().to_string());
        return out;
    }
    if let Some(caps) = RE_MERGE.captures(line) {
        out.output_path = caps.name("path").map(|m| m.as_str().trim().to_string());
        out.postprocessing = true;
        return out;
    }
    if let Some(caps) = RE_AUDIO_DEST.captures(line) {
        out.output_path = caps.name("path").map(|m| m.as_str().trim().to_string());
        out.postprocessing = true;
        return out;
    }
    if line.contains("[ExtractAudio]")
        || line.contains("[Merger]")
        || line.contains("[Fixup")
        || line.contains("[EmbedSubtitle]")
        || line.contains("[Metadata]")
    {
        out.postprocessing = true;
    }
    out
}

pub fn make_reader(child: &mut Child) -> (
    Option<BufReader<tokio::process::ChildStdout>>,
    Option<BufReader<tokio::process::ChildStderr>>,
) {
    let stdout = child.stdout.take().map(BufReader::new);
    let stderr = child.stderr.take().map(BufReader::new);
    (stdout, stderr)
}

pub async fn read_lines<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    mut on_line: impl FnMut(String),
) {
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let line = buf.trim_end_matches(['\n', '\r']).to_string();
                if !line.is_empty() {
                    on_line(line);
                }
            }
            Err(_) => break,
        }
    }
}
