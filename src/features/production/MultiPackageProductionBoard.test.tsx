// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MultiPackageBoardPackage } from "./MultiPackageProductionBoard";
import { MultiPackageProductionBoard } from "./MultiPackageProductionBoard";

afterEach(cleanup);

const pkg = (overrides: Partial<MultiPackageBoardPackage> = {}): MultiPackageBoardPackage => {
  const status = overrides.status ?? "READY";
  return {
    packageKey: "ep01",
    packageRoot: "D:/season/ep01",
    relativePath: "ep01",
    packageName: "第 01 集",
    itemCount: 10,
    status,
    canCreate: status === "READY",
    readyCount: 10,
    ...overrides,
  };
};

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

  it("selects READY by default and disables WARNING, blocked, and created rows", () => {
    render(<MultiPackageProductionBoard packages={[
      pkg(),
      pkg({ packageKey: "warning", packageName: "警告包", status: "WARNING" }),
      pkg({ packageKey: "blocked", packageName: "阻塞包", status: "BLOCKED" }),
      pkg({ packageKey: "created", packageName: "已创建包", status: "CREATED" }),
    ]} />);
    const readyCheckbox = screen.getByRole("checkbox", { name: "选择生产包 第 01 集" }) as HTMLInputElement;
    const warningCheckbox = screen.getByRole("checkbox", { name: "选择生产包 警告包" }) as HTMLInputElement;
    const blockedCheckbox = screen.getByRole("checkbox", { name: "选择生产包 阻塞包" }) as HTMLInputElement;
    const createdCheckbox = screen.getByRole("checkbox", { name: "选择生产包 已创建包" }) as HTMLInputElement;
    expect(readyCheckbox.checked).toBe(true);
    expect(readyCheckbox.disabled).toBe(false);
    expect(warningCheckbox.checked).toBe(false);
    expect(warningCheckbox.disabled).toBe(true);
    expect(blockedCheckbox.checked).toBe(false);
    expect(blockedCheckbox.disabled).toBe(true);
    expect(createdCheckbox.checked).toBe(false);
    expect(createdCheckbox.disabled).toBe(true);
    expect(screen.getByText("请进入单生产包确认警告镜头。")).toBeTruthy();
  });

  it("requires an explicit canCreate flag instead of inferring eligibility from READY", () => {
    render(<MultiPackageProductionBoard packages={[pkg({ canCreate: false })]} onCreateSelected={vi.fn()} />);
    const checkbox = screen.getByRole("checkbox", { name: "选择生产包 第 01 集" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(true);
    expect((screen.getByRole("button", { name: /创建所选生产批次/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows a resumable PARTIAL package with remaining READY items", () => {
    render(<MultiPackageProductionBoard packages={[pkg({
      packageKey: "partial-ready",
      packageName: "部分已创建包",
      status: "PARTIAL",
      itemCount: 150,
      boundItemCount: 100,
      remainingCount: 50,
      remainingReadyCount: 50,
      remainingWarningCount: 0,
      remainingBlockedCount: 0,
      batchIds: ["batch-existing"],
      canCreate: true,
    })]} onCreateSelected={vi.fn()} />);
    const row = screen.getByRole("row", { name: /部分已创建包/ });
    const checkbox = within(row).getByRole("checkbox") as HTMLInputElement;
    expect(screen.getByText("部分已创建")).toBeTruthy();
    expect(screen.getByText("已创建 100")).toBeTruthy();
    expect(row.textContent).toContain("剩余 50");
    expect(screen.getByText(/批次 batch-existing/)).toBeTruthy();
    expect(checkbox.checked).toBe(true);
    expect(checkbox.disabled).toBe(false);
  });

  it("blocks PARTIAL bulk creation when remaining items include WARNING", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn();
    const onHandleWarning = vi.fn();
    render(<MultiPackageProductionBoard packages={[pkg({
      packageKey: "partial-warning",
      packageName: "待人工确认的部分包",
      status: "PARTIAL",
      itemCount: 150,
      boundItemCount: 100,
      remainingCount: 50,
      remainingReadyCount: 40,
      remainingWarningCount: 10,
      remainingBlockedCount: 0,
      batchIds: ["batch-existing"],
      canCreate: false,
    })]} onCreateSelected={onCreateSelected} onHandleWarning={onHandleWarning} />);
    const row = screen.getByRole("row", { name: /待人工确认的部分包/ });
    const checkbox = within(row).getByRole("checkbox") as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(true);
    expect(screen.getByText(/40 可创建/)).toBeTruthy();
    expect(screen.getByText(/10 需要人工确认/)).toBeTruthy();
    expect((screen.getByRole("button", { name: /创建所选生产批次/ }) as HTMLButtonElement).disabled).toBe(true);
    await user.click(within(row).getByRole("button", { name: "在单生产包中处理" }));
    expect(onHandleWarning).toHaveBeenCalledWith("partial-warning");
    expect(onCreateSelected).not.toHaveBeenCalled();
  });

  it("keeps packageKey as the durable identity when display paths use different representations", async () => {
    const user = userEvent.setup();
    const onOpenBatch = vi.fn();
    render(<MultiPackageProductionBoard packages={[pkg({
      packageKey: "abc",
      packageRoot: "D:\\Season\\EP01",
      relativePath: "\\\\?\\D:\\Season\\EP01",
      packageName: "第 01 集",
      status: "CREATED",
      itemCount: 150,
      boundItemCount: 150,
      remainingCount: 0,
      batchIds: ["batch-abc"],
      canCreate: false,
    })]} onOpenBatch={onOpenBatch} />);
    const row = screen.getByRole("row", { name: /第 01 集/ });
    expect(row.getAttribute("data-package-key")).toBe("abc");
    await user.click(within(row).getByRole("button", { name: "打开生产批次" }));
    expect(onOpenBatch).toHaveBeenCalledWith("abc", ["batch-abc"]);
  });

  it("keeps CREATE_FAILED disabled until the package is re-inspected", async () => {
    const user = userEvent.setup();
    const onReinspect = vi.fn();
    render(<MultiPackageProductionBoard packages={[pkg({
      packageKey: "failed",
      packageName: "创建失败包",
      status: "CREATE_FAILED",
      canCreate: false,
      firstError: "上次创建失败",
    })]} onReinspect={onReinspect} />);
    const row = screen.getByRole("row", { name: /创建失败包/ });
    expect((within(row).getByRole("checkbox") as HTMLInputElement).disabled).toBe(true);
    await user.click(within(row).getByRole("button", { name: "重新检查" }));
    expect(onReinspect).toHaveBeenCalledWith("failed");
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
      pkg({ packageKey: "A", packageName: "A", relativePath: "A", itemCount: 4 }),
      pkg({ packageKey: "B", packageName: "B", relativePath: "B", itemCount: 6 }),
      pkg({ packageKey: "C", packageName: "C", relativePath: "C", status: "WARNING", itemCount: 3 }),
    ]} onCreateSelected={onCreateSelected} />);
    const button = screen.getByRole("button", { name: "创建所选生产批次（2 个生产包 · 10 个镜头）" });
    expect((button as HTMLButtonElement).disabled).toBe(false);
    await user.click(button);
    await vi.waitFor(() => expect(onCreateSelected).toHaveBeenCalledTimes(1));
    expect(onCreateSelected).toHaveBeenCalledWith(["A", "B"]);
    expect(screen.queryByText("开始生成")).toBeNull();
  });

  it("does not mark deferred packages as created when bulk creation stops on PARTIAL", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockResolvedValue(undefined);
    const initialPackages = [
      pkg({ packageKey: "EP01", packageName: "EP01", relativePath: "EP01" }),
      pkg({ packageKey: "EP02", packageName: "EP02", relativePath: "EP02" }),
      pkg({ packageKey: "EP03", packageName: "EP03", relativePath: "EP03" }),
    ];
    const { rerender } = render(<MultiPackageProductionBoard packages={initialPackages} onCreateSelected={onCreateSelected} />);

    await user.click(screen.getByRole("button", { name: "创建所选生产批次（3 个生产包 · 30 个镜头）" }));
    await vi.waitFor(() => expect(onCreateSelected).toHaveBeenCalledWith(["EP01", "EP02", "EP03"]));

    expect(within(screen.getByRole("row", { name: /EP01/ })).getByText("可创建")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP02/ })).getByText("可创建")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP03/ })).getByText("可创建")).toBeTruthy();
    expect((within(screen.getByRole("row", { name: /EP02/ })).getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
    expect((within(screen.getByRole("row", { name: /EP03/ })).getByRole("checkbox") as HTMLInputElement).checked).toBe(true);

    rerender(<MultiPackageProductionBoard packages={[
      pkg({ packageKey: "EP01", packageName: "EP01", relativePath: "EP01", status: "PARTIAL", itemCount: 10, boundItemCount: 5, remainingCount: 5, remainingReadyCount: 5, canCreate: true, batchIds: ["batch-ep01"] }),
      pkg({ packageKey: "EP02", packageName: "EP02", relativePath: "EP02", status: "NOT_CREATED", canCreate: true }),
      pkg({ packageKey: "EP03", packageName: "EP03", relativePath: "EP03", status: "NOT_CREATED", canCreate: true }),
    ]} onCreateSelected={onCreateSelected} />);

    expect(within(screen.getByRole("row", { name: /EP01/ })).getByText("部分已创建")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP02/ })).getByText("未创建")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP03/ })).getByText("未创建")).toBeTruthy();
    expect(screen.queryByText("已创建", { selector: ".multi-package-production-board-status" })).toBeNull();
  });

  it("renders Host success, failure, and deferred statuses after an error stop", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockRejectedValue(new Error("EP02 创建失败"));
    const initialPackages = [
      pkg({ packageKey: "EP01", packageName: "EP01", relativePath: "EP01" }),
      pkg({ packageKey: "EP02", packageName: "EP02", relativePath: "EP02" }),
      pkg({ packageKey: "EP03", packageName: "EP03", relativePath: "EP03" }),
    ];
    const { rerender } = render(<MultiPackageProductionBoard packages={initialPackages} onCreateSelected={onCreateSelected} />);

    await user.click(screen.getByRole("button", { name: "创建所选生产批次（3 个生产包 · 30 个镜头）" }));
    await vi.waitFor(() => expect(screen.getByRole("alert").textContent).toContain("EP02 创建失败"));
    expect(screen.getAllByText("可创建")).toHaveLength(3);

    rerender(<MultiPackageProductionBoard packages={[
      pkg({ packageKey: "EP01", packageName: "EP01", relativePath: "EP01", status: "COMPLETED", itemCount: 10, boundItemCount: 10, remainingCount: 0, canCreate: false }),
      pkg({ packageKey: "EP02", packageName: "EP02", relativePath: "EP02", status: "CREATE_FAILED", canCreate: false, firstError: "EP02 创建失败" }),
      pkg({ packageKey: "EP03", packageName: "EP03", relativePath: "EP03", status: "NOT_CREATED", canCreate: true }),
    ]} onCreateSelected={onCreateSelected} />);

    expect(within(screen.getByRole("row", { name: /EP01/ })).getByText("已完成")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP02/ })).getByText("创建失败")).toBeTruthy();
    expect(within(screen.getByRole("row", { name: /EP03/ })).getByText("未创建")).toBeTruthy();
    expect((within(screen.getByRole("row", { name: /EP03/ })).getByRole("checkbox") as HTMLInputElement).checked).toBe(true);
  });

  it("returns to the Host status immediately after a successful create resolves", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockResolvedValue(undefined);
    const readyPackage = pkg({ packageKey: "EP01", packageName: "EP01", relativePath: "EP01" });
    const { rerender } = render(<MultiPackageProductionBoard packages={[readyPackage]} onCreateSelected={onCreateSelected} />);

    await user.click(screen.getByRole("button", { name: /创建所选生产批次/ }));
    await vi.waitFor(() => expect(onCreateSelected).toHaveBeenCalledTimes(1));
    expect(within(screen.getByRole("row", { name: /EP01/ })).getByText("可创建")).toBeTruthy();

    rerender(<MultiPackageProductionBoard packages={[pkg({
      packageKey: "EP01",
      packageName: "EP01",
      relativePath: "EP01",
      status: "COMPLETED",
      boundItemCount: 10,
      remainingCount: 0,
      canCreate: false,
    })]} onCreateSelected={onCreateSelected} />);
    expect(within(screen.getByRole("row", { name: /EP01/ })).getByText("已完成")).toBeTruthy();
  });

  it("does not create an externally selected WARNING package", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockResolvedValue(undefined);
    render(<MultiPackageProductionBoard packages={[
      pkg({ packageKey: "A", packageName: "A", relativePath: "A", itemCount: 4 }),
      pkg({ packageKey: "B", packageName: "B", relativePath: "B", itemCount: 6 }),
      pkg({ packageKey: "C", packageName: "C", relativePath: "C", status: "WARNING", itemCount: 3 }),
    ]} selectedPackageKeys={["A", "B", "C"]} onCreateSelected={onCreateSelected} />);
    expect((screen.getByRole("checkbox", { name: "选择生产包 C" }) as HTMLInputElement).checked).toBe(false);
    const button = screen.getByRole("button", { name: "创建所选生产批次（2 个生产包 · 10 个镜头）" });
    await user.click(button);
    await vi.waitFor(() => expect(onCreateSelected).toHaveBeenCalledTimes(1));
    expect(onCreateSelected).toHaveBeenCalledWith(["A", "B"]);
  });

  it("keeps the WARNING single-package handling entry", async () => {
    const user = userEvent.setup();
    const onHandleWarning = vi.fn();
    render(<MultiPackageProductionBoard packages={[pkg({ packageKey: "C", packageName: "C", relativePath: "C", status: "WARNING", itemCount: 3 })]} onHandleWarning={onHandleWarning} />);
    await user.click(screen.getByRole("button", { name: "在单生产包中处理" }));
    expect(onHandleWarning).toHaveBeenCalledWith("C");
  });

  it("shows creating and a top-level error after onCreateSelected rejects without package-level failure truth", async () => {
    const user = userEvent.setup();
    const onCreateSelected = vi.fn().mockRejectedValue(new Error("父层创建失败"));
    render(<MultiPackageProductionBoard packages={[pkg()]} onCreateSelected={onCreateSelected} />);
    await user.click(screen.getByRole("button", { name: /创建所选生产批次/ }));
    await vi.waitFor(() => expect(screen.getByRole("alert").textContent).toContain("父层创建失败"));
    expect(within(screen.getByRole("row", { name: /第 01 集/ })).getByText("可创建")).toBeTruthy();
    expect(screen.queryByText("创建失败", { selector: ".multi-package-production-board-status" })).toBeNull();
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
