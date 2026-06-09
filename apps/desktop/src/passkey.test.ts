import { afterEach, describe, expect, it, vi } from "vitest";
import { loginWithPasskey } from "./passkey";

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
});
