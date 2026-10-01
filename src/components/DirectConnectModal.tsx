import { faCircleCheck, faUsers } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, DirectConnectInfo } from "../bindings";
import { useConnect, useError } from "../hooks";
import { formatCommandError } from "../lib/formatCommandError";
import { useSettingsStore } from "../stores";
import { useUiStateStore } from "../stores/uiStateStore";
import { Modal } from "./Modal";

function TrustInfo({ connectInfo }: { connectInfo: DirectConnectInfo }) {
  const { t } = useTranslation();

  if (connectInfo.trust === "DomainAttested" && connectInfo.verified_domain) {
    return (
      <>
        <p className="settings-description">
          {t("directConnect.domainAttestedInfo")}{" "}
          <span className="badge badge-verified">
            <FontAwesomeIcon icon={faCircleCheck} /> {connectInfo.verified_domain}
          </span>
        </p>
        <p className="settings-description settings-description-hint">
          {t("directConnect.domainAttestedDetail")}
        </p>
      </>
    );
  }

  if (connectInfo.trust === "Unreachable") {
    return (
      <>
        <p className="settings-description">{t("directConnect.unreachableWarning")}</p>
        <p className="settings-description settings-description-hint">
          {t("directConnect.unreachableDetail")}
        </p>
      </>
    );
  }

  if (connectInfo.trust === "SelfReported") {
    return (
      <>
        <p className="settings-description">{t("directConnect.selfReportedWarning")}</p>
        <p className="settings-description settings-description-hint">
          {t("directConnect.selfReportedDetail")}
        </p>
      </>
    );
  }

  if (connectInfo.trust === "ByondOnly") {
    return <p className="settings-description">{t("directConnect.byondOnlyInfo")}</p>;
  }

  return null;
}

function ServerPreview({ connectInfo }: { connectInfo: DirectConnectInfo }) {
  const { t } = useTranslation();

  const hasName = !!connectInfo.server_name;
  const isSelfReported =
    connectInfo.trust === "SelfReported" ||
    connectInfo.trust === "ByondOnly" ||
    connectInfo.trust === "DomainAttested";
  const isHubTrusted =
    connectInfo.trust === "HubVerified" || connectInfo.trust === "HubKnown";

  return (
    <div className="server-item">
      <div className="server-item-row">
        <div className="server-info">
          <div className="server-name">
            {hasName ? connectInfo.server_name : `${connectInfo.hostname}:${connectInfo.port}`}
            {connectInfo.verified_domain && isHubTrusted && (
              <span className="badge badge-verified">
                <FontAwesomeIcon icon={faCircleCheck} /> {connectInfo.verified_domain}
              </span>
            )}
            {hasName && isSelfReported && !connectInfo.verified_domain && (
              <span className="badge badge-tag">
                {t("directConnect.selfReportedLabel")}
              </span>
            )}
            {connectInfo.tags?.map((tag) => (
              <span key={tag} className="badge badge-tag">
                {tag}
              </span>
            ))}
          </div>
          {(connectInfo.map_name || connectInfo.server_description) && (
            <div className="server-details">
              <div className="detail-line">
                {[connectInfo.map_name, connectInfo.server_region].filter(Boolean).map((part, i) => (
                  <span key={String(part)}>
                    {i > 0 && " · "}
                    {part}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
        {connectInfo.players != null && (
          <div className="server-stats">
            <span className="player-count">
              <FontAwesomeIcon icon={faUsers} />{" "}
              {connectInfo.player_cap
                ? t("directConnect.playersCountWithCap", {
                    count: connectInfo.players,
                    cap: connectInfo.player_cap,
                  })
                : t("directConnect.playersCount", { count: connectInfo.players })}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

function shouldSkipConfirmation(_info: DirectConnectInfo, address: string): boolean {
  if (useSettingsStore.getState().isAddressTrusted(address)) return true;
  return false;
}

interface DirectConnectModalProps {
  visible: boolean;
  onClose: () => void;
}

export const DirectConnectModal = ({ visible, onClose }: DirectConnectModalProps) => {
  const { t } = useTranslation();
  const [address, setAddress] = useState("");
  const [resolving, setResolving] = useState(false);
  const [connectInfo, setConnectInfo] = useState<DirectConnectInfo | null>(null);
  const [trustAddress, setTrustAddress] = useState(false);
  const { showError } = useError();
  const { connectToAddress } = useConnect();

  const handleResolve = async () => {
    const trimmed = address.trim();
    if (!trimmed) return;

    setResolving(true);
    try {
      const result = await commands.resolveDirectConnect(trimmed);
      if (result.status === "error") {
        showError(formatCommandError(result.error));
        return;
      }
      const info = result.data;
      if (shouldSkipConfirmation(info, trimmed)) {
        await doConnect(trimmed, info.server_id ?? undefined, info.server_name ?? undefined);
      } else {
        setConnectInfo(info);
      }
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      setResolving(false);
    }
  };

  const doConnect = async (addr: string, serverId?: string, serverName?: string) => {
    handleClose();
    const success = await connectToAddress(addr, "DirectConnect", serverId);
    if (success) {
      useUiStateStore.getState().addRecentConnection({
        serverId: serverId ?? null,
        address: addr,
        serverName: serverName ?? null,
      });
    }
  };

  const handleConfirm = async () => {
    if (trustAddress) {
      await useSettingsStore.getState().trustDirectConnectAddress(address.trim());
    }
    await doConnect(address.trim(), connectInfo?.server_id ?? undefined, connectInfo?.server_name ?? undefined);
  };

  const handleClose = () => {
    setConnectInfo(null);
    setTrustAddress(false);
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !resolving && !connectInfo) {
      handleResolve();
    }
  };

  if (connectInfo) {
    return (
      <Modal
        visible={visible}
        onClose={handleClose}
        className="settings-modal"
        closeOnOverlayClick
        title={t("directConnect.title")}
      >
        <div className="modal-body">
          <div className="settings-section">
            <ServerPreview connectInfo={connectInfo} />
            <TrustInfo connectInfo={connectInfo} />
          </div>
        </div>
        <div className="modal-footer" style={{ justifyContent: "space-between" }}>
          <label className="styled-checkbox">
            <input
              type="checkbox"
              checked={trustAddress}
              onChange={(e) => setTrustAddress(e.target.checked)}
            />
            {t("directConnect.rememberTrust")}
          </label>
          <div>
            <button type="button" className="button-secondary" onClick={handleClose}>
              {t("common.cancel")}
            </button>{" "}
            <button type="button" className="button" onClick={handleConfirm}>
              {t("common.connect")}
            </button>
          </div>
        </div>
      </Modal>
    );
  }

  return (
    <Modal
      visible={visible}
      onClose={handleClose}
      className="settings-modal"
      closeOnOverlayClick
      title={t("directConnect.title")}
    >
      <div className="modal-body">
        <div className="settings-section">
          <p className="settings-description">{t("directConnect.hint")}</p>
          <input
            type="text"
            className="search-input direct-connect-input"
            placeholder={t("directConnect.placeholder")}
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            onKeyDown={handleKeyDown}
            autoFocus
          />
        </div>
      </div>
      <div className="modal-footer">
        <button
          type="button"
          className="button"
          onClick={handleResolve}
          disabled={resolving || !address.trim()}
        >
          {resolving ? "..." : t("common.connect")}
        </button>
      </div>
    </Modal>
  );
};
