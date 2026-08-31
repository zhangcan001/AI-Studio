// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MultiPackageBoardPackage } from "./MultiPackageProductionBoard";
import { MultiPackageProductionBoard } from "./MultiPackageProductionBoard";

afterEach(cleanup);

const pkg = (overrides: Partial<MultiPackageBoardPackage> = {}): MultiPackageBoardPackage => ({
  packageKey: "ep01",
  packageRoot: "D:/season/ep01",
  relativePath: "ep01",
  packageName: "第 01 集",
  itemCount: 10,
  status: "READY",
  readyCount: 10,
  ...overrides,
});

const fixtures = [
  pkg(),
  pkg({ packageKey: "ep02", packageName: "第 02 集", relativePath: "ep02", status: "WARNING", itemCount: 12, issueSummary: "有 2 个镜头需要确认" }),
  pkg({ packageKey: "ep03", packageName: "第 03 集", relativePath: "ep03", status: "BLOCKED", itemCount: 8, issueSummary: "缺少必要素材" }),
  pkg({ packageKey: "ep04", packageName: "第 04 集", relativePath: "ep04", status: "RUNNING", itemCount: 20, batchIds: ["batch-04"], succeeded: 6, running: 2, pending: 12 }),
  pkg({ packageKey: "ep05", packageName: "第 05 集", relativePath: "ep05", status: "COMPLETED", itemCount: 15, batchIds: ["batch-05"], succeeded: 15, pending: 0 }),
];

describe("MultiPackageProductionBoard", () => {
  it("renders empty state, manual-start warning, and root chooser callback", async () => {
    const user = userEvent.setup();
    const onChooseRoot = vi.fn();
    render(<MultiPackageProductionBoard onChooseRoot={onChooseRoot} />);
    expect(screen.getByRole("heading", { name: "批量生产包" })).toBeTruthy();
    expect(screen.getAllByText(/不会自动开始生成/).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "选择根目录" }));
    expect(onChooseRoot).toHaveBeenCalledTimes(1);
  });

  it("renders five package rows and summary metrics", () => {
    render(<MultiPackageProductionBoard packages={fixtures} />);
    expect(screen.getByRole("table", { name: "批量生产包列表" })).toBeTruthy();
    const summary = screen.getByLabelText("批量生产摘要");
    expect(within(summary).getByText("5")).toBeTruthy();
    expect(screen.getByText("65")).toBeTruthy();
    for (const [label, value] of [["READY · 可创建", "1"], ["WARNING · 警告", "1"], ["BLOCKED · 阻塞", "1"]]) {
      const card = screen.getByText(label).parentElement;
      expect(card).toBeTruthy();
      expect(within(card as HTMLElement).getByText(value)).toBeTruthy();
    }
    for (const item of fixtures) expect(screen.getByText(item.relativePath)).toBeTruthy();
    expect(screen.getAllByText("运行中").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已完成").length).toBeGreaterThan(0);
  });

  it("selects READY by default, leaves WARNING unchecked, and disables blocked and created rows", () => {
    render(<MultiPackageProductionBoard packages={[
      pkg(),
      pkg({ packageKey: "warning", packageName: "警告包", status: "WARNING" }),
      pkg({ packageKey: "blocked", packageName: "阻塞包", status: "BLOCKED" }),
      pkg({ packageKey: "created", packageName: "已创建包", status: "CREATED" }),
    ]} />);
    expect((screen.getByRole("checkbox", { name: "选择生产包 第 01 集" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择生产包 警告包" }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole("checkbox", { name: "选择生产包 阻塞包" }) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择生产包 已创建包" }) as HTMLInputElement).disabled).toBe(true);
  });

  it("filters all, issues, and running packages", async () => {
    const user = userEvent.setup();
    render(<MultiPackageProductionBoard packages={fixtures} />);
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(6);
    await user.click(screen.getByRole("button", { name: /^问题/ }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(3);
    expect(screen.getByText("第 02 集")).toBeTruthy();
    expect(screen.getByText("第 03 集")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /^运行中/ }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(2);
    expect(screen.getByText("第 04 集")).toBeTruthy();
  });

  it("gates more than 100 packages and more than 10000 selected items", () => {
    const tooManyPackages = Array.from({ length: 101 }, (_, index) => pkg({ packageKey: `p-${index}`, packageName: `包 ${index}`, itemCount: 1 }));
    const { rerender } = render(<MultiPackageProductionBoard packages={tooManyPackages} onCreateSelected={vi.fn()} />);
    expect((screen.getByRole("button", { name: /创建所选生产批次/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("最多 100 个");
    rerender(<MultiPackageProductionBoard packages={[pkg({ itemCount: 10_001 })]} onCreateSelected={vi.fn()} />);
    expect((screen.getByRole("button", { name: /创建所选生产批次/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("最多 10000 个");
  });

  it("shows exact selection counts and sends ordered keys once", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockResolvedValue(undefined);
    render(<MultiPackageProductionBoard packages={[
      pkg({ packageKey: "first", packageName: "第一包", itemCount: 4 }),
      pkg({ packageKey: "second", packageName: "第二包", itemCount: 6 }),
      pkg({ packageKey: "warning", packageName: "警告包", status: "WARNING", itemCount: 3 }),
    ]} onCreateSelected={onCreateSelected} />);
    await user.click(screen.getByRole("checkbox", { name: "选择生产包 警告包" }));
    const button = screen.getByRole("button", { name: "创建所选生产批次（3 个生产包 · 13 个镜头）" });
    expect((button as HTMLButtonElement).disabled).toBe(false);
    await user.click(button);
    await vi.waitFor(() => expect(onCreateSelected).toHaveBeenCalledTimes(1));
    expect(onCreateSelected).toHaveBeenCalledWith(["first", "second", "warning"]);
    expect(screen.queryByText("开始生成")).toBeNull();
  });

  it("shows creating and failure state after onCreateSelected rejects without retrying implicitly", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockRejectedValue(new Error("父层创建失败"));
    render(<MultiPackageProductionBoard packages={[pkg()]} onCreateSelected={onCreateSelected} />);
    await user.click(screen.getByRole("button", { name: /创建所选生产批次/ }));
    await vi.waitFor(() => expect(screen.getByText("创建失败")).toBeTruthy());
    expect(screen.getByRole("alert").textContent).toContain("父层创建失败");
    expect(screen.getByRole("alert").textContent).toContain("失败后可从未创建 / 剩余项继续");
    expect(onCreateSelected).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("开始生成")).toBeNull();
  });

  it("reports discovery progress accessibly and exposes blocked-row actions", async () => {
    const user = userEvent.setup();
    const onViewIssues = vi.fn();
    const onReinspect = vi.fn();
    render(<MultiPackageProductionBoard isDiscovering inspectProgress={{ current: 2, total: 5, currentPackage: "ep03", readyCount: 1, warningCount: 1, blockedCount: 0 }} packages={[pkg({ status: "BLOCKED", issueSummary: "缺少素材" })]} onViewIssues={onViewIssues} onReinspect={onReinspect} />);
    expect(screen.getByRole("status").textContent).toContain("2 / 5");
    await user.click(screen.getByRole("button", { name: "查看问题" }));
    await user.click(screen.getByRole("button", { name: "重新检查" }));
    expect(onViewIssues).toHaveBeenCalledWith("ep01");
    expect(onReinspect).toHaveBeenCalledWith("ep01");
  });
});
