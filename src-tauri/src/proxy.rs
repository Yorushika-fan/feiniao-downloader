//! Cross-platform auto-detection of HTTP/SOCKS proxies.
//! macOS: scutil --proxy
//! Windows: Internet Settings registry
//! Linux: env vars + gsettings
//! All platforms: scan common ports + read HTTP(S)_PROXY env

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyCandidate {
    pub kind: String,
    pub url: String,
    pub label: String,
    pub source: String,
}

const COMMON_HTTP: &[(u16, &str)] = &[
    (7890, "Clash / ClashX / Mihomo"),
    (7892, "Clash Verge"),
    (7893, "Clash 备用"),
    (6152, "Surge"),
    (8001, "Surge 备用"),
    (1087, "Shadowsocks-NG"),
    (10809, "V2Ray / V2RayN"),
    (10852, "sing-box"),
    (8118, "Privoxy / Polipo"),
    (8888, "Charles"),
    (8080, "通用 HTTP 代理"),
];

const COMMON_SOCKS: &[(u16, &str)] = &[
    (7891, "Clash / ClashX (SOCKS5)"),
    (6153, "Surge (SOCKS5)"),
    (1086, "Shadowsocks-NG (SOCKS5)"),
    (10808, "V2Ray (SOCKS5)"),
];

async fn port_open(port: u16) -> bool {
    matches!(
        timeout(
            Duration::from_millis(120),
            TcpStream::connect(("127.0.0.1", port))
        )
        .await,
        Ok(Ok(_))
    )
}

/// macOS: read system proxy via `scutil --proxy`.
#[cfg(target_os = "macos")]
fn system_proxy() -> Option<ProxyCandidate> {
    let out = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut http_enable = false;
    let mut http_host = String::new();
    let mut http_port = 0u16;
    let mut socks_enable = false;
    let mut socks_host = String::new();
    let mut socks_port = 0u16;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("HTTPEnable") {
            http_enable = rest.contains('1');
        } else if let Some(rest) = line.strip_prefix("HTTPProxy") {
            http_host = rest.split(':').nth(1).unwrap_or("").trim().to_string();
        } else if let Some(rest) = line.strip_prefix("HTTPPort") {
            http_port = rest
                .split(':')
                .nth(1)
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("SOCKSEnable") {
            socks_enable = rest.contains('1');
        } else if let Some(rest) = line.strip_prefix("SOCKSProxy") {
            socks_host = rest.split(':').nth(1).unwrap_or("").trim().to_string();
        } else if let Some(rest) = line.strip_prefix("SOCKSPort") {
            socks_port = rest
                .split(':')
                .nth(1)
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0);
        }
    }
    if http_enable && !http_host.is_empty() && http_port > 0 {
        return Some(ProxyCandidate {
            kind: "http".into(),
            url: format!("http://{}:{}", http_host, http_port),
            label: "系统 HTTP 代理".into(),
            source: "system".into(),
        });
    }
    if socks_enable && !socks_host.is_empty() && socks_port > 0 {
        return Some(ProxyCandidate {
            kind: "socks5".into(),
            url: format!("socks5://{}:{}", socks_host, socks_port),
            label: "系统 SOCKS 代理".into(),
            source: "system".into(),
        });
    }
    None
}

/// Windows: read Internet Settings registry.
#[cfg(target_os = "windows")]
fn system_proxy() -> Option<ProxyCandidate> {
    // Try to read via `reg query` — avoids adding a Windows-only crate dep.
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "ProxyEnable",
        ])
        .output()
        .ok()?;
    let enable_text = String::from_utf8_lossy(&out.stdout);
    if !enable_text.contains("0x1") {
        return None;
    }
    let server_out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "ProxyServer",
        ])
        .output()
        .ok()?;
    let server_text = String::from_utf8_lossy(&server_out.stdout);
    let server = server_text
        .lines()
        .find(|l| l.contains("ProxyServer"))?
        .split_whitespace()
        .last()?
        .to_string();
    Some(ProxyCandidate {
        kind: "http".into(),
        url: if server.starts_with("http") {
            server
        } else {
            format!("http://{}", server)
        },
        label: "系统代理".into(),
        source: "system".into(),
    })
}

/// Linux: read gsettings (GNOME) or env.
#[cfg(target_os = "linux")]
fn system_proxy() -> Option<ProxyCandidate> {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.system.proxy", "mode"])
        .output()
        .ok()?;
    let mode = String::from_utf8_lossy(&out.stdout);
    if !mode.contains("manual") {
        return None;
    }
    let host_out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.system.proxy.http", "host"])
        .output()
        .ok()?;
    let port_out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.system.proxy.http", "port"])
        .output()
        .ok()?;
    let host = String::from_utf8_lossy(&host_out.stdout)
        .trim()
        .trim_matches('\'')
        .to_string();
    let port: u16 = String::from_utf8_lossy(&port_out.stdout)
        .trim()
        .parse()
        .ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some(ProxyCandidate {
        kind: "http".into(),
        url: format!("http://{}:{}", host, port),
        label: "GNOME 代理".into(),
        source: "system".into(),
    })
}

fn env_proxy() -> Option<ProxyCandidate> {
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(ProxyCandidate {
                    kind: if v.starts_with("socks") {
                        "socks5".into()
                    } else {
                        "http".into()
                    },
                    url: v,
                    label: format!("环境变量 {}", key),
                    source: "env".into(),
                });
            }
        }
    }
    None
}

pub async fn detect_all() -> Vec<ProxyCandidate> {
    let mut results: Vec<ProxyCandidate> = Vec::new();

    if let Some(c) = system_proxy() {
        results.push(c);
    }

    for (port, label) in COMMON_HTTP {
        if port_open(*port).await {
            results.push(ProxyCandidate {
                kind: "http".into(),
                url: format!("http://127.0.0.1:{}", port),
                label: (*label).into(),
                source: "scan".into(),
            });
        }
    }

    for (port, label) in COMMON_SOCKS {
        if port_open(*port).await {
            results.push(ProxyCandidate {
                kind: "socks5".into(),
                url: format!("socks5://127.0.0.1:{}", port),
                label: (*label).into(),
                source: "scan".into(),
            });
        }
    }

    if let Some(c) = env_proxy() {
        if !results.iter().any(|r| r.url == c.url) {
            results.push(c);
        }
    }

    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.url.clone()));
    results
}

#[allow(dead_code)]
pub async fn detect_best() -> Option<ProxyCandidate> {
    detect_all().await.into_iter().next()
}
