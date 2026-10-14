import { listen } from "@tauri-apps/api/event";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useEffect, useRef } from "react";

import { commands } from "../bindings";
import { useConnect } from "./useConnect";
import { useServerStore } from "../stores/serverStore";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type ConnectTarget =
  | { kind: "address"; address: string }
  | { kind: "server_id"; id: string };

function parseConnectTarget(raw: string): ConnectTarget | null {
  if (UUID_RE.test(raw)) {
    return { kind: "server_id", id: raw };
  }

  if (raw.includes(":")) {
    return { kind: "address", address: raw };
  }

  return null;
}

function parseDeepLink(raw: string): ConnectTarget | null {
  try {
    const url = new URL(raw);
    if (url.protocol !== "ss13:") return null;

    const host = url.hostname;
    if (!host) return null;

    const port = url.port;
    return parseConnectTarget(port ? `${host}:${port}` : host);
  } catch {
    return null;
  }
}

export function useDeepLink() {
  const { connect, connectToAddress } = useConnect();
  const connectRef = useRef(connect);
  const connectToAddressRef = useRef(connectToAddress);
  connectRef.current = connect;
  connectToAddressRef.current = connectToAddress;

  useEffect(() => {
    let unlistenDeepLink: (() => void) | undefined;
    let unlistenSingleInstance: (() => void) | undefined;
    let unlistenSteamJoin: (() => void) | undefined;
    let unsubscribePending: (() => void) | undefined;

    const connectToServerId = (id: string, source: string) => {
      unsubscribePending?.();
      unsubscribePending = undefined;

      const { servers, loading } = useServerStore.getState();
      if (servers.some((s) => s.id === id)) {
        connectRef.current(id, source);
        return;
      }
      if (!loading) {
        console.warn(`Join target ${id} from ${source} is not in the server list`);
        return;
      }

      // Joins from a cold start arrive before the first server list fetch, so wait for it
      // rather than dropping the join.
      unsubscribePending = useServerStore.subscribe((state) => {
        if (state.loading) return;
        unsubscribePending?.();
        unsubscribePending = undefined;
        if (state.servers.some((s) => s.id === id)) {
          connectRef.current(id, source);
        } else {
          console.warn(`Join target ${id} from ${source} is not in the server list`);
        }
      });
    };

    const dispatchTarget = (target: ConnectTarget, source: string) => {
      if (target.kind === "address") {
        connectToAddressRef.current(target.address, source);
      } else {
        connectToServerId(target.id, source);
      }
    };

    const handleUrls = (urls: string[]) => {
      for (const raw of urls) {
        const target = parseDeepLink(raw);
        if (target) {
          dispatchTarget(target, "deep-link");
        }
      }
    };

    const setup = async () => {
      const initialUrls = await commands.getInitialDeepLinks();
      if (initialUrls.length > 0) {
        handleUrls(initialUrls);
      } else {
        const steamLaunch = await commands.getSteamLaunchOptions().catch(() => null);
        const connectStr =
          steamLaunch?.status === "ok" ? steamLaunch.data.connect_target : null;
        if (connectStr) {
          const target = parseConnectTarget(connectStr);
          if (target) {
            dispatchTarget(target, "steam-launch");
          }
        }
      }

      unlistenDeepLink = await onOpenUrl(handleUrls);
      unlistenSingleInstance = await listen<string[]>("deep-link://new-url", (event) =>
        handleUrls(event.payload),
      );
      unlistenSteamJoin = await listen<string>("steam-join-requested", (event) => {
        const target = parseConnectTarget(event.payload);
        if (target) {
          dispatchTarget(target, "steam-join");
        }
      });
    };

    setup();

    return () => {
      unlistenDeepLink?.();
      unlistenSingleInstance?.();
      unlistenSteamJoin?.();
      unsubscribePending?.();
    };
  }, []);
}
