import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import { type AuthState, commands } from "../bindings";
import { formatCommandError } from "../lib/formatCommandError";
import { unwrap } from "../lib/unwrap";

interface AuthStore {
  authState: AuthState;
  oauthProviders: string[];
  setAuthState: (state: AuthState) => void;
  login: () => Promise<{ success: boolean; error?: string }>;
  hubLogin: (
    username: string,
    password: string,
    totpCode?: string,
  ) => Promise<{ success: boolean; error?: string; requires2fa?: boolean }>;
  hubOAuthLogin: (
    provider: string,
  ) => Promise<{ success: boolean; error?: string; requires2fa?: boolean }>;
  hubSteamLogin: (acceptedTos?: boolean, preferredUsername?: string) => Promise<{ success: boolean; error?: string; requires2fa?: boolean; requiresTos?: boolean; suggestedUsername?: string }>;
  hubComplete2fa: (totpCode: string) => Promise<{ success: boolean; error?: string }>;
  logout: () => Promise<void>;
  initListener: () => Promise<() => void>;
  loadOauthProviders: () => Promise<void>;
}

const initialAuthState: AuthState = {
  logged_in: false,
  user: null,
  loading: true,
  error: null,
};

export const useAuthStore = create<AuthStore>()((set, get) => ({
  authState: initialAuthState,
  oauthProviders: [],

  setAuthState: (authState) => set({ authState }),

  loadOauthProviders: async () => {
    try {
      const providers = unwrap(await commands.getHubOauthProviders());
      set({ oauthProviders: providers.filter((p) => p !== "steam") });
    } catch {
      // Non-fatal - leave providers empty
    }
  },

  login: async () => {
    try {
      const state = unwrap(await commands.startLogin());
      set({ authState: state });
      return { success: state.logged_in };
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      return { success: false, error };
    }
  },

  hubLogin: async (username, password, totpCode?) => {
    const r = await commands.hubLogin(username, password, totpCode || null);
    if (r.status === "ok") {
      set({ authState: r.data });
      return { success: r.data.logged_in };
    }
    if (r.error.type === "requires_2fa") {
      return { success: false, requires2fa: true };
    }
    return { success: false, error: formatCommandError(r.error) };
  },

  hubOAuthLogin: async (provider) => {
    const r = await commands.hubOauthLogin(provider);
    if (r.status === "ok") {
      set({ authState: r.data });
      return { success: r.data.logged_in };
    }
    if (r.error.type === "requires_2fa") {
      return { success: false, requires2fa: true };
    }
    return { success: false, error: formatCommandError(r.error) };
  },

  hubSteamLogin: async (acceptedTos = false, preferredUsername?: string) => {
    const r = await commands.hubSteamLogin(acceptedTos, preferredUsername ?? null);
    if (r.status === "ok") {
      set({ authState: r.data });
      return { success: r.data.logged_in };
    }
    if (r.error.type === "requires_2fa") {
      return { success: false, requires2fa: true };
    }
    if (r.error.type === "requires_tos") {
      return { success: false, requiresTos: true, suggestedUsername: r.error.data.suggested_username ?? undefined };
    }
    return { success: false, error: formatCommandError(r.error) };
  },

  hubComplete2fa: async (totpCode) => {
    const r = await commands.hubComplete2fa(totpCode);
    if (r.status === "ok") {
      set({ authState: r.data });
      return { success: r.data.logged_in };
    }
    return { success: false, error: formatCommandError(r.error) };
  },

  logout: async () => {
    try {
      const state = unwrap(await commands.logout());
      set({ authState: state });
    } catch (err) {
      console.error("Logout failed:", err);
    }
  },

  initListener: async () => {
    const unlisten = await listen<AuthState>("auth-state-changed", (event) => {
      get().setAuthState(event.payload);
    });

    try {
      const cached = unwrap(await commands.getCurrentAuthState());
      if (cached) {
        get().setAuthState(cached);
      }
    } catch {
    }

    return unlisten;
  },
}));
