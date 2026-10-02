import { create } from "zustand";
import { createTauriStore } from "@tauri-store/zustand";

interface UiFilters {
  tags: string[];
  show18Plus: boolean;
  showOffline: boolean | null;
  showHubStatus: boolean;
  regions: string[];
  languages: string[];
  searchQuery: string;
}

interface RecentConnection {
  serverId: string | null;
  address: string;
  serverName: string | null;
}

const MAX_RECENT_CONNECTIONS = 20;

type UiState = {
  [key: string]: unknown;
  recentConnections: RecentConnection[];
  lastViewMode: string | null;
  lastReadAnnouncement: string | null;
  ageVerified: boolean;
  filters: UiFilters;
};

interface UiStateActions {
  addRecentConnection: (connection: RecentConnection) => void;
  setLastViewMode: (mode: string) => void;
  setLastReadAnnouncement: (id: string) => void;
  setAgeVerified: () => void;
  updateFilters: (patch: Partial<UiFilters>) => void;
}

export type { UiFilters, RecentConnection };

export const useUiStateStore = create<UiState & UiStateActions>()((set, get) => ({
  recentConnections: [],
  lastViewMode: null,
  lastReadAnnouncement: null,
  ageVerified: false,
  filters: {
    tags: [],
    show18Plus: false,
    showOffline: null,
    showHubStatus: false,
    regions: [],
    languages: [],
    searchQuery: "",
  },

  addRecentConnection: (connection) => {
    const existing = get().recentConnections;
    const deduped = existing.filter(
      (c) => c.address.toLowerCase() !== connection.address.toLowerCase(),
    );
    set({
      recentConnections: [connection, ...deduped].slice(0, MAX_RECENT_CONNECTIONS),
    });
  },
  setLastViewMode: (mode) => set({ lastViewMode: mode }),
  setLastReadAnnouncement: (id) => set({ lastReadAnnouncement: id }),
  setAgeVerified: () => set({ ageVerified: true }),
  updateFilters: (patch) => set({ filters: { ...get().filters, ...patch } }),
}));

export const uiStateTauriStore = createTauriStore("ui-state", useUiStateStore, {
  autoStart: true,
  saveOnChange: true,
  saveStrategy: "debounce",
  saveInterval: 500,
});
