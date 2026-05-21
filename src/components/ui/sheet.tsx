import { useEffect } from "react";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

interface SheetProps {
  open: boolean;
  onClose: () => void;
  side?: "right" | "left";
  width?: number;
  title?: string;
  children: React.ReactNode;
  className?: string;
}

/**
 * macOS-style slide-in panel. Used for Settings overlay, Library detail, etc.
 */
export function Sheet({
  open,
  onClose,
  side = "right",
  width = 420,
  title,
  children,
  className,
}: SheetProps) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  return (
    <>
      {/* Backdrop */}
      <div
        className={cn(
          "fixed inset-0 bg-black/20 backdrop-blur-[2px] z-40 transition-opacity duration-300",
          open ? "opacity-100" : "opacity-0 pointer-events-none",
        )}
        onClick={onClose}
        aria-hidden="true"
      />
      {/* Panel */}
      <div
        className={cn(
          "fixed top-0 bottom-0 z-50 flex flex-col",
          "bg-[hsl(var(--card))] border-[hsl(var(--card-border))]",
          "shadow-[-8px_0_32px_-8px_rgba(0,0,0,0.16)]",
          "transition-transform duration-300 ease-out",
          side === "right"
            ? "right-0 border-l"
            : "left-0 border-r",
          side === "right" && !open && "translate-x-full",
          side === "left" && !open && "-translate-x-full",
          className,
        )}
        style={{ width }}
        role="dialog"
        aria-modal="true"
      >
        {title !== undefined && (
          <header className="flex items-center justify-between gap-2 px-5 py-3.5 border-b border-[hsl(var(--border)/0.6)] shrink-0">
            <h3 className="text-[14px] font-bold tracking-tight truncate">
              {title}
            </h3>
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-lg flex items-center justify-center text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--secondary))] hover:text-[hsl(var(--foreground))] transition-colors cursor-pointer"
              aria-label="关闭"
            >
              <X className="w-4 h-4" />
            </button>
          </header>
        )}
        <div className="flex-1 overflow-y-auto">{children}</div>
      </div>
    </>
  );
}
