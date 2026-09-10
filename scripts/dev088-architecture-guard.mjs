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
if (!shotWorkspaceSource.includes("useShotMultiPackageController")) {
  throw new Error("SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER failed: ShotWorkspace must delegate multi-package ownership to useShotMultiPackageController");
}
if (["multiPackageRunId", "multiPackageRefreshInFlight", "multiPackageRefreshPending", "multiPackageMounted"].some((marker) => shotWorkspaceSource.includes(marker))) {
  throw new Error("SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER failed: ShotWorkspace must not own multi-package lifecycle refs");
}

const assetVideoBatchWorkspaceSource = readFileSync(join(root, "src/features/assets/AssetVideoBatchWorkspace.tsx"), "utf8");
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoWorkflowController")) {
  throw new Error("ASSET_VIDEO_WORKFLOW_CONTROLLER failed: AssetVideoBatchWorkspace must delegate workflow resolution ownership to useAssetVideoWorkflowController");
}
const assetVideoWorkflowStateLines = assetVideoBatchWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && ["manualVideoSelection", "projectWorkflowConfig", "projectWorkflowStrategy", "projectManualOverrides"].some((marker) => line.includes(marker)));
if (assetVideoWorkflowStateLines.length) {
  throw new Error("ASSET_VIDEO_WORKFLOW_CONTROLLER failed: AssetVideoBatchWorkspace must not own workflow resolution state");
}
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoLibraryController")) {
  throw new Error("ASSET_VIDEO_LIBRARY_CONTROLLER failed: AssetVideoBatchWorkspace must delegate asset library query ownership to useAssetVideoLibraryController");
}
if (assetVideoBatchWorkspaceSource.includes("assetLibraryRequestVersion") || assetVideoBatchWorkspaceSource.includes("assetLibraryPage(")) {
  throw new Error("ASSET_VIDEO_LIBRARY_CONTROLLER failed: AssetVideoBatchWorkspace must not own asset library request lifecycle or call assetLibraryPage directly");
}
if (!assetVideoBatchWorkspaceSource.includes("useAssetVideoLocalImportController")) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must delegate local import session ownership to useAssetVideoLocalImportController");
}
const localImportStateMarkers = ["localInspection", "projectSegmentForms", "localBatchName", "localAutoStart", "expandedLocalOrdinal"];
const localImportStateLines = assetVideoBatchWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && localImportStateMarkers.some((marker) => line.includes(marker)));
if (localImportStateLines.length) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must not own local import session state");
}
if (["pickH3LocalImportDirectory(", "rescanH3LocalImport(", "updateH3ProjectSegmentDraft("].some((marker) => assetVideoBatchWorkspaceSource.includes(marker))) {
  throw new Error("ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER failed: AssetVideoBatchWorkspace must delegate local import commands");
}

console.log(`FRONTEND_NO_RAW_INVOKE=PASS`);
console.log(`RAW_INVOKE_OUTSIDE_TRANSPORT=0`);
console.log(`RPC_PARITY=PASS (${frontendCommands.size} frontend commands checked)`);
console.log(`SHOT_WORKSPACE_DIRECT_TASK_SUBSCRIPTION=PASS`);
console.log(`SHOT_WORKSPACE_QUEUE_CONTROLLER=PASS`);
console.log(`SHOT_WORKSPACE_MONITOR_CONTROLLER=PASS`);
console.log(`SHOT_WORKSPACE_MULTI_PACKAGE_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_WORKFLOW_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_LIBRARY_CONTROLLER=PASS`);
console.log(`ASSET_VIDEO_LOCAL_IMPORT_CONTROLLER=PASS`);
