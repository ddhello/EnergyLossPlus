import { fireEvent, render, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as tauri from "./tauri";

describe("App", () => {
  it("opens the local dashboard in browser development mode", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".phone-app")).toBeInTheDocument());
    expect(container.querySelector(".auth-card")).not.toBeInTheDocument();
  });

  it("moves the meal segment slider when selecting another meal", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".segment-slider")).toBeInTheDocument());
    const segmented = container.querySelector(".segmented") as HTMLElement;
    fireEvent.click(within(segmented).getByText("晚餐"));

    expect(segmented).toHaveStyle("--segment-index: 2");
    expect(within(segmented).getByText("晚餐")).toHaveClass("active");
  });

  it("allows calorie number fields to be temporarily empty", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".add-card")).toBeInTheDocument());
    const view = within(container);
    const calories = view.getByLabelText("热量") as HTMLInputElement;
    const dailyTarget = view.getByLabelText("每日目标") as HTMLInputElement;

    fireEvent.change(calories, { target: { value: "" } });
    fireEvent.change(dailyTarget, { target: { value: "" } });

    expect(calories.value).toBe("");
    expect(dailyTarget.value).toBe("");
    expect(view.getByText("添加记录").closest("button")).toBeDisabled();
  });

  it("saves a changed daily calorie target", async () => {
    const { container } = render(<App />);
    const updateDailyTarget = vi.spyOn(tauri, "updateDailyTarget");

    await waitFor(() => expect(container.querySelector(".target-row")).toBeInTheDocument());
    const dailyTarget = container.querySelector(".target-row input") as HTMLInputElement;

    fireEvent.change(dailyTarget, { target: { value: "2500" } });
    fireEvent.blur(dailyTarget);

    await waitFor(() => expect(updateDailyTarget).toHaveBeenCalledWith("browser-dev-demo", 2500));
  });

  it("shows an explicit animated status while a network request is pending", async () => {
    const { container } = render(<App />);
    const syncSnapshot = vi.spyOn(tauri, "syncSnapshot").mockImplementationOnce(() => new Promise(() => {}));

    await waitFor(() => expect(container.querySelector(".phone-app")).toBeInTheDocument());
    fireEvent.click(within(container).getByRole("button", { name: "同步" }));

    const status = within(container).getByRole("status");
    expect(status).toHaveTextContent("正在同步云端数据");
    expect(status.querySelector(".spinner")).toBeInTheDocument();
    expect(within(container).getByRole("button", { name: "同步" })).toBeDisabled();

    syncSnapshot.mockRestore();
  });

  it("calculates package calories from a kJ nutrition label", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".add-card")).toBeInTheDocument());
    const addCard = container.querySelector(".add-card") as HTMLElement;
    const view = within(addCard);
    const modeSwitch = view.getByLabelText("热量输入方式");

    fireEvent.click(view.getByText("包装换算"));
    expect(modeSwitch).toHaveStyle("--switch-index: 1");
    expect(addCard.querySelector(".calorie-input-panel.package")).toBeInTheDocument();
    fireEvent.click(view.getByText("kJ"));
    expect(view.getByLabelText("能量单位")).toHaveStyle("--switch-index: 1");
    fireEvent.change(view.getByLabelText("营养表能量"), { target: { value: "840" } });
    fireEvent.change(view.getByLabelText("营养表对应克数"), { target: { value: "100" } });
    fireEvent.change(view.getByLabelText("一包克数"), { target: { value: "50" } });
    fireEvent.change(view.getByLabelText("吃了什么"), { target: { value: "能量棒" } });

    expect(view.getByText("100", { exact: false })).toBeInTheDocument();
    expect(view.getByText("添加记录").closest("button")).toBeEnabled();
  });

  it("hides the dock when scrolling down and shows it when scrolling up", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".bottom-nav")).toBeInTheDocument());
    const dock = container.querySelector(".bottom-nav") as HTMLElement;

    Object.defineProperty(window, "scrollY", { configurable: true, value: 120 });
    fireEvent.scroll(window);
    await waitFor(() => expect(dock).toHaveClass("hidden"));

    Object.defineProperty(window, "scrollY", { configurable: true, value: 80 });
    fireEvent.scroll(window);
    await waitFor(() => expect(dock).not.toHaveClass("hidden"));
  });
});
