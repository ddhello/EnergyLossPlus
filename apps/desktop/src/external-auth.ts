import { invoke } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Session } from "./types";

const API_BASE = (
  import.meta.env.VITE_API_BASE_URL || "https://x38dzo14cd.execute-api.ap-northeast-1.amazonaws.com"
).replace(/\/+$/, "");
const stateKey = "energylossplus.externalAuthState";
const verifierKey = "energylossplus.externalAuthVerifier";
const isTauri = "__TAURI_INTERNALS__" in window;

export function isExternalAuthAvailable(): boolean {
  return isTauri || (!import.meta.env.DEV && window.location.protocol === "https:");
}

export async function startExternalAuth(
  mode: "login" | "register",
  nickname: string,
  deviceName: string
): Promise<void> {
  const state = randomState();
  const verifier = randomState();
  window.localStorage.setItem(stateKey, state);
  window.localStorage.setItem(verifierKey, verifier);
  const codeChallenge = await pkceChallenge(verifier);
  const query = new URLSearchParams({ state, mode, nickname, deviceName, codeChallenge });
  if (isTauri) {
    await openUrl(`${API_BASE}/auth/app?${query}`);
  } else {
    query.set("callbackOrigin", window.location.origin);
    window.location.assign(`${API_BASE}/auth/app?${query}`);
  }
}

export async function listenForExternalAuth(
  onSession: (session: Session) => void,
  onError: (error: unknown) => void
): Promise<() => void> {
  const handleUrls = (urls: string[]) => {
    for (const url of urls) {
      void handleCallback(url).then(onSession).catch(onError);
    }
  };
  if (isTauri) {
    const unlisten = await onOpenUrl(handleUrls);
    const current = await getCurrent();
    if (current) handleUrls(current);
    return unlisten;
  }
  if (new URL(window.location.href).searchParams.has("code")) {
    handleUrls([window.location.href]);
  }
  return () => {};
}

async function handleCallback(value: string): Promise<Session> {
  const url = new URL(value);
  const isAppCallback = url.protocol === "energylossplus:" && url.host === "auth" && url.pathname === "/callback";
  const isWebCallback = url.origin === window.location.origin;
  if (!isAppCallback && !isWebCallback) {
    throw new Error("收到无效的登录回调。");
  }
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const expectedState = window.localStorage.getItem(stateKey);
  const codeVerifier = window.localStorage.getItem(verifierKey);
  if (!code || !state || !expectedState || !codeVerifier || state !== expectedState) {
    throw new Error("登录回调 state 校验失败，请重新登录。");
  }
  window.localStorage.removeItem(stateKey);
  window.localStorage.removeItem(verifierKey);
  if (isTauri) {
    return invoke<Session>("auth_post", {
      path: "/auth/app/exchange",
      body: { code, state, codeVerifier }
    });
  }
  window.history.replaceState({}, "", `${window.location.pathname}${window.location.hash}`);
  const response = await fetch(`${API_BASE}/auth/app/exchange`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ code, state, codeVerifier })
  });
  if (!response.ok) throw new Error(await response.text());
  return response.json() as Promise<Session>;
}

function randomState(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

async function pkceChallenge(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  let binary = "";
  for (const byte of new Uint8Array(digest)) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
