import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { Modal, ModalContent, ModalSpinner } from "./Modal";

export type SteamAuthModalState = "idle" | "loading" | "linking" | "tos" | "error";

interface SteamAuthModalProps {
  visible: boolean;
  state: SteamAuthModalState;
  error?: string;
  linkingUrl?: string;
  suggestedUsername?: string;
  authProviderName: string;
  onAuthenticate: (createAccount: boolean, acceptedTos?: boolean, preferredUsername?: string) => void;
  tosUrl?: string;
  privacyUrl?: string;
  onClose: () => void;
}

export const SteamAuthModal = ({
  visible,
  state,
  error,
  linkingUrl,
  suggestedUsername,
  authProviderName,
  onAuthenticate,
  tosUrl,
  privacyUrl,
  onClose,
}: SteamAuthModalProps) => {
  const { t } = useTranslation();
  const [username, setUsername] = useState(suggestedUsername ?? "");
  useEffect(() => {
    if (suggestedUsername) setUsername(suggestedUsername);
  }, [suggestedUsername]);
  const openLinkingUrl = async () => {
    if (linkingUrl) {
      await commands.openUrl(linkingUrl);
      onClose();
    }
  };

  const titleMap: Record<SteamAuthModalState, string> = {
    idle: t("auth.steamAuth"),
    loading: t("auth.authenticating"),
    linking: t("auth.steamLinkingTitle"),
    tos: t("auth.tosRequired"),
    error: t("auth.authFailed"),
  };

  return (
    <Modal visible={visible} onClose={onClose} title={titleMap[state]}>
      {state === "idle" && (
        <ModalContent>
          <p>{t("auth.steamAuthenticating")}</p>
          <ModalSpinner />
        </ModalContent>
      )}
      {state === "loading" && (
        <ModalContent>
          <p>{t("auth.steamValidating")}</p>
          <ModalSpinner />
        </ModalContent>
      )}
      {state === "linking" && (
        <ModalContent>
          <p>{t("auth.steamNoAccount", { provider: authProviderName })}</p>
          <p>{t("auth.steamHaveAccount", { provider: authProviderName })}</p>
          <div className="auth-modal-buttons">
            <button type="button" className="button" onClick={openLinkingUrl}>
              {t("auth.steamYesLink")}
            </button>
            <button type="button" className="button-secondary" onClick={() => onAuthenticate(true)}>
              {t("auth.steamNoStart")}
            </button>
          </div>
        </ModalContent>
      )}
      {state === "tos" && (
        <ModalContent>
          <div className="auth-username-field">
            <label htmlFor="steam-username">{t("auth.chooseUsername")}</label>
            <input
              id="steam-username"
              type="text"
              className="auth-username-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={suggestedUsername}
            />
          </div>
          {error && <p className="auth-error-message">{error}</p>}
          <p>{t("auth.tosPrompt")}</p>
          <div className="hub-login-links">
            {tosUrl && (
              <button
                type="button"
                className="hub-login-toggle"
                onClick={() => commands.openUrl(tosUrl)}
              >
                {t("auth.viewTos")}
              </button>
            )}
            {tosUrl && privacyUrl && <span className="hub-login-separator">·</span>}
            {privacyUrl && (
              <button
                type="button"
                className="hub-login-toggle"
                onClick={() => commands.openUrl(privacyUrl)}
              >
                {t("auth.viewPrivacy")}
              </button>
            )}
          </div>
          <div className="auth-modal-buttons">
            <button
              type="button"
              className="button"
              disabled={!username.trim()}
              onClick={() => onAuthenticate(true, true, username.trim())}
            >
              {t("auth.acceptAndContinue")}
            </button>
            <button type="button" className="button-secondary" onClick={onClose}>
              {t("common.decline")}
            </button>
          </div>
        </ModalContent>
      )}
      {state === "error" && (
        <ModalContent>
          <p className="auth-error-message">{error}</p>
          <button type="button" className="button" onClick={() => onAuthenticate(false)}>
            {t("common.tryAgain")}
          </button>
        </ModalContent>
      )}
    </Modal>
  );
};
