import { AlertTriangle, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useAppStore } from "@/store/app";

export function StatusBanner() {
  const status = useAppStore((s) => s.status);
  const refresh = useAppStore((s) => s.refreshStatus);

  if (!status) {
    return (
      <div className="flex items-center gap-2 px-4 py-1.5 text-[11px] text-[hsl(var(--muted-foreground))] border-b border-[hsl(var(--border)/0.5)] shrink-0">
        <Loader2 className="w-3 h-3 animate-spin" />
        <span>正在检测 yt-dlp 环境…</span>
      </div>
    );
  }

  if (status.installed && status.ffmpeg_installed) {
    return (
      <div className="flex items-center gap-2 px-4 py-1.5 text-[11px] border-b border-[hsl(var(--border)/0.4)] shrink-0">
        <span className="relative flex h-1.5 w-1.5 shrink-0">
          <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[hsl(var(--success))] opacity-60" />
          <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-[hsl(var(--success))]" />
        </span>
        <span className="text-[hsl(var(--success))] font-medium num">
          yt-dlp {status.version}
        </span>
        <span className="text-[hsl(var(--muted-foreground)/0.5)]">·</span>
        <span className="text-[hsl(var(--muted-foreground))]">
          ffmpeg 已就绪
        </span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-[hsl(var(--warning)/0.08)] border-b border-[hsl(var(--warning)/0.25)] shrink-0">
      <AlertTriangle className="w-3.5 h-3.5 text-[hsl(var(--warning))] shrink-0" />
      <span className="flex-1 text-[11.5px] text-[hsl(var(--warning))] font-medium leading-snug">
        {!status.installed
          ? "未检测到 yt-dlp，请安装：brew install yt-dlp"
          : "未检测到 ffmpeg，建议安装：brew install ffmpeg（用于合并视频和音频）"}
      </span>
      <Button
        size="sm"
        variant="outline"
        onClick={() => refresh()}
        className="h-7 text-[11px] shrink-0"
      >
        重新检测
      </Button>
    </div>
  );
}
