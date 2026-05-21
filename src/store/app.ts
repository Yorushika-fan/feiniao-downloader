import { create } from "zustand";
import {
  api,
  type AppSettings,
  type DownloadTask,
  type HistoryItem,
  type ProgressUpdate,
  type VideoInfo,
  type YtDlpStatus,
} from "@/lib/tauri";

interface AppStore {
  status: YtDlpStatus | null;
  settings: AppSettings | null;
  videoInfo: VideoInfo | null;
  parsing: boolean;
  parseError: string | null;
  tasks: DownloadTask[];
  history: HistoryItem[];
  theme: "light" | "dark" | "system";

  initialize: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  setVideoInfo: (info: VideoInfo | null) => void;
  setParsing: (b: boolean) => void;
  setParseError: (e: string | null) => void;
  upsertTask: (task: DownloadTask) => void;
  applyProgress: (p: ProgressUpdate) => void;
  refreshTasks: () => Promise<void>;
  refreshHistory: () => Promise<void>;
  saveSettings: (s: AppSettings) => Promise<void>;
  setTheme: (t: "light" | "dark" | "system") => void;
  applyTheme: () => void;
}

const applyDom = (theme: "light" | "dark" | "system") => {
  const root = document.documentElement;
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  root.classList.toggle("dark", isDark);
};

export const useAppStore = create<AppStore>((set, get) => ({
  status: null,
  settings: null,
  videoInfo: null,
  parsing: false,
  parseError: null,
  tasks: [],
  history: [],
  theme: "system",

  async initialize() {
    const [status, settings, history, tasks] = await Promise.all([
      api.checkYtDlp().catch(() => null),
      api.getSettings().catch(() => null),
      api.getHistory().catch(() => []),
      api.listTasks().catch(() => []),
    ]);
    set({
      status,
      settings,
      history,
      tasks,
      theme: settings?.theme ?? "system",
    });
    applyDom(settings?.theme ?? "system");
    if (window.matchMedia) {
      window
        .matchMedia("(prefers-color-scheme: dark)")
        .addEventListener("change", () => {
          if (get().theme === "system") applyDom("system");
        });
    }
  },

  async refreshStatus() {
    const status = await api.checkYtDlp().catch(() => null);
    set({ status });
  },

  setVideoInfo(info) {
    set({ videoInfo: info, parseError: null });
  },
  setParsing(parsing) {
    set({ parsing });
  },
  setParseError(parseError) {
    set({ parseError });
  },

  upsertTask(task) {
    set((s) => {
      const idx = s.tasks.findIndex((t) => t.id === task.id);
      const next = [...s.tasks];
      if (idx >= 0) next[idx] = task;
      else next.unshift(task);
      return { tasks: next };
    });
  },

  applyProgress(p) {
    set((s) => {
      const idx = s.tasks.findIndex((t) => t.id === p.task_id);
      if (idx < 0) return s;
      const next = [...s.tasks];
      const cur = next[idx];
      next[idx] = {
        ...cur,
        progress: p.percent,
        speed: p.speed,
        eta: p.eta,
        size_total: p.size_total ?? cur.size_total,
        size_downloaded: p.size_downloaded ?? cur.size_downloaded,
        status: p.status,
        error: p.status === "failed" ? p.message ?? cur.error : cur.error,
        finished_at:
          p.status === "completed" ||
          p.status === "failed" ||
          p.status === "cancelled"
            ? cur.finished_at ?? Math.floor(Date.now() / 1000)
            : cur.finished_at,
      };
      return { tasks: next };
    });
    if (
      p.status === "completed" ||
      p.status === "failed" ||
      p.status === "cancelled"
    ) {
      get().refreshHistory();
    }
  },

  async refreshTasks() {
    const tasks = await api.listTasks().catch(() => []);
    set({ tasks });
  },

  async refreshHistory() {
    const history = await api.getHistory().catch(() => []);
    set({ history });
  },

  async saveSettings(s) {
    await api.saveSettings(s);
    set({ settings: s, theme: s.theme });
    applyDom(s.theme);
  },

  setTheme(theme) {
    set({ theme });
    applyDom(theme);
    const s = get().settings;
    if (s) {
      const next = { ...s, theme };
      get().saveSettings(next);
    }
  },

  applyTheme() {
    applyDom(get().theme);
  },
}));
