import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faGlobe } from "@fortawesome/free-solid-svg-icons";
import type { Server } from "../bindings";
import { Modal, ModalContent } from "./Modal";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../stores";

interface TermsOfServiceModalProps {
  visible: boolean;
  onClose: () => void;
  onJoin: () => void;
  linkClick: (link: string | null) => void;
  server?: Server;
}

export const TermsOfServiceModal = ({
  visible,
  onClose,
  onJoin,
  linkClick,
  server,
}: TermsOfServiceModalProps) => {
  const { t } = useTranslation();

  if (!server || !server.terms_of_service) return;

  const handleAccept = async () => {
    if (server.id) {
      await useSettingsStore.getState().setAcceptedTos(server.id, true);
    }
    onClose();
    onJoin();
  };

  return (
    <Modal visible={visible} onClose={onClose} title={t("servers.tosViewLink")}>
      <ModalContent>
        <p>{t("servers.tosRequired")}</p>
        {server.terms_of_service.url && (
          <div className="server-links">
            <button
              type="button"
              className="button-secondary server-link-button"
              onClick={() => {
                onClose();
                linkClick(server.terms_of_service?.url ?? "");
              }}
            >
              <FontAwesomeIcon icon={faGlobe} />
              <span>{t("servers.tosViewLink")}</span>
            </button>
          </div>
        )}
        <div style={{ marginTop: 12, display: "flex", gap: 8 }}>
          <button type="button" className="button" onClick={handleAccept}>
            {t("common.accept")}
          </button>
          <button type="button" className="button-secondary" onClick={onClose}>
            {t("common.decline")}
          </button>
        </div>
      </ModalContent>
    </Modal>
  );
};
