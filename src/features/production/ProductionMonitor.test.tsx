// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProductionMonitor, PRODUCTION_MONITOR_PAGE_SIZE } from "./ProductionMonitor";

const item = (ordinal: number, status = "PENDING", extra: Record<string, unknown> = {}) => ({
  id: `item-${ordinal}`,
  ordinal,
  status,
  name: `镜头 ${ordinal}`,
  ...extra,
});

afterEach(cleanup);

describe("ProductionMonitor", () => {
  it("renders all summary counts, terminal progress, success rate and Chinese status labels", () => {
    render(
      <ProductionMonitor
        batch={{
          id: "batch-1",
          name: "第一批",
          status: "RUNNING",
          items: [item(1, "PENDING"), item(2, "RUNNING"), item(3, "SUCCEEDED", { videoUrl: "https://example.test/video.mp4" }), item(4, "FAILED"), item(5, "CANCELLED"), item(6, "SKIPPED")],
          total: 6,
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "第一批" })).toBeTruthy();
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("6总数");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1等待中");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1生成中");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1成功");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1失败");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1已取消");
    expect(screen.getByTestId("production-monitor-summary").textContent).toContain("1已跳过");
    expect(screen.getByLabelText("终态进度 67%")).toBeTruthy();
    expect(screen.getByLabelText("成功率 17%")).toBeTruthy();
    expect(screen.getAllByText("生成中").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已取消").length).toBeGreaterThan(0);
  });

  it("maps paused batch and item statuses to 已暂停", () => {
    render(<ProductionMonitor batch={{ status: "PAUSED", items: [item(1, "PAUSED")] }} />);

    expect(screen.getAllByText("已暂停").length).toBe(2);
    expect(screen.queryByText("处理中")).toBeNull();
  });

  it("orders items by ordinal ascending and paginates 100 items at 50 per page", async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 100 }, (_, index) => item(100 - index));
    render(<ProductionMonitor batch={{ id: "batch-100", items, total: 100 }} />);

    let rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(PRODUCTION_MONITOR_PAGE_SIZE);
    expect(rows[0].getAttribute("data-ordinal")).toBe("1");
    expect(rows[rows.length - 1].getAttribute("data-ordinal")).toBe("50");
    expect(screen.getByText("第 1 / 2 页 · 每页 50 项")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "下一页" }));
    rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(50);
    expect(rows[0].getAttribute("data-ordinal")).toBe("51");
    expect(rows[rows.length - 1].getAttribute("data-ordinal")).toBe("100");
  });

  it("filters generating, failed and completed items and resets to page one", async () => {
    const user = userEvent.setup();
    const items = [
      ...Array.from({ length: 60 }, (_, index) => item(index + 1, "SUCCEEDED", { videoUrl: "https://example.test/a.mp4" })),
      item(61, "RUNNING"),
      item(62, "FAILED", { errorMessage: "生成超时" }),
    ];
    render(<ProductionMonitor batch={{ items, total: 62 }} />);

    await user.click(screen.getByRole("button", { name: "下一页" }));
    await user.click(screen.getByRole("button", { name: /已完成/ }));
    expect(screen.getByText("第 1 / 2 页 · 每页 50 项")).toBeTruthy();
    expect(screen.getAllByRole("listitem")).toHaveLength(50);
    expect(screen.getByRole("button", { name: /已完成/ }).getAttribute("aria-pressed")).toBe("true");

    await user.click(screen.getByRole("button", { name: /生成中/ }));
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByText("镜头 61")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /失败/ }));
    expect(screen.getByText("生成超时")).toBeTruthy();
  });

  it("shows failure details and invokes a manual retry callback", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    render(<ProductionMonitor batch={{ items: [item(7, "FAILED", { errorCode: "TIMEOUT", errorMessage: "ComfyUI 超时" })] }} onRetry={onRetry} />);

    expect(screen.getByText("错误 TIMEOUT")).toBeTruthy();
    expect(screen.getByText("ComfyUI 超时")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "重试" }));
    expect(onRetry).toHaveBeenCalledWith("item-7");
  });

  it("keeps output rows action-only, invokes play with item and asset IDs, opens file location, and explains unavailable records", async () => {
    const user = userEvent.setup();
    const onPlay = vi.fn();
    const onOpenFileLocation = vi.fn();
    render(
      <ProductionMonitor
        batch={{ items: [item(1, "SUCCEEDED", { videoUrl: "https://example.test/video.mp4", assetId: "asset-1", filePath: "D:/成果/1.mp4" }), item(2, "SUCCEEDED", { assetId: "image-2", assetType: "image", mimeType: "image/png", filePath: "D:/成果/2.png" }), item(3, "SUCCEEDED")] }}
        onPlay={onPlay}
        onOpenFileLocation={onOpenFileLocation}
      />,
    );

    expect(document.querySelector("video")).toBeNull();
    expect(document.querySelector("[preload]")).toBeNull();
    await user.click(screen.getByRole("button", { name: "播放" }));
    expect(onPlay).toHaveBeenCalledWith("item-1", "asset-1");
    const imageButtons = Array.from(document.querySelectorAll('[data-item-id="item-2"] button'));
    expect(imageButtons.some((button) => button.textContent === "播放")).toBe(false);
    const videoRow = document.querySelector('[data-item-id="item-1"]');
    expect(videoRow).toBeTruthy();
    await user.click(within(videoRow as HTMLElement).getByRole("button", { name: "打开文件位置" }));
    expect(onOpenFileLocation).toHaveBeenCalledWith("item-1", "D:/成果/1.mp4");
    expect(screen.getByText("成品记录不可用")).toBeTruthy();
  });

  it("renders completion actions and sends them to Host callbacks", async () => {
    const user = userEvent.setup();
    const callbacks = { view: vi.fn(), folder: vi.fn(), export: vi.fn(), next: vi.fn() };
    render(
      <ProductionMonitor
        batch={{ status: "COMPLETED", total: 1, items: [item(1, "SUCCEEDED", { videoUrl: "https://example.test/1.mp4" })] }}
        onViewAllProducts={callbacks.view}
        onOpenProductsFolder={callbacks.folder}
        onExportProductList={callbacks.export}
        onSelectNextProductionPackage={callbacks.next}
      />,
    );

    expect(screen.getByText("批次已完成")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "查看全部成品" }));
    await user.click(screen.getByRole("button", { name: "打开成品文件夹" }));
    await user.click(screen.getByRole("button", { name: "导出成品清单" }));
    await user.click(screen.getByRole("button", { name: "选择下一个生产包" }));
    expect(callbacks.view).toHaveBeenCalledTimes(1);
    expect(callbacks.folder).toHaveBeenCalledTimes(1);
    expect(callbacks.export).toHaveBeenCalledTimes(1);
    expect(callbacks.next).toHaveBeenCalledTimes(1);
  });

  it("switches to completed items before optionally notifying the Host", async () => {
    const user = userEvent.setup();
    render(<ProductionMonitor batch={{ status: "COMPLETED", items: [item(1, "SUCCEEDED", { assetId: "asset-1", assetType: "video" }), item(2, "FAILED")] }} />);

    const viewAll = screen.getByRole("button", { name: "查看全部成品" });
    expect((viewAll as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByRole("button", { name: "打开成品文件夹" })).toBeNull();
    expect(screen.queryByRole("button", { name: "导出成品清单" })).toBeNull();
    expect(screen.queryByRole("button", { name: "选择下一个生产包" })).toBeNull();
    await user.click(viewAll);

    expect(screen.getByRole("button", { name: /已完成/ }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByText("镜头 1")).toBeTruthy();
    expect(screen.queryByText("镜头 2")).toBeNull();
  });

  it("keeps a 500-item batch to ten pages and reaches the final ordered page", async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 500 }, (_, index) => item(500 - index));
    render(<ProductionMonitor batch={{ items, total: 500 }} />);

    expect(screen.getAllByRole("listitem")).toHaveLength(50);
    expect(screen.getByText("第 1 / 10 页 · 每页 50 项")).toBeTruthy();
    const next = screen.getByRole("button", { name: "下一页" });
    for (let page = 1; page < 10; page += 1) await user.click(next);

    const rows = screen.getAllByRole("listitem");
    expect(screen.getByText("第 10 / 10 页 · 每页 50 项")).toBeTruthy();
    expect(rows[0].getAttribute("data-ordinal")).toBe("451");
    expect(rows[rows.length - 1].getAttribute("data-ordinal")).toBe("500");
    expect((next as HTMLButtonElement).disabled).toBe(true);
  });
});
