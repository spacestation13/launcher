import { faStar, faUsers } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import type { Favorite, Server } from "../bindings";
import { useConnect } from "../hooks";
import { useConfigStore, useSettingsStore } from "../stores";
import { useUiStateStore, RecentConnection } from "../stores/uiStateStore";
import { ServerItem } from "./ServerItem";

const RECENT_VISIBLE_DEFAULT = 3;

interface HomePageProps {
  servers: Server[];
}

function RecentConnectionItem({
  connection,
  server,
}: {
  connection: RecentConnection;
  server?: Server;
}) {
  const { t } = useTranslation();
  const { connectToAddress } = useConnect();
  const config = useConfigStore((s) => s.config);
  const toggleFavorite = useSettingsStore((s) => s.toggleFavorite);
  const isFavorited = useSettingsStore((s) =>
    s.isFavorited({ type: "address", address: connection.address, name: null }),
  );

  const supportsHub = server?.auth_methods?.includes("hub") ?? false;

  const handleConnect = async () => {
    await connectToAddress(
      connection.address,
      "HomePage.RecentConnection",
      connection.serverId ?? undefined,
    );
  };

  const handleToggleFavorite = () => {
    const fav: Favorite = {
      type: "address",
      address: connection.address,
      name: server?.name ?? connection.serverName ?? null,
    };
    toggleFavorite(fav, !isFavorited);
  };

  const displayName = server?.name ?? connection.serverName ?? connection.address;
  const showAddress = displayName !== connection.address;

  return (
    <div className="server-item">
      <div className="server-item-row">
        <div className="server-info">
          <div className="server-name">{displayName}</div>
          {showAddress && (
            <div className="server-details">
              <div className="detail-line">
                <span>{connection.address}</span>
              </div>
            </div>
          )}
        </div>
        {server?.players != null && (
          <div className="server-status">
            <div className="server-counts">
              <div className="player-count">
                <FontAwesomeIcon icon={faUsers} className="player-icon" />
                {server.players}
                {server.data?.popcap != null && `/${server.data.popcap}`}
              </div>
            </div>
          </div>
        )}
        <div className="connect-group">
          <button type="button" className="button connect-button" onClick={handleConnect}>
            {config?.features.connect_logo && (
              <img
                src={supportsHub ? "/logo-ss13.png" : "/byond.png"}
                alt=""
                className="connect-auth-icon"
              />
            )}
            {t("common.join")}
          </button>
          <button
            type="button"
            className={`notify-toggle favorite-toggle ${isFavorited ? "favorited" : ""}`}
            onClick={handleToggleFavorite}
            title={isFavorited ? t("servers.unfavorite") : t("servers.favorite")}
          >
            <FontAwesomeIcon icon={faStar} />
          </button>
        </div>
      </div>
    </div>
  );
}

function FavoriteAddressItem({ favorite }: { favorite: Extract<Favorite, { type: "address" }> }) {
  const { t } = useTranslation();
  const { connectToAddress } = useConnect();
  const toggleFavorite = useSettingsStore((s) => s.toggleFavorite);

  const handleConnect = async () => {
    await connectToAddress(favorite.address, "HomePage.FavoriteAddress");
  };

  const handleUnfavorite = () => {
    toggleFavorite(favorite, false);
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
        <div className="connect-group">
          <button type="button" className="button connect-button" onClick={handleConnect}>
            {t("common.join")}
          </button>
          <button
            type="button"
            className="notify-toggle favorite-toggle favorited"
            onClick={handleUnfavorite}
            title={t("servers.unfavorite")}
          >
            <FontAwesomeIcon icon={faStar} />
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
  const [showAllRecent, setShowAllRecent] = useState(false);

  const recentItems = useMemo(() => {
    return recentConnections.map((conn) => {
      const server = conn.serverId
        ? servers.find((s) => s.id === conn.serverId)
        : undefined;
      return { connection: conn, server };
    });
  }, [servers, recentConnections]);

  const visibleRecent = showAllRecent
    ? recentItems
    : recentItems.slice(0, RECENT_VISIBLE_DEFAULT);

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
            {visibleRecent.map(({ connection, server }) => (
              <RecentConnectionItem
                key={connection.address}
                connection={connection}
                server={server}
              />
            ))}
          </div>
          {recentItems.length > RECENT_VISIBLE_DEFAULT && (
            <button
              type="button"
              className="button-secondary"
              onClick={() => setShowAllRecent(!showAllRecent)}
              style={{ marginTop: "0.5rem" }}
            >
              {showAllRecent
                ? t("common.showLess")
                : t("common.showMore", { count: recentItems.length - RECENT_VISIBLE_DEFAULT })}
            </button>
          )}
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
