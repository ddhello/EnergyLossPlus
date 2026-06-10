import type { Session } from "./types";
import { invoke } from "@tauri-apps/api/core";

const API_BASE = (import.meta.env.VITE_API_BASE_URL || "http://localhost:3000").replace(/\/+$/, "");
const isTauri = "__TAURI_INTERNALS__" in window;

interface ChallengeResponse {
  challengeId: string;
  publicKey: PublicKeyCredentialCreationOptions | PublicKeyCredentialRequestOptions;
}

export function isPasskeyAvailable(): boolean {
  return Boolean(window.PublicKeyCredential && navigator.credentials);
}

export async function registerWithPasskey(nickname: string, deviceName: string): Promise<Session> {
  ensurePasskeyAvailable();
  const challenge = await post<ChallengeResponse>("/auth/register/start", { nickname, deviceName });
  const credential = await runCredentialCeremony("registration", () =>
    navigator.credentials.create({
      publicKey: revivePublicKeyOptions(challenge.publicKey) as PublicKeyCredentialCreationOptions
    })
  );
  if (!credential) {
    throw new Error("Passkey registration was cancelled.");
  }
  return post<Session>("/auth/register/finish", {
    challengeId: challenge.challengeId,
    credential: serializeCredential(credential as PublicKeyCredential)
  });
}

export async function loginWithPasskey(nickname: string): Promise<Session> {
  ensurePasskeyAvailable();
  const challenge = await post<ChallengeResponse>("/auth/login/start", { nickname });
  const credential = await runCredentialCeremony("login", () =>
    navigator.credentials.get({
      publicKey: revivePublicKeyOptions(challenge.publicKey) as PublicKeyCredentialRequestOptions
    })
  );
  if (!credential) {
    throw new Error("Passkey login was cancelled.");
  }
  return post<Session>("/auth/login/finish", {
    challengeId: challenge.challengeId,
    credential: serializeCredential(credential as PublicKeyCredential)
  });
}

async function post<T>(path: string, body: unknown): Promise<T> {
  if (isTauri) {
    return invoke<T>("auth_post", { path, body });
  }

  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body)
    });
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(
      `Unable to reach the EnergyLossPlus API at ${API_BASE} from ${window.location.origin}. ` +
      `Check API_BASE_URL and the API CORS origin. ${detail}`
    );
  }
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json() as Promise<T>;
}

function ensurePasskeyAvailable(): void {
  if (!isPasskeyAvailable()) {
    throw new Error("This runtime does not support Passkey.");
  }
}

async function runCredentialCeremony(
  action: "registration" | "login",
  ceremony: () => Promise<Credential | null>
): Promise<Credential | null> {
  try {
    return await ceremony();
  } catch (error) {
    throw new Error(
      `Passkey ${action} failed at ${window.location.origin}: ${errorMessage(error)}`
    );
  }
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.name && error.name !== "Error" ? `${error.name}: ${error.message}` : error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error === "object") {
    const value = error as { name?: unknown; message?: unknown };
    const name = typeof value.name === "string" ? value.name : "";
    const message = typeof value.message === "string" ? value.message : "";
    if (name || message) {
      return [name, message].filter(Boolean).join(": ");
    }
  }
  return String(error);
}

function revivePublicKeyOptions(
  options: PublicKeyCredentialCreationOptions | PublicKeyCredentialRequestOptions
): PublicKeyCredentialCreationOptions | PublicKeyCredentialRequestOptions {
  const copy = structuredClone(options) as unknown as Record<string, unknown>;
  copy.challenge = base64UrlToBuffer(copy.challenge as string);

  if ("user" in copy && copy.user && typeof copy.user === "object") {
    const user = copy.user as Record<string, unknown>;
    user.id = base64UrlToBuffer(user.id as string);
  }

  if (Array.isArray(copy.allowCredentials)) {
    copy.allowCredentials = copy.allowCredentials.map((credential: Record<string, unknown>) => ({
      ...credential,
      id: base64UrlToBuffer(credential.id as string)
    }));
  }

  return copy as unknown as PublicKeyCredentialCreationOptions | PublicKeyCredentialRequestOptions;
}

function serializeCredential(credential: PublicKeyCredential): unknown {
  return {
    id: credential.id,
    rawId: bufferToBase64Url(credential.rawId),
    type: credential.type,
    transports: credentialTransports(credential),
    response: serializeResponse(credential.response)
  };
}

function credentialTransports(credential: PublicKeyCredential): string[] {
  const response = credential.response as AuthenticatorAttestationResponse;
  if (typeof response.getTransports === "function") {
    return response.getTransports();
  }
  return [];
}

function serializeResponse(response: AuthenticatorResponse): Record<string, string> {
  const output: Record<string, string> = {
    clientDataJSON: bufferToBase64Url(response.clientDataJSON)
  };
  const attestation = response as AuthenticatorAttestationResponse;
  if (attestation.attestationObject) {
    output.attestationObject = bufferToBase64Url(attestation.attestationObject);
  }
  const assertion = response as AuthenticatorAssertionResponse;
  if (assertion.authenticatorData) {
    output.authenticatorData = bufferToBase64Url(assertion.authenticatorData);
    output.signature = bufferToBase64Url(assertion.signature);
    if (assertion.userHandle) {
      output.userHandle = bufferToBase64Url(assertion.userHandle);
    }
  }
  return output;
}

function base64UrlToBuffer(value: string): ArrayBuffer {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes.buffer;
}

function bufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
