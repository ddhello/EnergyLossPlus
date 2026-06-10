import { afterEach, describe, expect, it, vi } from "vitest";
import { errorMessage, loginWithPasskey, passkeyUnavailableReason } from "./passkey";

describe("Passkey API errors", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("explains an unreachable API instead of only reporting Load failed", async () => {
    vi.stubGlobal("PublicKeyCredential", class {});
    vi.stubGlobal("navigator", { credentials: {} });
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Load failed")));

    await expect(loginWithPasskey("new-user")).rejects.toThrow(
      "Check API_BASE_URL and the API CORS origin"
    );
  });

  it("preserves string errors returned by Tauri invoke", () => {
    expect(errorMessage("unknown passkey user")).toBe("unknown passkey user");
  });

  it("includes DOMException names in credential errors", () => {
    expect(errorMessage(new DOMException("RP ID mismatch", "SecurityError"))).toBe(
      "SecurityError: RP ID mismatch"
    );
  });

  it("rejects web Passkeys in a Tauri custom-protocol page", () => {
    expect(passkeyUnavailableReason("tauri:")).toContain("AuthenticationServices");
  });
});
