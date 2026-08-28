// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProductionPackageInspection } from "../../types/productionPackage";
import { ProductionPackagePreview, truncateProductionPackagePrompt } from "./ProductionPackagePreview";

const inspection: ProductionPackageInspection = {
  packageName: "EP01 · 雨夜",
  itemCount: 3,
  readyCount: 1,
  warningCount: 1,
  blockedCount: 1,
  items: [
    {
      id: "EP01-SC01-SH001",
      name: "巷口回望",
      mode: "FL2VA_IMAGE_TO_VIDEO",
      videoPromptPreview: "镜头缓慢推进，雨水沿着屋檐落下。",
      firstFrame: { relativePath: "images/SH001.png", width: 864, height: 480 },
      references: [{ relativePath: "references/hero.png" }],
      duration: 5,
      resolution: { width: 864, height: 480 },
      status: "READY",
      warnings: [],
      errors: [],
    },
    {
      id: "EP01-SC01-SH002",
      name: "火光切入",
      mode: "FL2VA_FIRST_LAST",
      videoPromptPreview: "火光从画面左侧掠过。",
      firstFrame: "images/SH002-first.png",
      lastFrame: "images/SH002-last.png",
      duration: 8,
      resolution: "1280 × 720",
      status: "WARNING",
      warnings: [{ code: "PACKAGE_MODE_ALIAS", message: "模式使用了兼容别名" }],
      errors: [],
    },
    {
      id: "EP01-SC01-SH003",
      name: "空镜",
      mode: "TEXT_ONLY",
      videoPromptPreview: "",
      durationSeconds: 5,
      width: 864,
      height: 480,
      status: "BLOCKED",
      warnings: [],
      errors: ["PACKAGE_MEDIA_MISSING"],
    },
  ],
};

afterEach(cleanup);

describe("ProductionPackagePreview", () => {
  it("renders package identity, stats, all required columns, media metadata, and a bounded prompt preview", () => {
    const longPrompt = "镜".repeat(301);
    const { rerender } = render(
      <ProductionPackagePreview inspection={{ ...inspection, items: [{ ...inspection.items[0], videoPromptPreview: longPrompt }, ...inspection.items.slice(1)] }} />,
    );
    const table = screen.getByRole("table", { name: "生产包项目列表" });

    expect(screen.getByRole("heading", { name: "EP01 · 雨夜" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "生产包预览" }).textContent).toContain("READY");
    for (const heading of ["选择", "状态", "ID", "名称", "模式", "图片", "首帧", "末帧", "参考图数量", "Video Prompt", "时长", "分辨率", "错误"]) {
      expect(within(table).getByRole("columnheader", { name: heading })).toBeTruthy();
    }
    expect(screen.getByText(/images\/SH001\.png/)).toBeTruthy();
    expect(screen.getByText(/末帧：images\/SH002-last\.png/)).toBeTruthy();
    expect(screen.getByText(/参考图：1 张/)).toBeTruthy();
    expect(screen.getByText(`${"镜".repeat(300)}…`)).toBeTruthy();
    expect(screen.queryByText("镜".repeat(301))).toBeNull();
    expect(screen.getByText("PACKAGE_MEDIA_MISSING")).toBeTruthy();

    rerender(<ProductionPackagePreview />);
    expect(screen.getByRole("status").textContent).toContain("尚未加载");
  });

  it("filters rows through real status buttons without changing the inspection data", async () => {
    const user = userEvent.setup();
    render(<ProductionPackagePreview inspection={inspection} />);
    const table = screen.getByRole("table", { name: "生产包项目列表" });

    expect(within(table).getAllByRole("row")).toHaveLength(4);
    await user.click(screen.getByRole("button", { name: /BLOCKED 1/ }));

    expect(within(table).getAllByRole("row")).toHaveLength(2);
    expect(within(table).getByText("EP01-SC01-SH003")).toBeTruthy();
    expect(within(table).queryByText("EP01-SC01-SH001")).toBeNull();
    expect(screen.getByRole("button", { name: /BLOCKED 1/ }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText(/显示\s*1\s*\/\s*3\s*个项目/)).toBeTruthy();
  });

  it("selects an item through an accessible item control and reports the original DTO", async () => {
    const user = userEvent.setup();
    const onSelectItem = vi.fn();
    render(<ProductionPackagePreview inspection={inspection} onSelectItem={onSelectItem} />);

    await user.click(screen.getByRole("button", { name: "选择项目 EP01-SC01-SH002" }));

    expect(onSelectItem).toHaveBeenCalledWith(inspection.items[1]);
    expect(screen.getByRole("button", { name: "选择项目 EP01-SC01-SH002" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("pages 500 items, keeps Set selection across pages and filters, and handles READY/WARNING/BLOCKED", async () => {
    const user = userEvent.setup();
    render(<ProductionPackagePreview inspection={makeInspection(500)} />);

    const region = screen.getByRole("region", { name: "生产包预览" });
    const table = screen.getByRole("table", { name: "生产包项目列表" });
    expect(within(table).getAllByRole("row")).toHaveLength(51);
    expect(screen.getByText(/每页 50/)).toBeTruthy();

    const ready = within(table).getByRole("checkbox", { name: "选择项目 item-001" }) as HTMLInputElement;
    const warning = within(table).getByRole("checkbox", { name: "选择项目 item-002" }) as HTMLInputElement;
    const blocked = within(table).getByRole("checkbox", { name: "选择项目 item-003" }) as HTMLInputElement;
    expect(ready.checked).toBe(true);
    expect(warning.checked).toBe(false);
    expect(blocked.disabled).toBe(true);
    expect(blocked.getAttribute("aria-description")).toContain("BLOCKED");
    expect(blocked.getAttribute("aria-describedby")).toBeTruthy();

    await user.click(warning);
    expect(warning.checked).toBe(true);
    expect(region.getAttribute("data-selected-count")).toBe("401");

    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getByLabelText("第 2 / 10 页")).toBeTruthy();
    const secondPageWarning = screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement;
    expect(secondPageWarning.checked).toBe(false);
    await user.click(secondPageWarning);
    expect(secondPageWarning.checked).toBe(true);

    await user.click(screen.getByRole("button", { name: "上一页" }));
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement).checked).toBe(true);

    await user.click(screen.getByRole("button", { name: /WARNING 50/ }));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText(/已选择 402 项（/)).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "清空选择" }));
    expect(region.getAttribute("data-selected-count")).toBe("0");
    await user.click(screen.getByRole("button", { name: "全选 READY" }));
    expect(region.getAttribute("data-selected-count")).toBe("400");
  });

  it("offers 50 or 100 rows per page without rendering the full inspection", async () => {
    const user = userEvent.setup();
    render(<ProductionPackagePreview inspection={makeInspection(500)} />);

    const table = screen.getByRole("table", { name: "生产包项目列表" });
    const pageSize = screen.getByRole("combobox", { name: "每页显示项目数" });
    expect((pageSize as HTMLSelectElement).value).toBe("50");
    await user.selectOptions(pageSize, "100");

    expect(within(table).getAllByRole("row")).toHaveLength(101);
    expect(screen.getByLabelText("第 1 / 5 页")).toBeTruthy();
    expect(within(table).queryByText("item-500")).toBeNull();
  });

  it("supports controlled selectedItemIds and reports a fresh Set", async () => {
    const user = userEvent.setup();
    const onSelectionChange = vi.fn();
    const selectedItemIds = new Set([inspection.items[0].id]);
    const { rerender } = render(
      <ProductionPackagePreview
        inspection={inspection}
        selectedItemIds={selectedItemIds}
        onSelectionChange={onSelectionChange}
      />,
    );

    expect((screen.getByRole("checkbox", { name: "选择项目 EP01-SC01-SH001" }) as HTMLInputElement).checked).toBe(true);
    await user.click(screen.getByRole("checkbox", { name: "选择项目 EP01-SC01-SH002" }));

    const nextSelection = onSelectionChange.mock.calls[0]?.[0] as Set<string>;
    expect(nextSelection).toEqual(new Set([inspection.items[0].id, inspection.items[1].id]));
    rerender(
      <ProductionPackagePreview
        inspection={inspection}
        selectedItemIds={nextSelection}
        onSelectionChange={onSelectionChange}
      />,
    );
    expect((screen.getByRole("checkbox", { name: "选择项目 EP01-SC01-SH002" }) as HTMLInputElement).checked).toBe(true);
  });

  it("truncates by Unicode characters and keeps short prompts unchanged", () => {
    expect(truncateProductionPackagePrompt("  简短提示词  ")).toBe("简短提示词");
    expect(truncateProductionPackagePrompt("a".repeat(300))).toBe("a".repeat(300));
    expect(truncateProductionPackagePrompt("a".repeat(301))).toBe(`${"a".repeat(300)}…`);
    expect(truncateProductionPackagePrompt("😀".repeat(301))).toBe(`${"😀".repeat(300)}…`);
  });
});

function makeInspection(count: number): ProductionPackageInspection {
  const items = Array.from({ length: count }, (_, index) => {
    const status = index % 10 === 1 ? "WARNING" : index % 10 === 2 ? "BLOCKED" : "READY";
    return {
      id: `item-${String(index + 1).padStart(3, "0")}`,
      name: `镜头 ${index + 1}`,
      mode: "FL2VA_IMAGE_TO_VIDEO",
      videoPromptPreview: "镜头缓慢推进。",
      duration: 5,
      resolution: { width: 864, height: 480 },
      status,
      warnings: status === "WARNING" ? [{ code: "PACKAGE_MODE_ALIAS", message: "使用兼容模式别名" }] : [],
      errors: status === "BLOCKED" ? [{ code: "PACKAGE_MEDIA_MISSING", message: "媒体文件缺失" }] : [],
    };
  });
  return {
    packageName: "EP01 · 雨夜",
    itemCount: items.length,
    readyCount: items.filter((item) => item.status === "READY").length,
    warningCount: items.filter((item) => item.status === "WARNING").length,
    blockedCount: items.filter((item) => item.status === "BLOCKED").length,
    items,
  };
}
