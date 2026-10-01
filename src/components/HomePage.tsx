import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import type { Favorite, Server } from "../bindings";
import { useConnect } from "../hooks";
import { useSettingsStore } from "../stores";
import { useUiStateStore, RecentConnection } from "../stores/uiStateStore";
import { ServerItem } from "./ServerItem";

interface HomePageProps {
  servers: Server[];
}

function RecentAddressItem({ connection }: { connection: RecentConnection }) {
  const { t } = useTranslation();
  const { connectToAddress } = useConnect();

  const handleConnect = async () => {
    await connectToAddress(connection.address, "HomePage.RecentAddress", connection.serverId ?? undefined);
  };

  return (
    <div className="server-item">
      <div className="server-item-row">
        <div className="server-info">
          <div className="server-name">{connection.serverName ?? connection.address}</div>
          {connection.serverName && (
            <div className="server-details">
              <div className="detail-line">{connection.address}</div>
            </div>
          )}
        </div>
        <div className="server-actions">
          <button type="button" className="button" onClick={handleConnect}>
            {t("common.connect")}
          </button>
        </div>
      </div>
    </div>
  );
}

function FavoriteAddressItem({ favorite }: { favorite: Extract<Favorite, { type: "address" }> }) {
  const { t } = useTranslation();
  const { connectToAddress } = useConnect();

  const handleConnect = async () => {
    await connectToAddress(favorite.address, "HomePage.FavoriteAddress");
  };

  return (
    <div className="server-item">
      <div className="server-item-row">
        <div className="server-info">
          <div className="server-name">{favorite.name ?? favorite.address}</div>
          {favorite.name && (
            <div className="server-details">
              <div className="detail-line">{favorite.address}</div>
            </div>
          )}
        </div>
        <div className="server-actions">
          <button type="button" className="button" onClick={handleConnect}>
            {t("common.connect")}
          </button>
        </div>
      </div>
    </div>
  );
}

export const HomePage = ({ servers }: HomePageProps) => {
  const { t } = useTranslation();
  const recentConnections = useUiStateStore((s) => s.recentConnections);
  const favorites = useSettingsStore((s) => s.favorites);

  const recentItems = useMemo(() => {
    return recentConnections.map((conn) => {
      const server = conn.serverId
        ? servers.find((s) => s.id === conn.serverId && s.status === "available")
        : undefined;
      return { connection: conn, server };
    });
  }, [servers, recentConnections]);

  const favoriteItems = useMemo(() => {
    return favorites.map((fav) => {
      if (fav.type === "server") {
        const server = servers.find((s) => s.id === fav.id);
        return { favorite: fav, server };
      }
      return { favorite: fav, server: undefined };
    });
  }, [servers, favorites]);

  const hasContent = recentItems.length > 0 || favoriteItems.length > 0;

  return (
    <div className="home-page">
      {recentItems.length > 0 && (
        <div className="home-section">
          <div className="home-section-title">{t("home.continuePlaying")}</div>
          <div className="server-list home-server-list">
            {recentItems.map(({ connection, server }) =>
              server ? (
                <ServerItem key={server.id} server={server} />
              ) : (
                <RecentAddressItem key={connection.address} connection={connection} />
              ),
            )}
          </div>
        </div>
      )}
      {favoriteItems.length > 0 && (
        <div className="home-section">
          <div className="home-section-title">{t("home.favorites")}</div>
          <div className="server-list home-server-list">
            {favoriteItems.map(({ favorite, server }) =>
              server ? (
                <ServerItem key={server.id} server={server} />
              ) : favorite.type === "address" ? (
                <FavoriteAddressItem key={favorite.address} favorite={favorite} />
              ) : null,
            )}
          </div>
        </div>
      )}
      {!hasContent && <div className="home-empty">{t("home.noFavorites")}</div>}
    </div>
  );
};
