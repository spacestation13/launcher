import { useCallback, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import type { SteamAuthModalState } from "../components/SteamAuthModal";
import { useSteamStore } from "../stores";
import { useRawConnect } from "./useConnect";
import { useError } from "./useError";

export interface SteamModalView {
  visible: boolean;
  state: SteamAuthModalState;
  error?: string;
  linkingUrl?: string;
  suggestedUsername?: string;
}

const CLOSED: SteamModalView = {
  visible: false,
  state: "idle",
  error: undefined,
  linkingUrl: undefined,
};

export function useSteamLinking() {
  const { showError } = useError();
  const { connect } = useRawConnect();
  const { authenticateSteam, steamLogout, cancelSteamAuthTicket } = useSteamStore(
    useShallow((s) => ({
      authenticateSteam: s.authenticate,
      steamLogout: s.logout,
      cancelSteamAuthTicket: s.cancelAuthTicket,
    })),
  );

  const [steamModal, setSteamModal] = useState<SteamModalView>(CLOSED);
  const [pendingServerId, setPendingServerId] = useState<string | null>(null);

  const handleSteamAuthenticate = useCallback(
    async (createAccountIfMissing: boolean, acceptedTos = false, preferredUsername?: string) => {
      setSteamModal((prev) => ({
        ...prev,
        state: "loading",
        error: undefined,
        linkingUrl: undefined,
      }));

      const result = await authenticateSteam(createAccountIfMissing, acceptedTos, preferredUsername);

      if (result?.success && result.access_token) {
        setSteamModal(CLOSED);

        if (pendingServerId) {
          const serverToConnect = pendingServerId;
          setPendingServerId(null);
          connect(serverToConnect, "SteamAuthModal.afterAuth").catch((err) => {
            showError(err instanceof Error ? err.message : String(err));
          });
        }

        return result;
      }
      if (result?.requires_linking) {
        setSteamModal({
          visible: true,
          state: "linking",
          error: undefined,
          linkingUrl: result.linking_url || undefined,
        });
        return result;
      }
      if (result?.requires_tos) {
        setSteamModal({
          visible: true,
          state: "tos",
          error: undefined,
          linkingUrl: undefined,
          suggestedUsername: result.suggested_username ?? undefined,
        });
        return result;
      }
      if (acceptedTos && result?.error) {
        setSteamModal((prev) => ({
          visible: true,
          state: "tos",
          error: result.error ?? undefined,
          linkingUrl: undefined,
          suggestedUsername: prev.suggestedUsername,
        }));
      } else {
        setSteamModal({
          visible: true,
          state: "error",
          error: result?.error || "Authentication failed",
          linkingUrl: undefined,
        });
      }
      return result;
    },
    [authenticateSteam, connect, pendingServerId, showError],
  );

  const handleSteamModalClose = useCallback(async () => {
    setSteamModal(CLOSED);
    await cancelSteamAuthTicket();
  }, [cancelSteamAuthTicket]);

  const handleSteamLogout = useCallback(() => {
    steamLogout();
  }, [steamLogout]);

  const onSteamAuthRequired = useCallback(
    (serverId?: string) => {
      if (serverId) setPendingServerId(serverId);
      setSteamModal({ visible: true, state: "idle", error: undefined, linkingUrl: undefined });
      handleSteamAuthenticate(false);
    },
    [handleSteamAuthenticate],
  );

  const onAutoConnectLinkingRequired = useCallback((linkingUrl: string | null) => {
    setSteamModal({
      visible: true,
      state: "linking",
      error: undefined,
      linkingUrl: linkingUrl || undefined,
    });
  }, []);

  return {
    steamModal,
    handleSteamAuthenticate,
    handleSteamModalClose,
    handleSteamLogout,
    onSteamAuthRequired,
    onAutoConnectLinkingRequired,
  };
}
