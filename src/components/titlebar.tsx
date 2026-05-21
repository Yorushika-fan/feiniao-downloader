import { Settings } from "lucide-react";

export function TitleBar({ onOpenSettings }: { onOpenSettings?: () => void }) {
  return (
    <div className="titlebar-drag h-11 w-full flex items-center justify-between shrink-0 relative z-30">
      <div className="absolute inset-0 glass border-b border-[hsl(var(--glass-stroke))]" />

      {/* Center: brand */}
      <div className="relative z-10 flex-1 flex items-center justify-center gap-2.5 pl-20">
        <BirdMark size={18} />
        <span
          className="text-[12.5px] font-semibold tracking-tight text-[hsl(var(--foreground)/0.9)]"
          style={{ letterSpacing: "-0.015em" }}
        >
          飞鸟下载器
        </span>
      </div>

      {/* Right: settings */}
      <div className="relative z-10 pr-3 titlebar-no-drag">
        {onOpenSettings && (
          <button
            onClick={onOpenSettings}
            className="w-7 h-7 rounded-lg flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--secondary))] hover:text-[hsl(var(--foreground))] transition-colors cursor-pointer"
            title="设置"
            aria-label="打开设置"
          >
            <Settings className="w-[15px] h-[15px]" strokeWidth={1.8} />
          </button>
        )}
      </div>
    </div>
  );
}

/** Minimal SVG bird — premium amber gradient wing */
export function BirdMark({ size = 24 }: { size?: number }) {
  const id = `bird-grad-${size}`;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={id} x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#D97706" />
          <stop offset="50%" stopColor="#CA8A04" />
          <stop offset="100%" stopColor="#EAB308" />
        </linearGradient>
      </defs>
      <path
        d="M3 12 C6 7, 11 6, 15 10 C18 7, 22 8, 21 12 C19 16, 14 15, 12 12 C10 15, 5 16, 3 12Z"
        fill={`url(#${id})`}
      />
      <path
        d="M3 12 C1 13, 1 15, 3 14"
        fill="hsl(24 10% 18%)"
        opacity="0.7"
      />
    </svg>
  );
}
