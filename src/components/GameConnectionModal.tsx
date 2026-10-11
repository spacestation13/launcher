import { listen } from "@tauri-apps/api/event";
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../bindings";

export type GameConnectionState = "idle" | "connecting" | "connected" | "restarting";

interface RelayTransfer {
  received_bytes: number;
  bytes_per_sec: number;
}

const DOWNLOADING_THRESHOLD_BYTES = 1024 * 1024;

const formatBytes = (bytes: number) =>
  bytes >= 1024 * 1024
    ? `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    : `${Math.round(bytes / 1024)} KB`;

interface GameConnectionModalProps {
  visible: boolean;
  state: GameConnectionState;
  serverName: string | null;
  restartReason?: string | null;
  onClose: () => void;
}

export const GameConnectionModal = ({
  visible,
  state,
  serverName,
  restartReason,
  onClose,
}: GameConnectionModalProps) => {
  const { t } = useTranslation();
  const [closing, setClosing] = useState(false);
  const [transfer, setTransfer] = useState<RelayTransfer | null>(null);

  const showSpinner = state === "connecting" || state === "restarting";

  useEffect(() => {
    setTransfer(null);
    if (!showSpinner) return;
    const unlisten = listen<RelayTransfer>("relay-transfer", (event) => {
      setTransfer(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [showSpinner]);

  if (!visible) return null;

  const handleCloseGame = async () => {
    setClosing(true);
    try {
      await commands.killGame();
      onClose();
    } catch (err) {
      console.error("Failed to close game:", err);
    } finally {
      setClosing(false);
    }
  };

  const download =
    showSpinner && transfer && transfer.received_bytes >= DOWNLOADING_THRESHOLD_BYTES
      ? transfer
      : null;

  const getStatusText = () => {
    if (download) {
      return t("connection.downloading", { server: serverName });
    }
    switch (state) {
      case "restarting":
        return t("connection.restarting", { server: serverName });
      case "connecting":
        return t("connection.connecting", { server: serverName });
      default:
        return t("connection.connected", { server: serverName });
    }
  };

  return (
    <div className="game-connection-overlay">
      <div className="game-connection-modal">
        <div className="game-connection-status">
          {showSpinner && <div className="game-connection-spinner" />}
          <h2>{getStatusText()}</h2>
          {state === "restarting" && restartReason && (
            <p className="game-connection-reason">{restartReason}</p>
          )}
          {download && (
            <p className="game-connection-transfer">
              {t("connection.transfer", {
                speed: formatBytes(download.bytes_per_sec),
                received: formatBytes(download.received_bytes),
              })}
            </p>
          )}
        </div>
        <button type="button" className="button" onClick={handleCloseGame} disabled={closing}>
          {closing ? t("connection.closing") : t("connection.closeGame")}
        </button>
      </div>
    </div>
  );
};
