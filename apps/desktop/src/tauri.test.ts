import { describe, expect, it } from "vitest";
import { loadSnapshot } from "./tauri";

describe("browser development mode", () => {
  it("loads a local demo session without contacting the deployed API", async () => {
    const snapshot = await loadSnapshot();

    expect(snapshot.session).toMatchObject({
      token: "browser-dev-demo",
      userId: "demo",
      deviceName: "Vite"
    });
    expect(snapshot.syncStatus).toBe("cached");
  });
});
