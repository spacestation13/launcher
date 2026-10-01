import { create } from "zustand";

import {
  type AppSettings,
  type AuthMode,
  type Favorite,
  type RenderingPipeline,
  commands,
  type Theme,
} from "../bindings";
import { setLocale } from "../i18n";
import { unwrap } from "../lib/unwrap";

interface SettingsStore {
  loaded: boolean;
  authMode: AuthMode;
  theme: Theme;
  devMode: boolean;
  notificationServers: Set<string>;
  locale: string | null;
  renderingPipeline: RenderingPipeline;
  favorites: Favorite[];
  trustedAddresses: Set<string>;
  whitelistedServers: Set<string>;
  acceptedTosServers: Set<string>;
  richPresenceEnabled: boolean;

  setAuthMode: (mode: AuthMode) => void;
  setTheme: (theme: Theme) => void;
  load: () => Promise<AppSettings | null>;
  saveAuthMode: (mode: AuthMode) => Promise<void>;
  saveTheme: (theme: Theme) => Promise<void>;
  saveLocale: (locale: string | null) => Promise<void>;
  saveRenderingPipeline: (pipeline: RenderingPipeline) => Promise<void>;
  toggleServerNotifications: (serverId: string, enabled: boolean) => Promise<void>;
  isServerNotificationsEnabled: (serverId: string) => boolean;
  toggleFavorite: (favorite: Favorite, favorited: boolean) => Promise<void>;
  isFavorited: (favorite: Favorite) => boolean;
  isServerFavorited: (serverId: string) => boolean;
  trustDirectConnectAddress: (address: string) => Promise<void>;
  isAddressTrusted: (address: string) => boolean;
  setUserWhitelisted: (uuid: string, state: boolean) => Promise<void>;
  isUserWhitelisted: (uuid: string) => boolean;
  setAcceptedTos: (uuid: string, state: boolean) => Promise<void>;
  hasAcceptedTos: (uuid: string) => boolean;
  saveRichPresence: (enabled: boolean) => Promise<void>;
}

function favoritesMatch(a: Favorite, b: Favorite): boolean {
  if (a.type !== b.type) return false;
  if (a.type === "server" && b.type === "server") return a.id === b.id;
  if (a.type === "address" && b.type === "address")
    return a.address.toLowerCase() === b.address.toLowerCase();
  return false;
}

export const useSettingsStore = create<SettingsStore>()((set, get) => ({
  loaded: false,
  authMode: "oidc",
  theme: "tgui",
  devMode: false,
  notificationServers: new Set<string>(),
  locale: null,
  renderingPipeline: "dxvk",
  favorites: [],
  trustedAddresses: new Set<string>(),
  richPresenceEnabled: true,
  whitelistedServers: new Set<string>(),
  acceptedTosServers: new Set<string>(),

  setAuthMode: (authMode) => set({ authMode }),
  setTheme: (theme) => set({ theme }),

  load: async () => {
    try {
      const [settings, devMode] = await Promise.all([
        commands.getSettings().then(unwrap),
        commands.isDevMode(),
      ]);
      set({
        loaded: true,
        authMode: settings.auth_mode,
        theme: settings.theme ?? "tgui",
        devMode,
        notificationServers: new Set(settings.notification_servers ?? []),
        locale: settings.locale ?? null,
        renderingPipeline: settings.rendering_pipeline ?? "dxvk",
        favorites: settings.favorites ?? [],
        trustedAddresses: new Set(settings.trusted_direct_connect_addresses ?? []),
        richPresenceEnabled: settings.rich_presence_enabled ?? true,
        whitelistedServers: new Set(settings.whitelisted_servers ?? []),
        acceptedTosServers: new Set(settings.accepted_tos_servers ?? []),
      });
      if (settings.locale) {
        setLocale(settings.locale);
      }
      return settings;
    } catch (err) {
      console.error("Failed to load settings:", err);
      return null;
    }
  },

  saveAuthMode: async (mode: AuthMode) => {
    unwrap(await commands.setAuthMode(mode));
    set({ authMode: mode });
  },

  saveTheme: async (theme: Theme) => {
    unwrap(await commands.setTheme(theme));
    set({ theme });
  },

  saveLocale: async (locale: string | null) => {
    unwrap(await commands.setLocale(locale));
    setLocale(locale);
    set({ locale });
  },

  saveRenderingPipeline: async (pipeline: RenderingPipeline) => {
    unwrap(await commands.setRenderingPipeline(pipeline));
    set({ renderingPipeline: pipeline });
  },

  toggleServerNotifications: async (serverId: string, enabled: boolean) => {
    const settings = unwrap(await commands.toggleServerNotifications(serverId, enabled));
    set({ notificationServers: new Set(settings.notification_servers ?? []) });
  },

  isServerNotificationsEnabled: (serverId: string) => {
    return get().notificationServers.has(serverId);
  },

  toggleFavorite: async (favorite: Favorite, favorited: boolean) => {
    const settings = unwrap(await commands.toggleFavorite(favorite, favorited));
    set({ favorites: settings.favorites ?? [] });
  },

  isFavorited: (favorite: Favorite) => {
    return get().favorites.some((f) => favoritesMatch(f, favorite));
  },

  isServerFavorited: (serverId: string) => {
    return get().favorites.some((f) => f.type === "server" && f.id === serverId);
  },

  trustDirectConnectAddress: async (address: string) => {
    const settings = unwrap(await commands.trustDirectConnectAddress(address));
    set({ trustedAddresses: new Set(settings.trusted_direct_connect_addresses ?? []) });
  },

  isAddressTrusted: (address: string) => {
    return get().trustedAddresses.has(address.toLowerCase());
  },

  setUserWhitelisted: async (uuid: string, state: boolean) => {
    const settings = unwrap(await commands.setWhitelistedServer(uuid, state));
    set({ whitelistedServers: new Set(settings.whitelisted_servers ?? []) });
  },

  isUserWhitelisted: (uuid: string) => {
    return get().whitelistedServers.has(uuid);
  },

  setAcceptedTos: async (uuid: string, state: boolean) => {
    const settings = unwrap(await commands.setAcceptedTosServer(uuid, state));
    set({ acceptedTosServers: new Set(settings.accepted_tos_servers ?? []) });
  },

  hasAcceptedTos: (uuid: string) => {
    return get().acceptedTosServers.has(uuid);
  },

  saveRichPresence: async (enabled: boolean) => {
    unwrap(await commands.setRichPresence(enabled));
    set({ richPresenceEnabled: enabled });
  },
}));
