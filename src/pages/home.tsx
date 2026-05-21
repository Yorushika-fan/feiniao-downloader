import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import {
  Link2,
  Loader2,
  Search,
  PlayCircle,
  CheckCircle2,
  ChevronDown,
  X,
  FolderOpen,
  XCircle,
  Cog,
  Download,
  RotateCw,
  Sparkles,
  AlertTriangle,
  Wifi,
  Zap,
  Clipboard,
  Eye,
  User2,
  Clock,
  Play,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import {
  api,
  type DownloadMode,
  type DownloadRequest,
  type DownloadTask,
  type HistoryItem,
  type TaskStatus,
  type ProxyCandidate,
  type VideoInfo,
  type InstallProgress,
  onInstallProgress,
} from "@/lib/tauri";
import { useAppStore } from "@/store/app";
import { BirdMark } from "@/components/titlebar";
import {
  cn,
  formatDate,
  formatDuration,
  formatNumber,
  isValidUrl,
  truncate,
} from "@/lib/utils";

const QUALITY_OPTIONS = [
  { value: "best", label: "最佳" },
  { value: "2160", label: "4K" },
  { value: "1440", label: "2K" },
  { value: "1080", label: "1080p" },
  { value: "720", label: "720p" },
  { value: "480", label: "480p" },
];

const CONTAINER_OPTIONS = ["mp4", "mkv", "webm"];

function parseUrls(text: string): string[] {
  return text
    .split(/[\n,\s]+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0 && isValidUrl(s));
}

interface ProbedItem {
  url: string;
  state: "loading" | "ok" | "error";
  info?: VideoInfo;
  error?: string;
}

interface StreamItem {
  kind: "task" | "history";
  id: string;
  title: string;
  thumbnail: string | null;
  status: TaskStatus;
  progress: number;
  speed: string | null;
  eta: string | null;
  size_total: string | null;
  output_path: string | null;
  error: string | null;
  created_at: number;
  finished_at: number | null;
  url: string;
}

function tasksToStream(tasks: DownloadTask[]): StreamItem[] {
  return tasks.map((t) => ({
    kind: "task" as const,
    id: t.id,
    title: t.title,
    thumbnail: t.thumbnail,
    status: t.status,
    progress: t.progress,
    speed: t.speed,
    eta: t.eta,
    size_total: t.size_total,
    output_path: t.output_path,
    error: t.error,
    created_at: t.created_at,
    finished_at: t.finished_at,
    url: t.url,
  }));
}

function historyToStream(history: HistoryItem[]): StreamItem[] {
  return history.map((h) => ({
    kind: "history" as const,
    id: h.id,
    title: h.title,
    thumbnail: h.thumbnail,
    status: h.status,
    progress: h.status === "completed" ? 100 : 0,
    speed: null,
    eta: null,
    size_total: null,
    output_path: h.output_path,
    error: null,
    created_at: h.created_at,
    finished_at: h.finished_at,
    url: h.url,
  }));
}

function statusBadge(s: TaskStatus) {
  switch (s) {
    case "pending":
      return <Badge variant="secondary">等待中</Badge>;
    case "running":
      return <Badge variant="default">下载中</Badge>;
    case "postprocessing":
      return <Badge variant="warning">处理中</Badge>;
    case "completed":
      return <Badge variant="success">已完成</Badge>;
    case "failed":
      return <Badge variant="danger">失败</Badge>;
    case "cancelled":
      return <Badge variant="outline">已取消</Badge>;
  }
}

export function HomePage() {
  const status = useAppStore((s) => s.status);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const settings = useAppStore((s) => s.settings);
  const saveSettings = useAppStore((s) => s.saveSettings);
  const tasks = useAppStore((s) => s.tasks);
  const history = useAppStore((s) => s.history);
  const refreshTasks = useAppStore((s) => s.refreshTasks);
  const refreshHistory = useAppStore((s) => s.refreshHistory);

  const [urlText, setUrlText] = useState("");
  const [quality, setQuality] = useState("1080");
  const [container, setContainer] = useState("mp4");
  const [sourceTab, setSourceTab] = useState<"video" | "short">("video");
  const [submitting, setSubmitting] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installPct, setInstallPct] = useState(0);
  const [proxyHint, setProxyHint] = useState<ProxyCandidate | null>(null);
  const [proxyDismissed, setProxyDismissed] = useState(false);
  const [probed, setProbed] = useState<ProbedItem[]>([]);
  const [clipboardHint, setClipboardHint] = useState<string | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const probeAbort = useRef<AbortController | null>(null);

  useEffect(() => {
    if (settings) {
      setQuality(settings.default_quality);
      setContainer(settings.default_container);
    }
  }, [settings]);

  // auto-expand textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const lh = parseInt(getComputedStyle(el).lineHeight || "20");
    el.style.height = Math.min(el.scrollHeight, lh * 12) + "px";
  }, [urlText]);

  // proxy detection (only when none configured)
  useEffect(() => {
    if (!settings || settings.proxy || proxyDismissed) return;
    let cancelled = false;
    api
      .detectProxy()
      .then((list) => {
        if (!cancelled && list.length > 0) setProxyHint(list[0]);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [settings, proxyDismissed]);

  // install progress events
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onInstallProgress((p: InstallProgress) => setInstallPct(p.percent)).then(
      (u) => {
        unlisten = u;
      },
    );
    return () => unlisten?.();
  }, []);

  // Clipboard auto-detection — on mount + on window focus
  const checkClipboard = useCallback(async () => {
    try {
      const text = await navigator.clipboard.readText();
      const trimmed = text.trim();
      // Only show hint if: clipboard has a URL AND not already in input AND not the most recent task URL
      if (
        trimmed &&
        isValidUrl(trimmed) &&
        !urlText.includes(trimmed) &&
        probed.findIndex((p) => p.url === trimmed) === -1
      ) {
        setClipboardHint(trimmed);
      } else {
        setClipboardHint(null);
      }
    } catch {
      // Clipboard read may be denied; silently ignore
    }
  }, [urlText, probed]);

  useEffect(() => {
    checkClipboard();
    window.addEventListener("focus", checkClipboard);
    return () => window.removeEventListener("focus", checkClipboard);
  }, [checkClipboard]);

  // Debounced probe — kicks in 500ms after user stops typing
  useEffect(() => {
    const urls = parseUrls(urlText);
    if (urls.length === 0 || !status?.installed) {
      setProbed([]);
      return;
    }
    // Cancel any in-flight probe
    if (probeAbort.current) probeAbort.current.abort();
    const ctrl = new AbortController();
    probeAbort.current = ctrl;
    const timer = setTimeout(async () => {
      // Initialize loading state for new URLs
      setProbed((prev) => {
        const existing = new Map(prev.map((p) => [p.url, p]));
        return urls.map(
          (u) =>
            existing.get(u) ??
            ({ url: u, state: "loading" } as ProbedItem),
        );
      });

      for (const u of urls) {
        if (ctrl.signal.aborted) return;
        // Skip if already resolved (ok/error) — keep cached
        const cached = probed.find((p) => p.url === u);
        if (cached && cached.state !== "loading") continue;
        try {
          const info = await api.probeUrl(u);
          if (ctrl.signal.aborted) return;
          setProbed((prev) =>
            prev.map((p) =>
              p.url === u ? { url: u, state: "ok", info } : p,
            ),
          );
        } catch (e) {
          if (ctrl.signal.aborted) return;
          const msg =
            typeof e === "string"
              ? e
              : e instanceof Error
                ? e.message
                : String(e);
          setProbed((prev) =>
            prev.map((p) =>
              p.url === u ? { url: u, state: "error", error: msg } : p,
            ),
          );
        }
      }
    }, 600);
    return () => {
      clearTimeout(timer);
      ctrl.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [urlText, status?.installed]);

  const applyProxy = async () => {
    if (!proxyHint || !settings) return;
    try {
      await saveSettings({ ...settings, proxy: proxyHint.url });
      toast.success("代理已启用", { description: proxyHint.url });
      setProxyHint(null);
    } catch (e) {
      toast.error("启用代理失败", { description: String(e) });
    }
  };

  const installYtDlp = async () => {
    setInstalling(true);
    setInstallPct(0);
    try {
      await api.installYtDlp();
      toast.success("yt-dlp 已就绪");
      await refreshStatus();
    } catch (e) {
      toast.error("安装失败", { description: String(e) });
    } finally {
      setInstalling(false);
    }
  };

  const pasteFromClipboard = () => {
    if (!clipboardHint) return;
    setUrlText((prev) => (prev.trim() ? prev + "\n" + clipboardHint : clipboardHint));
    setClipboardHint(null);
  };

  const removeProbedItem = (url: string) => {
    setProbed((prev) => prev.filter((p) => p.url !== url));
    // Also remove from urlText
    setUrlText((prev) =>
      prev
        .split(/[\n,\s]+/)
        .filter((s) => s.trim() !== url)
        .join("\n"),
    );
  };

  const handleDownload = async () => {
    if (!settings) return;
    if (!status?.installed) {
      toast.error("yt-dlp 未就绪", { description: "请先点击「一键安装」" });
      return;
    }
    const okItems = probed.filter((p) => p.state === "ok" && p.info);
    if (okItems.length === 0) {
      toast.error("还没有可下载的视频", {
        description: "请先粘贴有效链接并等待解析",
      });
      return;
    }
    setSubmitting(true);
    let queued = 0;
    try {
      for (const item of okItems) {
        const info = item.info!;
        const m: DownloadMode = { kind: "video", quality, container };
        const req: DownloadRequest = {
          url: info.webpage_url ?? item.url,
          title: info.title,
          thumbnail: info.thumbnail ?? null,
          mode: m,
          output_dir: settings.output_dir,
          filename_template: settings.filename_template,
          subtitles: {
            enabled: false,
            auto_generated: false,
            embed: true,
            languages: ["zh-CN", "en"],
          },
          embed_metadata: settings.embed_metadata,
          embed_thumbnail: settings.embed_thumbnail,
          write_thumbnail: settings.write_thumbnail,
          speed_limit: settings.speed_limit,
          proxy: settings.proxy,
          cookies_from_browser: settings.cookies_from_browser,
          playlist_items: null,
          extra_args: [],
        };
        await api.startDownload(req);
        queued++;
      }
      toast.success(`已加入 ${queued} 个下载任务`);
      setUrlText("");
      setProbed([]);
      refreshTasks();
    } catch (e) {
      toast.error("开始下载失败", { description: String(e) });
    } finally {
      setSubmitting(false);
    }
  };

  // Merged stream
  const stream = useMemo(() => {
    const taskItems = tasksToStream(tasks);
    const taskIds = new Set(taskItems.map((t) => t.id));
    const historyItems = historyToStream(history).filter(
      (h) => !taskIds.has(h.id),
    );
    const all = [...taskItems, ...historyItems];
    all.sort((a, b) => {
      const aActive =
        a.status === "running" ||
        a.status === "pending" ||
        a.status === "postprocessing";
      const bActive =
        b.status === "running" ||
        b.status === "pending" ||
        b.status === "postprocessing";
      if (aActive !== bActive) return aActive ? -1 : 1;
      return (b.finished_at ?? b.created_at) - (a.finished_at ?? a.created_at);
    });
    return all;
  }, [tasks, history]);

  const ytdlpMissing = !!(status && !status.installed);
  const okCount = probed.filter((p) => p.state === "ok").length;
  const loadingCount = probed.filter((p) => p.state === "loading").length;

  return (
    <div className="px-6 py-6 max-w-[680px] mx-auto space-y-5 animate-fade-in pb-12">
      {/* yt-dlp 一键安装 卡片 */}
      {ytdlpMissing && (
        <div className="rounded-2xl border border-[hsl(var(--accent-amber)/0.35)] bg-gradient-amber-soft p-4 flex items-start gap-3 lume">
          <div className="w-9 h-9 rounded-xl bg-gradient-amber flex items-center justify-center shrink-0 shadow-[0_2px_8px_rgba(202,138,4,0.3)]">
            <Download className="w-4 h-4 text-white" strokeWidth={2.4} />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-[13.5px] font-semibold tracking-tight">
              首次使用，需要下载 yt-dlp 内核
            </div>
            <div className="text-[11.5px] text-[hsl(var(--muted-foreground))] mt-0.5">
              约 35MB，下载到 ~/.feiniao/bin/。不需要手动安装其他依赖。
            </div>
            {installing && (
              <div className="mt-2 space-y-1">
                <Progress value={installPct} />
                <div className="text-[10.5px] text-[hsl(var(--muted-foreground))] num">
                  {installPct.toFixed(0)}%
                </div>
              </div>
            )}
          </div>
          <Button
            variant="gradient"
            size="sm"
            onClick={installYtDlp}
            disabled={installing}
          >
            {installing ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Zap className="w-3.5 h-3.5" strokeWidth={2.4} />
            )}
            {installing ? "下载中…" : "一键安装"}
          </Button>
        </div>
      )}

      {/* 代理检测 */}
      {proxyHint && !ytdlpMissing && (
        <div className="rounded-2xl border border-[hsl(var(--accent-amber)/0.3)] bg-gradient-amber-soft p-3 flex items-center gap-3 lume">
          <Wifi
            className="w-4 h-4 text-[hsl(var(--accent-amber))] shrink-0"
            strokeWidth={2.2}
          />
          <div className="flex-1 min-w-0">
            <div className="text-[12px] font-medium tracking-tight">
              检测到代理：{proxyHint.label}
            </div>
            <div className="text-[10.5px] text-[hsl(var(--muted-foreground))] mt-0.5 font-mono">
              {proxyHint.url}
            </div>
          </div>
          <Button variant="gradient" size="sm" onClick={applyProxy}>
            启用
          </Button>
          <button
            onClick={() => {
              setProxyHint(null);
              setProxyDismissed(true);
            }}
            className="h-7 w-7 rounded-lg flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--muted))] transition-colors cursor-pointer"
            title="忽略"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* Hero — only when nothing else above */}
      {stream.length === 0 && probed.length === 0 && !ytdlpMissing && (
        <header className="text-center pt-4 pb-2 space-y-2">
          <div className="inline-block">
            <BirdMark size={32} />
          </div>
          <h1 className="text-[20px] font-bold tracking-tight">
            粘贴链接，开始下载
          </h1>
          <p className="text-[12px] text-[hsl(var(--muted-foreground))]">
            支持单个或多个链接，自动解析视频信息
          </p>
        </header>
      )}

      {/* 输入卡 */}
      <div
        className={cn(
          "rounded-2xl bg-[hsl(var(--card))] border border-[hsl(var(--card-border))] lume",
          "shadow-[0_1px_3px_rgba(0,0,0,0.04),0_8px_28px_-8px_rgba(0,0,0,0.08)]",
          "focus-within:shadow-[0_2px_4px_rgba(0,0,0,0.05),0_12px_36px_-8px_rgba(0,0,0,0.12)]",
          "focus-within:border-[hsl(var(--accent-amber)/0.4)]",
          "transition-all duration-200",
        )}
      >
        <div className="flex items-start gap-2 px-3.5 pt-3.5">
          <Link2
            className="w-4 h-4 mt-1 text-[hsl(var(--muted-foreground)/0.55)] shrink-0"
            strokeWidth={2}
          />
          <textarea
            ref={textareaRef}
            value={urlText}
            onChange={(e) => setUrlText(e.target.value)}
            placeholder={
              sourceTab === "short"
                ? "粘贴抖音 / 小红书 / TikTok 链接，自动去水印"
                : "粘贴链接（一行一个），自动解析视频信息"
            }
            rows={1}
            className="flex-1 bg-transparent text-[13px] resize-none outline-none border-0 placeholder:text-[hsl(var(--muted-foreground)/0.55)] min-h-[24px] py-1"
            disabled={submitting}
          />
        </div>

        {/* Clipboard hint */}
        {clipboardHint && (
          <div className="mx-3 mt-2 flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-gradient-amber-soft border border-[hsl(var(--accent-amber)/0.25)]">
            <Clipboard
              className="w-3 h-3 text-[hsl(var(--accent-amber))] shrink-0"
              strokeWidth={2.2}
            />
            <span className="text-[11px] text-[hsl(var(--foreground)/0.85)] truncate flex-1">
              检测到链接：
              <span className="font-mono text-[hsl(var(--muted-foreground))]">
                {truncate(clipboardHint, 50)}
              </span>
            </span>
            <button
              onClick={pasteFromClipboard}
              className="text-[11px] font-semibold text-[hsl(var(--accent-amber))] hover:underline cursor-pointer"
            >
              粘贴
            </button>
            <button
              onClick={() => setClipboardHint(null)}
              className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] cursor-pointer"
              title="忽略"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        )}

        {/* Footer row */}
        <div className="flex items-center gap-2 px-3 pb-3 pt-2">
          <div className="flex gap-0.5 p-0.5 bg-[hsl(var(--muted)/0.7)] rounded-lg">
            <SourceChip
              active={sourceTab === "video"}
              label="视频"
              onClick={() => setSourceTab("video")}
            />
            <SourceChip
              active={sourceTab === "short"}
              label="短视频"
              onClick={() => setSourceTab("short")}
            />
          </div>

          <InlineSelect
            value={quality}
            options={QUALITY_OPTIONS}
            onChange={setQuality}
          />

          <InlineSelect
            value={container}
            options={CONTAINER_OPTIONS.map((c) => ({
              value: c,
              label: c.toUpperCase(),
            }))}
            onChange={setContainer}
          />

          <div className="flex-1 min-w-2" />

          {okCount > 0 && (
            <Button
              variant="gradient"
              size="sm"
              onClick={handleDownload}
              disabled={submitting || ytdlpMissing}
              className="rounded-lg"
            >
              {submitting ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Sparkles className="w-3.5 h-3.5" strokeWidth={2.4} />
              )}
              下载 {okCount > 1 ? `${okCount} 个` : ""}
            </Button>
          )}
        </div>
      </div>

      {/* Probe results — preview cards */}
      {probed.length > 0 && (
        <div className="space-y-2 stagger">
          <div className="text-[10.5px] font-semibold tracking-wider text-[hsl(var(--muted-foreground)/0.7)] uppercase px-1">
            {loadingCount > 0
              ? `解析中… ${probed.length - loadingCount}/${probed.length}`
              : `已解析 · ${okCount} 个`}
          </div>
          {probed.map((item) => (
            <ProbeCard
              key={item.url}
              item={item}
              onRemove={() => removeProbedItem(item.url)}
            />
          ))}
        </div>
      )}

      {/* Status line — small */}
      {status?.installed && status.ffmpeg_installed && probed.length === 0 && (
        <div className="flex items-center gap-2 px-1 text-[10.5px] text-[hsl(var(--muted-foreground))]">
          <span className="relative flex h-1.5 w-1.5">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[hsl(var(--success))] opacity-60" />
            <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-[hsl(var(--success))]" />
          </span>
          <span className="text-[hsl(var(--success))] font-medium num">
            yt-dlp {status.version}
          </span>
          <span>· ffmpeg 已就绪</span>
        </div>
      )}

      {/* ffmpeg missing warning */}
      {status?.installed && !status.ffmpeg_installed && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--warning)/0.08)] border border-[hsl(var(--warning)/0.25)] text-[11px] text-[hsl(var(--warning))]">
          <AlertTriangle className="w-3.5 h-3.5" />
          <span className="flex-1">
            未检测到 ffmpeg，合并视频与音频需要它。运行：
            <code className="font-mono">brew install ffmpeg</code>
          </span>
        </div>
      )}

      {/* Task stream */}
      {stream.length > 0 && (
        <div className="space-y-2.5 stagger pt-2">
          <div className="flex items-center gap-2 text-[10.5px] font-semibold tracking-wider text-[hsl(var(--muted-foreground)/0.7)] uppercase px-1">
            <span>任务流</span>
            <span className="num">· {stream.length}</span>
            {history.length > 0 && (
              <button
                onClick={() => {
                  if (confirm("确定清空全部历史？")) {
                    api.clearHistory().then(() => {
                      refreshHistory();
                      toast.success("已清空");
                    });
                  }
                }}
                className="ml-auto text-[10.5px] font-medium text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--danger))] transition-colors cursor-pointer normal-case"
              >
                清空历史
              </button>
            )}
          </div>
          {stream.map((item) => (
            <StreamCard
              key={`${item.kind}-${item.id}`}
              item={item}
              onChange={refreshHistory}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SourceChip({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "h-7 px-2.5 rounded-md text-[11.5px] font-medium transition-all duration-150 cursor-pointer",
        active
          ? "bg-[hsl(var(--card))] text-[hsl(var(--foreground))] shadow-[0_1px_2px_rgba(0,0,0,0.06)]"
          : "text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]",
      )}
    >
      {label}
    </button>
  );
}

function InlineSelect({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <div className="relative">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="appearance-none h-7 px-2.5 pr-6 rounded-md bg-[hsl(var(--muted)/0.7)] hover:bg-[hsl(var(--muted))] text-[11.5px] font-medium cursor-pointer outline-none transition-colors border-0"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown
        className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3 h-3 text-[hsl(var(--muted-foreground))] pointer-events-none"
        strokeWidth={2}
      />
    </div>
  );
}

function ProbeCard({
  item,
  onRemove,
}: {
  item: ProbedItem;
  onRemove: () => void;
}) {
  if (item.state === "loading") {
    return (
      <div className="rounded-xl border border-[hsl(var(--card-border))] bg-[hsl(var(--card))] p-3 flex items-center gap-3 lume">
        <div className="w-24 h-[54px] rounded-lg shimmer shrink-0" />
        <div className="flex-1 min-w-0 space-y-1.5">
          <div className="h-3.5 w-2/3 rounded shimmer" />
          <div className="h-3 w-1/3 rounded shimmer" />
        </div>
        <Loader2 className="w-3.5 h-3.5 animate-spin text-[hsl(var(--muted-foreground))] shrink-0" />
      </div>
    );
  }

  if (item.state === "error") {
    return (
      <div className="rounded-xl border border-[hsl(var(--danger)/0.3)] bg-[hsl(var(--danger)/0.05)] p-3 flex items-start gap-3 lume">
        <XCircle
          className="w-4 h-4 text-[hsl(var(--danger))] mt-0.5 shrink-0"
          strokeWidth={2}
        />
        <div className="flex-1 min-w-0">
          <div className="text-[11.5px] font-medium font-mono text-[hsl(var(--foreground)/0.8)] truncate mb-1">
            {item.url}
          </div>
          <pre className="text-[10.5px] text-[hsl(var(--danger))] whitespace-pre-wrap font-sans leading-relaxed">
            {item.error}
          </pre>
        </div>
        <button
          onClick={onRemove}
          className="h-6 w-6 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--muted))] cursor-pointer shrink-0"
          title="移除"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    );
  }

  const info = item.info!;
  return (
    <div className="group rounded-xl border border-[hsl(var(--card-border))] bg-[hsl(var(--card))] p-3 flex gap-3 lume hover:shadow-[0_4px_18px_-4px_rgba(0,0,0,0.08)] transition-shadow">
      <div className="relative w-32 h-[72px] rounded-lg overflow-hidden bg-[hsl(var(--secondary))] shrink-0">
        {info.thumbnail ? (
          <img
            src={info.thumbnail}
            alt={info.title}
            referrerPolicy="no-referrer"
            className="w-full h-full object-cover"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <PlayCircle
              className="w-6 h-6 text-[hsl(var(--muted-foreground)/0.4)]"
              strokeWidth={1.5}
            />
          </div>
        )}
        {info.duration !== undefined && info.duration !== null && (
          <div className="absolute bottom-1 right-1 px-1.5 py-px rounded bg-black/75 backdrop-blur-sm text-white text-[9.5px] font-semibold num">
            {formatDuration(info.duration)}
          </div>
        )}
      </div>

      <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
        <h4
          className="text-[12.5px] font-semibold leading-snug line-clamp-2 tracking-tight"
          title={info.title}
        >
          {info.title}
        </h4>
        <div className="flex flex-wrap items-center gap-x-2.5 gap-y-0.5 text-[10.5px] text-[hsl(var(--muted-foreground))]">
          {info.uploader && (
            <span className="flex items-center gap-0.5">
              <User2 className="w-2.5 h-2.5" strokeWidth={2.2} />
              {info.uploader}
            </span>
          )}
          {info.view_count !== undefined && info.view_count !== null && (
            <span className="flex items-center gap-0.5 num">
              <Eye className="w-2.5 h-2.5" strokeWidth={2.2} />
              {formatNumber(info.view_count)}
            </span>
          )}
          {info.extractor && (
            <span className="text-[hsl(var(--accent-amber))] font-medium">
              {info.extractor}
            </span>
          )}
        </div>
      </div>

      <button
        onClick={onRemove}
        className="opacity-0 group-hover:opacity-100 h-6 w-6 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--danger)/0.1)] hover:text-[hsl(var(--danger))] cursor-pointer transition-opacity shrink-0 self-start"
        title="移除"
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  );
}

function StreamCard({
  item,
  onChange,
}: {
  item: StreamItem;
  onChange: () => void;
}) {
  const isActive =
    item.status === "running" ||
    item.status === "pending" ||
    item.status === "postprocessing";

  const cancel = async () => {
    try {
      await api.cancelDownload(item.id);
      toast.info("任务已取消");
    } catch (e) {
      toast.error("取消失败", { description: String(e) });
    }
  };

  const reveal = async () => {
    if (!item.output_path) return;
    try {
      await api.revealInFinder(item.output_path);
    } catch (e) {
      toast.error("无法打开文件位置", { description: String(e) });
    }
  };

  const playFile = async () => {
    if (!item.output_path) return;
    try {
      await api.openFile(item.output_path);
    } catch (e) {
      toast.error("无法打开文件", { description: String(e) });
    }
  };

  const removeHist = async () => {
    if (item.kind !== "history") return;
    try {
      await api.deleteHistoryItem(item.id);
      onChange();
    } catch (e) {
      toast.error("删除失败", { description: String(e) });
    }
  };

  const canPlay = item.status === "completed" && !!item.output_path;

  return (
    <div
      className={cn(
        "group rounded-2xl border bg-[hsl(var(--card))] p-3 flex gap-3 lume",
        "shadow-[0_1px_3px_rgba(0,0,0,0.03)]",
        "hover:shadow-[0_4px_18px_-4px_rgba(0,0,0,0.08)] hover:-translate-y-px transition-all duration-200",
        item.status === "running"
          ? "border-[hsl(var(--accent-amber)/0.3)]"
          : item.status === "completed"
            ? "border-[hsl(var(--success)/0.2)]"
            : item.status === "failed"
              ? "border-[hsl(var(--danger)/0.25)]"
              : "border-[hsl(var(--card-border))]",
      )}
    >
      {/* Clickable thumbnail — opens file when completed */}
      <button
        onClick={canPlay ? playFile : undefined}
        disabled={!canPlay}
        title={canPlay ? "点击播放" : undefined}
        className={cn(
          "relative w-28 h-[64px] rounded-lg overflow-hidden bg-[hsl(var(--secondary))] shrink-0 flex items-center justify-center group/thumb",
          canPlay && "cursor-pointer",
        )}
      >
        {item.thumbnail ? (
          <img
            src={item.thumbnail}
            alt=""
            referrerPolicy="no-referrer"
            className="w-full h-full object-cover"
          />
        ) : (
          <Download
            className="w-4 h-4 text-[hsl(var(--muted-foreground)/0.4)]"
            strokeWidth={1.5}
          />
        )}
        {canPlay && (
          <div className="absolute inset-0 bg-black/0 group-hover/thumb:bg-black/40 transition-colors flex items-center justify-center">
            <Play
              className="w-7 h-7 text-white opacity-0 group-hover/thumb:opacity-100 transition-opacity drop-shadow-[0_2px_6px_rgba(0,0,0,0.6)]"
              strokeWidth={2}
              fill="white"
            />
          </div>
        )}
      </button>

      <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
        <div className="flex items-start justify-between gap-2">
          <h4
            className="text-[12.5px] font-semibold truncate leading-snug tracking-tight"
            title={item.title}
          >
            {truncate(item.title, 50)}
          </h4>
          {statusBadge(item.status)}
        </div>

        <div className="flex items-center gap-1.5 text-[10.5px] text-[hsl(var(--muted-foreground))] min-h-[14px] num">
          {item.status === "running" && (
            <>
              <span className="font-bold text-[hsl(var(--accent-amber))]">
                {item.progress.toFixed(1)}%
              </span>
              {item.speed && <span>· {item.speed}</span>}
              {item.eta && <span>· {item.eta}</span>}
              {item.size_total && <span>· {item.size_total}</span>}
            </>
          )}
          {item.status === "postprocessing" && (
            <>
              <Cog
                className="w-3 h-3 text-[hsl(var(--warning))] animate-spin"
                strokeWidth={2}
              />
              <span className="text-[hsl(var(--warning))]">处理中…</span>
            </>
          )}
          {item.status === "completed" && (
            <>
              <CheckCircle2
                className="w-3 h-3 text-[hsl(var(--success))]"
                strokeWidth={2.5}
              />
              <span className="text-[hsl(var(--success))]">
                {item.finished_at ? formatDate(item.finished_at) : "完成"}
              </span>
            </>
          )}
          {item.status === "failed" && item.error && (
            <>
              <XCircle
                className="w-3 h-3 text-[hsl(var(--danger))]"
                strokeWidth={2}
              />
              <span
                className="truncate text-[hsl(var(--danger))]"
                title={item.error}
              >
                {item.error.split("\n")[0]}
              </span>
            </>
          )}
          {item.status === "cancelled" && <span>已取消</span>}
          {item.status === "pending" && <span>等待开始…</span>}
        </div>

        {isActive && (
          <Progress
            value={item.progress}
            indeterminate={item.status === "postprocessing"}
            variant="default"
          />
        )}
      </div>

      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
        {canPlay && (
          <button
            onClick={playFile}
            title="播放"
            className="w-7 h-7 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-gradient-amber-soft hover:text-[hsl(var(--accent-amber))] cursor-pointer"
          >
            <Play className="w-3.5 h-3.5" />
          </button>
        )}
        {isActive && (
          <button
            onClick={cancel}
            title="取消"
            className="w-7 h-7 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--danger)/0.08)] hover:text-[hsl(var(--danger))] cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
        {item.output_path && item.status === "completed" && (
          <button
            onClick={reveal}
            title="在 Finder 显示"
            className="w-7 h-7 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--secondary))] hover:text-[hsl(var(--foreground))] cursor-pointer"
          >
            <FolderOpen className="w-3.5 h-3.5" />
          </button>
        )}
        {item.status === "failed" && (
          <button
            onClick={() => toast.info("重新下载即将支持")}
            title="重新下载"
            className="w-7 h-7 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--secondary))] hover:text-[hsl(var(--foreground))] cursor-pointer"
          >
            <RotateCw className="w-3.5 h-3.5" />
          </button>
        )}
        {item.kind === "history" && !isActive && (
          <button
            onClick={removeHist}
            title="删除"
            className="w-7 h-7 rounded-md flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--danger)/0.08)] hover:text-[hsl(var(--danger))] cursor-pointer"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}
