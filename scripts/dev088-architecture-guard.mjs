import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(import.meta.url), "..", "..");
const ipcTransportPath = resolve(root, "src/services/ipc.ts");
const productionFrontendFiles = [];

function collectSourceFiles(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collectSourceFiles(path);
    else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\./.test(entry.name)) productionFrontendFiles.push(path);
  }
}

collectSourceFiles(join(root, "src"));

const rawInvokeImports = productionFrontendFiles.filter((path) => {
  const source = readFileSync(path, "utf8");
  return source.includes("@tauri-apps/api/core") && resolve(path) !== ipcTransportPath;
});
if (rawInvokeImports.length) {
  throw new Error(`FRONTEND_NO_RAW_INVOKE failed:\n${rawInvokeImports.join("\n")}`);
}

const transportSources = [
  readFileSync(join(root, "src/services/tauriClient.ts"), "utf8"),
  readFileSync(join(root, "src/services/workflowClient.ts"), "utf8"),
  readFileSync(join(root, "src/features/shots/ShotBulkImportPanel.tsx"), "utf8"),
].join("\n");
const frontendCommands = new Set(
  [...transportSources.matchAll(/\binvoke(?:Command)?\s*(?:<[\s\S]*?>)?\s*\(\s*["']([^"']+)["']/g)].map((match) => match[1]),
);

const libSource = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const handlerBlock = libSource.match(/generate_handler!\[([\s\S]*?)\]\)/)?.[1] ?? "";
const backendCommands = new Set(
  [...handlerBlock.matchAll(/commands::(?:[A-Za-z0-9_]+::)?([A-Za-z0-9_]+)/g)].map((match) => match[1]),
);
const missingCommands = [...frontendCommands].filter((command) => !backendCommands.has(command));
if (missingCommands.length) {
  throw new Error(`RPC_PARITY failed:\n${missingCommands.join("\n")}`);
}

const shotWorkspaceSource = readFileSync(join(root, "src/features/shots/ShotWorkspace.tsx"), "utf8");
if (shotWorkspaceSource.includes("subscribeTaskUpdates")) {
  throw new Error("SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION failed: ShotWorkspace must delegate task events to useShotTaskEvents");
}
if (!shotWorkspaceSource.includes("useShotQueueController")) {
  throw new Error("SHOT_WORKSPACE_QUEUE_CONTROLLER failed: ShotWorkspace must delegate queue ownership to useShotQueueController");
}
if (!shotWorkspaceSource.includes("useShotProductionMonitor")) {
  throw new Error("SHOT_WORKSPACE_MONITOR_CONTROLLER failed: ShotWorkspace must delegate monitor ownership to useShotProductionMonitor");
}
if (["productionMonitorRequest", "productionMonitorPendingBatch", "productionMonitorMounted", "productionMonitorBatchRef", "window.setInterval(refreshIfVisible, 3000)", "visibilitychange"].some((marker) => shotWorkspaceSource.includes(marker))) {
  throw new Error("SHOT_WORKSPACE_MONITOR_CONTROLLER failed: ShotWorkspace must not own monitor polling or request refs");
}

console.log(`FRONTEND_NO_RAW_INVOKE=PASS`);
console.log(`RAW_INVOKE_OUTSIDE_TRANSPORT=0`);
console.log(`RPC_PARITY=PASS (${frontendCommands.size} frontend commands checked)`);
console.log(`SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS`);
console.log(`SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS`);
console.log(`SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS`);
