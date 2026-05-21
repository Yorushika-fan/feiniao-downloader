import { useEffect, useState } from "react";
import { Toaster } from "sonner";
import { TitleBar } from "@/components/titlebar";
import { HomePage } from "@/pages/home";
import { SettingsPage } from "@/pages/settings";
import { Sheet } from "@/components/ui/sheet";
import { useAppStore } from "@/store/app";
import { onDownloadProgress } from "@/lib/tauri";

export function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    useAppStore.getState().initialize();
    let unlisten: (() => void) | undefined;
    onDownloadProgress((p) => useAppStore.getState().applyProgress(p)).then(
      (u) => {
        unlisten = u;
      },
    );
    return () => {
      unlisten?.();
    };
  }, []);

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground overflow-hidden">
      <TitleBar onOpenSettings={() => setSettingsOpen(true)} />
      <main className="flex-1 overflow-y-auto">
        <HomePage />
      </main>
      <Sheet
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        side="right"
        width={460}
        title="偏好设置"
      >
        <SettingsPage />
      </Sheet>
      <Toaster
        position="bottom-right"
        richColors
        closeButton
        theme="system"
      />
    </div>
  );
}
