import { create } from "zustand";

import { commands } from "../bindings";
import type { SteamAuthResult, SteamUserInfo } from "../bindings";
import { unwrap } from "../lib/unwrap";

interface SteamStore {
  available: boolean;
  user: SteamUserInfo | null;
  accessToken: string | null;

  setAccessToken: (token: string | null) => void;
  initialize: () => Promise<boolean>;
  authenticate: (createAccountIfMissing: boolean, acceptedTos?: boolean) => Promise<SteamAuthResult | null>;
  logout: () => void;
  cancelAuthTicket: () => Promise<void>;
}

export const useSteamStore = create<SteamStore>()((set) => ({
  available: false,
  user: null,
  accessToken: null,

  setAccessToken: (accessToken) => set({ accessToken }),

  initialize: async () => {
    try {
      const user = unwrap(await commands.getSteamUserInfo());
      set({ available: true, user });
      return true;
    } catch {
      set({ available: false });
      return false;
    }
  },

  authenticate: async (createAccountIfMissing: boolean, acceptedTos = false) => {
    try {
      const result = unwrap(await commands.steamAuthenticate(createAccountIfMissing, acceptedTos));

      if (result.success && result.access_token) {
        set({ accessToken: result.access_token });
      }

      return result;
    } catch (err) {
      return {
        success: false,
        user_exists: false,
        access_token: null,
        requires_linking: false,
        linking_url: null,
        error: err instanceof Error ? err.message : String(err),
        requires_tos: false,
      };
    }
  },

  logout: () => {
    set({ accessToken: null });
  },

  cancelAuthTicket: async () => {
    try {
      await commands.cancelSteamAuthTicket();
    } catch {
      // Ignore errors when canceling
    }
  },
}));
