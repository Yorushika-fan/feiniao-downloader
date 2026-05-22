import { useEffect, useState } from "react";
import {
  Save,
  Globe2,
  Cookie,
  Zap,
  FolderOpen,
  CheckCircle2,
  AlertCircle,
  Loader2,
  ShieldCheck,
  RefreshCw,
  Sparkles,
  Download,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useAppStore } from "@/store/app";
import { api, type AppSettings, type UpdateInfo } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const settings = useAppStore((s) => s.settings);
  const saveSettings = useAppStore((s) => s.saveSettings);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const [draft, setDraft] = useState<AppSettings | null>(settings);
  const [saving, setSaving] = useState(false);
  const [cookieTest, setCookieTest] = useState<{
    state: "idle" | "loading" | "ok" | "err";
    msg?: string;
  }>({ state: "idle" });
  const [updateState, setUpdateState] = useState<{
    state: "idle" | "checking" | "found" | "uptodate" | "err" | "installing";
    info?: UpdateInfo;
    msg?: string;
  }>({ state: "idle" });

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  if (!draft) {
    return (
      <div className="h-full flex items-center justify-center text-[12px] text-[hsl(var(--muted-foreground))]">
        加载中…
      </div>
    );
  }

  const update = <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => {
    setDraft((d) => (d ? { ...d, [k]: v } : d));
  };

  const pickDir = async () => {
    try {
      const path = await api.pickDirectory();
      if (path) update("output_dir", path);
    } catch (e) {
      toast.error("选择目录失败", { description: String(e) });
    }
  };

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      await saveSettings(draft);
      await refreshStatus();
      toast.success("设置已保存");
    } catch (e) {
      toast.error("保存失败", { description: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const checkUpdate = async () => {
    setUpdateState({ state: "checking" });
    try {
      const info = await api.checkUpdate();
      if (info.has_update) {
        setUpdateState({ state: "found", info });
      } else {
        setUpdateState({
          state: "uptodate",
          info,
          msg: `已是最新版本（${info.current_version}）`,
        });
      }
    } catch (e) {
      setUpdateState({
        state: "err",
        msg: typeof e === "string" ? e : String(e),
      });
    }
  };

  const runUpdate = async () => {
    const info = updateState.info;
    if (!info) return;
    if (!info.asset_url) {
      if (info.release_url) {
        try {
          await api.openExternal(info.release_url);
        } catch (e) {
          toast.error("打开浏览器失败", { description: String(e) });
        }
      }
      return;
    }
    setUpdateState({ state: "installing", info });
    try {
      await api.installUpdate(info.asset_url);
      toast.success("已下载完成，正在打开安装包…", {
        description: "请按提示完成安装后重启应用",
      });
    } catch (e) {
      toast.error("更新失败", { description: String(e) });
      setUpdateState({ state: "found", info });
    }
  };

  const runCookieTest = async () => {
    if (!draft.cookies_from_browser) {
      setCookieTest({ state: "err", msg: "请先选择浏览器" });
      return;
    }
    setCookieTest({ state: "loading" });
    try {
      const n = await api.testCookies(draft.cookies_from_browser);
      setCookieTest({
        state: "ok",
        msg: `从 ${draft.cookies_from_browser} 读取到 ${n} 条 Cookie`,
      });
    } catch (e) {
      setCookieTest({
        state: "err",
        msg: typeof e === "string" ? e : String(e),
      });
    }
  };

  return (
    <div className="p-5 space-y-5">
      <Section icon={FolderOpen} title="下载位置">
        <div className="flex gap-2">
          <Input
            value={draft.output_dir}
            onChange={(e) => update("output_dir", e.target.value)}
            className="flex-1 font-mono text-[11.5px]"
          />
          <Button variant="outline" onClick={pickDir} size="sm">
            <FolderOpen className="w-3.5 h-3.5" />
            选择
          </Button>
        </div>
        <Field label="文件名模板" hint="留默认即可">
          <Input
            value={draft.filename_template}
            onChange={(e) => update("filename_template", e.target.value)}
            placeholder="%(title)s.%(ext)s"
            className="font-mono text-[11.5px]"
          />
        </Field>
      </Section>

      <Section icon={Zap} title="默认偏好">
        <div className="grid grid-cols-2 gap-3">
          <Field label="默认画质">
            <Select
              value={draft.default_quality}
              onChange={(e) => update("default_quality", e.target.value)}
            >
              <option value="best">最佳画质</option>
              <option value="2160">4K · 2160p</option>
              <option value="1440">2K · 1440p</option>
              <option value="1080">1080p</option>
              <option value="720">720p</option>
              <option value="480">480p</option>
            </Select>
          </Field>
          <Field label="默认容器">
            <Select
              value={draft.default_container}
              onChange={(e) => update("default_container", e.target.value)}
            >
              <option value="mp4">MP4</option>
              <option value="mkv">MKV</option>
              <option value="webm">WebM</option>
            </Select>
          </Field>
          <Field label="并发数">
            <Input
              type="number"
              min={1}
              max={8}
              value={draft.max_concurrent}
              onChange={(e) =>
                update(
                  "max_concurrent",
                  Math.max(1, parseInt(e.target.value || "1")),
                )
              }
              className="num"
            />
          </Field>
          <Field label="速度限制" hint="2M / 留空">
            <Input
              value={draft.speed_limit ?? ""}
              onChange={(e) => update("speed_limit", e.target.value || null)}
              placeholder="留空"
              className="num"
            />
          </Field>
        </div>
        <ToggleRow
          title="嵌入元数据"
          desc="标题/作者写入文件标签"
          checked={draft.embed_metadata}
          onChange={(b) => update("embed_metadata", b)}
        />
        <ToggleRow
          title="嵌入封面"
          desc="封面图嵌入文件"
          checked={draft.embed_thumbnail}
          onChange={(b) => update("embed_thumbnail", b)}
        />
        <ToggleRow
          title="保存封面图"
          desc="额外保存 .jpg"
          checked={draft.write_thumbnail}
          onChange={(b) => update("write_thumbnail", b)}
        />
      </Section>

      <Section icon={Globe2} title="网络">
        <Field
          label={
            <span className="flex items-center gap-1.5">
              <Globe2 className="w-3 h-3" strokeWidth={2.2} /> 代理服务器
            </span>
          }
          hint="例如 http://127.0.0.1:7890 / socks5://..."
        >
          <Input
            value={draft.proxy ?? ""}
            onChange={(e) => update("proxy", e.target.value || null)}
            placeholder="http://127.0.0.1:7890"
            className="font-mono text-[11.5px]"
          />
        </Field>
        <Field
          label={
            <span className="flex items-center gap-1.5">
              <Cookie className="w-3 h-3" strokeWidth={2.2} /> 浏览器 Cookies
            </span>
          }
          hint="登录账号才能下载时使用"
        >
          <div className="flex gap-2">
            <Select
              value={draft.cookies_from_browser ?? ""}
              onChange={(e) => {
                update("cookies_from_browser", e.target.value || null);
                setCookieTest({ state: "idle" });
              }}
              className="flex-1"
            >
              <option value="">不使用</option>
              <option value="safari">Safari</option>
              <option value="chrome">Chrome</option>
              <option value="edge">Edge</option>
              <option value="firefox">Firefox</option>
              <option value="brave">Brave</option>
              <option value="chromium">Chromium</option>
              <option value="opera">Opera</option>
              <option value="vivaldi">Vivaldi</option>
            </Select>
            <Button
              variant="outline"
              size="sm"
              onClick={runCookieTest}
              disabled={
                cookieTest.state === "loading" ||
                !draft.cookies_from_browser
              }
            >
              {cookieTest.state === "loading" ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <ShieldCheck className="w-3.5 h-3.5" />
              )}
              测试
            </Button>
          </div>
          {cookieTest.state === "ok" && (
            <div className="flex items-center gap-1.5 text-[11px] text-[hsl(var(--success))] mt-1.5">
              <CheckCircle2 className="w-3 h-3" strokeWidth={2.5} />
              <span>{cookieTest.msg}</span>
            </div>
          )}
          {cookieTest.state === "err" && (
            <div className="flex items-start gap-1.5 text-[11px] text-[hsl(var(--danger))] mt-1.5 leading-relaxed">
              <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
              <span>{cookieTest.msg}</span>
            </div>
          )}
        </Field>
      </Section>

      <Section icon={Sparkles} title="版本与更新">
        <div className="flex items-center justify-between gap-3 rounded-lg bg-[hsl(var(--secondary)/0.5)] px-3 py-2.5">
          <div className="min-w-0">
            <div className="text-[12px] font-medium">
              当前版本{" "}
              <span className="num text-[hsl(var(--muted-foreground))]">
                v{updateState.info?.current_version ?? "1.3.3"}
              </span>
            </div>
            {updateState.state === "uptodate" && (
              <div className="flex items-center gap-1.5 text-[11px] text-[hsl(var(--success))] mt-1">
                <CheckCircle2 className="w-3 h-3" strokeWidth={2.5} />
                <span>{updateState.msg}</span>
              </div>
            )}
            {updateState.state === "found" && updateState.info && (
              <div className="flex items-center gap-1.5 text-[11px] text-[hsl(var(--accent-amber))] font-medium mt-1">
                <Sparkles className="w-3 h-3" strokeWidth={2.4} />
                <span>
                  发现新版本 {updateState.info.latest_version}
                </span>
              </div>
            )}
            {updateState.state === "err" && (
              <div className="flex items-start gap-1.5 text-[11px] text-[hsl(var(--danger))] mt-1">
                <AlertCircle className="w-3 h-3 mt-0.5 shrink-0" />
                <span>{updateState.msg}</span>
              </div>
            )}
            {updateState.state === "idle" && (
              <div className="text-[11px] text-[hsl(var(--muted-foreground))] mt-1">
                每次打开应用会自动检查更新
              </div>
            )}
          </div>
          {updateState.state === "found" ? (
            <Button
              variant="gradient"
              size="sm"
              onClick={runUpdate}
              disabled={updateState.state !== "found"}
            >
              <Download className="w-3.5 h-3.5" strokeWidth={2.4} />
              立即更新
            </Button>
          ) : updateState.state === "installing" ? (
            <Button variant="gradient" size="sm" disabled>
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              下载中…
            </Button>
          ) : (
            <Button
              variant="outline"
              size="sm"
              onClick={checkUpdate}
              disabled={updateState.state === "checking"}
            >
              {updateState.state === "checking" ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <RefreshCw className="w-3.5 h-3.5" strokeWidth={2.2} />
              )}
              {updateState.state === "checking" ? "检查中…" : "检查更新"}
            </Button>
          )}
        </div>
      </Section>

      <Section icon={RefreshCw} title="主题">
        <div className="grid grid-cols-3 gap-2">
          {(["light", "system", "dark"] as const).map((t) => (
            <button
              key={t}
              onClick={() => update("theme", t)}
              className={cn(
                "h-9 rounded-lg text-[12px] font-medium cursor-pointer border transition-all",
                draft.theme === t
                  ? "bg-gradient-amber-soft text-[hsl(var(--accent-amber))] border-[hsl(var(--accent-amber)/0.3)]"
                  : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--secondary))]",
              )}
            >
              {t === "light" ? "亮色" : t === "dark" ? "暗色" : "跟随系统"}
            </button>
          ))}
        </div>
      </Section>

      <div className="sticky bottom-0 -mx-5 -mb-5 px-5 py-3 bg-[hsl(var(--card)/0.92)] backdrop-blur border-t border-[hsl(var(--border))] flex justify-end">
        <Button variant="gradient" onClick={save} disabled={saving} size="sm">
          {saving ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <Save className="w-3.5 h-3.5" strokeWidth={2.4} />
          )}
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
  );
}

function Section({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof FolderOpen;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div className="flex items-center gap-2">
        <Icon
          className="w-3.5 h-3.5 text-[hsl(var(--accent-amber))]"
          strokeWidth={2.2}
        />
        <h3 className="text-[12.5px] font-bold tracking-tight">{title}</h3>
      </div>
      <div className="space-y-2.5">{children}</div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: React.ReactNode;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block space-y-1.5">
      <div className="text-[11px] font-medium text-[hsl(var(--foreground)/0.8)]">
        {label}
      </div>
      {children}
      {hint && (
        <div className="text-[10.5px] text-[hsl(var(--muted-foreground)/0.85)] leading-relaxed">
          {hint}
        </div>
      )}
    </label>
  );
}

function ToggleRow({
  title,
  desc,
  checked,
  onChange,
}: {
  title: string;
  desc: string;
  checked: boolean;
  onChange: (b: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-1.5 border-t border-[hsl(var(--border)/0.4)] first:border-t-0 first:pt-0">
      <div className="flex-1 min-w-0">
        <div className="text-[12.5px] font-medium">{title}</div>
        <div className="text-[10.5px] text-[hsl(var(--muted-foreground))]">
          {desc}
        </div>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  );
}
