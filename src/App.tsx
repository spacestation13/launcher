import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

import { commands } from "./bindings";
import {
  AccountInfo,
  ByondLoginModal,
  DirectConnectModal,
  ErrorNotifications,
  GameConnectionModal,
  HomePage,
  HubAnnouncements,
  NewsPage,
  RelayDropdown,
  ServerFilterPanel,
  ServerItem,
  SettingsModal,
  SinglePlayerPanel,
  SocialLinks,
  Titlebar,
  UpdateNotification,
  WineSetupModal,
} from "./components";
import type { ViewMode } from "./components/ServerFilterPanel";
import {
  AuthFlowProvider,
  ErrorProvider,
  useAnnouncements,
  useAppBootstrap,
  useDeepLink,
  useError,
  useGameConnection,
  useServerFilters,
  useWine,
} from "./hooks";
import {
  useByondStore,
  useConfigStore,
  useServerStore,
  useSettingsStore,
  useSteamStore,
} from "./stores";
import { useUiStateStore } from "./stores/uiStateStore";
const AppContent = () => {
  const { t } = useTranslation();
  const { errors, dismissError, showError } = useError();

  useAppBootstrap();

  const settingsLoaded = useSettingsStore((s) => s.loaded);
  const config = useConfigStore((s) => s.config);
  const steamAvailable = useSteamStore((s) => s.available);

  const {
    servers,
    loading: serversLoading,
    error: serversError,
    relays,
    selectedRelay,
    setSelectedRelay,
    lastUpdated,
  } = useServerStore(
    useShallow((s) => ({
      servers: s.servers,
      loading: s.loading,
      error: s.error,
      relays: s.relays,
      selectedRelay: s.selectedRelay,
      setSelectedRelay: s.setSelectedRelay,
      lastUpdated: s.lastUpdated,
    })),
  );

  const {
    authMode,
    theme,
    devMode,
    renderingPipeline,
    saveAuthMode,
    saveTheme,
    saveRenderingPipeline,
  } = useSettingsStore(
    useShallow((s) => ({
      authMode: s.authMode,
      theme: s.theme,
      devMode: s.devMode,
      renderingPipeline: s.renderingPipeline,
      saveAuthMode: s.saveAuthMode,
      saveTheme: s.saveTheme,
      saveRenderingPipeline: s.saveRenderingPipeline,
    })),
  );

  const {
    gameConnectionState,
    connectedServerName,
    restartReason,
    closeGameConnectionModal,
    showGameConnectionModal,
  } = useGameConnection();

  const {
    platform,
    status: wineStatus,
    setupProgress: wineSetupProgress,
    isSettingUp: wineIsSettingUp,
    needsSetup: wineNeedsSetup,
    checkStatus: checkWineStatus,
    initializePrefix: initializeWinePrefix,
    resetPrefix: resetWinePrefix,
  } = useWine();

  const byondLoginVisible = useByondStore((s) => s.loginVisible);
  const byondLoggingIn = useByondStore((s) => s.loggingIn);

  const [settingsVisible, setSettingsVisible] = useState(false);
  const [relayDropdownOpen, setRelayDropdownOpen] = useState(false);
  const [wineModalVisible, setWineModalVisible] = useState(false);
  const [directConnectVisible, setDirectConnectVisible] = useState(false);

  const showHome = config?.features.favorites ?? false;
  const lastViewMode = useUiStateStore((s) => s.lastViewMode) as ViewMode | null;
  const setLastViewMode = useUiStateStore((s) => s.setLastViewMode);

  const defaultMode: ViewMode = showHome ? "home" : "browse";
  const viewMode =
    lastViewMode && (lastViewMode !== "home" || showHome) ? lastViewMode : defaultMode;

  const handleViewModeChange = (mode: ViewMode) => {
    setLastViewMode(mode);
  };

  const announcements = useAnnouncements();
  const lastReadAnnouncement = useUiStateStore((s) => s.lastReadAnnouncement);
  const newsAnnouncements = announcements.filter((a) => a.kind === "announcement");
  const hasUnreadNews =
    newsAnnouncements.length > 0 && newsAnnouncements[0].id !== lastReadAnnouncement;

  const filters = useServerFilters(servers, config);
  const { showHubStatus, filteredServers } = filters;

  useDeepLink();

  useEffect(() => {
    document.documentElement.className = `theme-${theme}`;
  }, [theme]);

  useEffect(() => {
    const unlisten = listen("relay-login-failed", () => {
      showError(t("connection.relayLoginFailed"));
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [showError, t]);

  useEffect(() => {
    if (platform === "linux") {
      checkWineStatus().then((status) => {
        if (!status.prefix_initialized) {
          setWineModalVisible(true);
        }
      });
    }
  }, [platform, checkWineStatus]);

  const handleAuthModeChange = useCallback(
    async (mode: typeof authMode) => {
      try {
        await saveAuthMode(mode);
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
      }
    },
    [saveAuthMode, showError],
  );

  const handleThemeChange = useCallback(
    async (newTheme: typeof theme) => {
      try {
        await saveTheme(newTheme);
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
      }
    },
    [saveTheme, showError],
  );

  const handleRenderingPipelineChange = useCallback(
    async (pipeline: typeof renderingPipeline) => {
      try {
        await saveRenderingPipeline(pipeline);
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
      }
    },
    [saveRenderingPipeline, showError],
  );

  const handleWineSetup = useCallback(
    async (pipeline: typeof renderingPipeline) => {
      await saveRenderingPipeline(pipeline);
      await initializeWinePrefix(pipeline);
    },
    [initializeWinePrefix, saveRenderingPipeline],
  );

  const handleWineRetry = useCallback(async () => {
    await checkWineStatus();
  }, [checkWineStatus]);

  const handleWineModalClose = useCallback(() => {
    if (!wineIsSettingUp && !wineNeedsSetup) {
      setWineModalVisible(false);
    }
  }, [wineIsSettingUp, wineNeedsSetup]);

  const handleRelaySelect = useCallback(
    (relayId: string) => {
      setSelectedRelay(relayId);
      setRelayDropdownOpen(false);
    },
    [setSelectedRelay],
  );

  const toggleRelayDropdown = useCallback(() => {
    setRelayDropdownOpen((prev) => !prev);
  }, []);

  if (!settingsLoaded) return null;

  return (
    <div className="launcher-frame">
      {theme === "crt" && (
        <>
          <div className="crt-bezel" />
          <div className="crt" />
        </>
      )}
      <ByondLoginModal
        visible={byondLoginVisible}
        loggingIn={byondLoggingIn}
        onClose={() => commands.cancelByondLogin()}
      />
      {config?.features.direct_connect && (
        <DirectConnectModal
          visible={directConnectVisible}
          onClose={() => setDirectConnectVisible(false)}
        />
      )}
      <UpdateNotification />
      <ErrorNotifications errors={errors} onDismiss={dismissError} />
      <SettingsModal
        visible={settingsVisible && !byondLoginVisible}
        authMode={authMode}
        theme={theme}
        steamAvailable={steamAvailable}
        devMode={devMode}
        platform={platform}
        wineStatus={wineStatus}
        renderingPipeline={renderingPipeline}
        isResettingWine={wineIsSettingUp}
        onAuthModeChange={handleAuthModeChange}
        onThemeChange={handleThemeChange}
        onRenderingPipelineChange={handleRenderingPipelineChange}
        onResetWinePrefix={resetWinePrefix}
        onClose={() => setSettingsVisible(false)}
      />
      <GameConnectionModal
        visible={showGameConnectionModal}
        state={gameConnectionState}
        serverName={connectedServerName}
        restartReason={restartReason}
        onClose={closeGameConnectionModal}
      />
      <WineSetupModal
        visible={wineModalVisible}
        status={wineStatus}
        progress={wineSetupProgress}
        isSettingUp={wineIsSettingUp}
        renderingPipeline={renderingPipeline}
        onSetup={handleWineSetup}
        onRenderingPipelineChange={handleRenderingPipelineChange}
        onClose={handleWineModalClose}
        onRetry={handleWineRetry}
      />

      <div className="launcher">
        <Titlebar />

        <main className="main-content">
          <section className="section servers-section">
            {config && (
              <ServerFilterPanel
                features={config.features}
                filters={filters}
                serverCount={filteredServers.length}
                playerCount={filteredServers
                  .filter((s) => s.status === "available")
                  .reduce((sum, s) => sum + (s.players ?? 0), 0)}
                viewMode={viewMode}
                onViewModeChange={handleViewModeChange}
                showHome={showHome}
                onDirectConnect={
                  config.features.direct_connect ? () => setDirectConnectVisible(true) : undefined
                }
                hasEighteenPlus={servers.filter((s) => s.is_18_plus).length > 0}
                hasUnreadNews={hasUnreadNews}
              />
            )}
            <HubAnnouncements announcements={announcements} />
            {viewMode === "news" ? (
              <NewsPage announcements={announcements} />
            ) : viewMode === "singleplayer" && config?.features.singleplayer ? (
              <SinglePlayerPanel />
            ) : viewMode === "home" ? (
              <HomePage servers={servers} />
            ) : (
              <div className="server-list">
                {serversLoading && servers.length === 0 && (
                  <div className="server-loading">{t("servers.loading")}</div>
                )}
                {serversError && (
                  <div className="server-error">
                    {t("errors.prefix", { message: serversError })}
                  </div>
                )}
                {filteredServers.map((server) => (
                  <ServerItem
                    key={server.url}
                    server={server}
                    showHubStatus={showHubStatus}
                  />
                ))}
              </div>
            )}
            {lastUpdated !== null && (
              <div className="refresh-bar">
                <div key={lastUpdated} className="refresh-bar-fill" />
              </div>
            )}
          </section>
        </main>

        <footer className="section footer">
          <div className="account-info">
            <AccountInfo />
          </div>
          <div className="footer-actions">
            {config && config.social_links.length > 0 && (
              <SocialLinks links={config.social_links} />
            )}
            {config?.features.relay_selector && (
              <RelayDropdown
                relays={relays}
                selectedRelay={selectedRelay}
                isOpen={relayDropdownOpen}
                onToggle={toggleRelayDropdown}
                onSelect={handleRelaySelect}
              />
            )}
            <button
              type="button"
              className="button-secondary settings-button"
              onClick={() => setSettingsVisible(true)}
              title={t("common.settings")}
            >
              {t("common.settings")}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
};

const App = () => {
  return (
    <ErrorProvider>
      <AuthFlowProvider>
        <AppContent />
      </AuthFlowProvider>
    </ErrorProvider>
  );
};

export default App;
