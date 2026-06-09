import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders passkey-only authentication first", async () => {
    render(<App />);
    expect(await screen.findByText("EnergyLossPlus")).toBeInTheDocument();
    expect(screen.getByText("用 Passkey 安全登录，开始记录你的每日能量。")).toBeInTheDocument();
  });
});
