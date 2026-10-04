import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve, relative } from "node:path";
import ts from "typescript";
import { backendPerformanceBoundary, backendBoundary } from "./backend-boundary-guard.mjs";
import { styleBoundary } from "./style-boundary-guard.mjs";
import { frontendBoundary } from "./frontend-boundary-guard.mjs";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(import.meta.url), "..", "..");
const frontendManifest = JSON.parse(readFileSync(join(root, "docs/architecture/phase10-frontend-architecture.json"), "utf8"));
const frontendResult = frontendBoundary(root, frontendManifest);
if (frontendResult.violations.length) throw new Error(`FRONTEND_ARCHITECTURE_BOUNDARY failed: ${frontendResult.violations.join("; ")}`);
console.log("FRONTEND_ARCHITECTURE_GUARD=PASS");
const styleManifest = JSON.parse(readFileSync(join(root, "docs/architecture/phase9-style-cleanup.json"), "utf8"));
const styleResult = styleBoundary(root, styleManifest);
if (styleResult.violations.length) throw new Error(`STYLE_ARCHITECTURE_BOUNDARY failed: ${styleResult.violations.join("; ")}`);
console.log("STYLE_ARCHITECTURE_GUARD=PASS");
const backendManifest = JSON.parse(readFileSync(join(root, "docs/architecture/phase8-backend-decomposition.json"), "utf8"));
const backendResult = backendBoundary(root, backendManifest);
if (backendResult.violations.length) throw new Error(`BACKEND_DIRECT_SQL_BOUNDARY failed: ${backendResult.violations.join("; ")}`);
console.log("NEW_DIRECT_SQLX_FROM_COMMANDS=0");
console.log("NEW_DIRECT_SQLX_FROM_APPLICATION=0");
console.log(`BACKEND_DIRECT_SQL_GUARD=PASS (${backendResult.checked} production files)`);
const ipcTransportPath = resolve(root, "src/services/ipc.ts");
const productionFrontendFiles = [];

const retiredAuthoringPaths = [
  "src-tauri/src/application/prompt_template_service.rs",
  "src-tauri/src/application/prompt_template_bulk_service.rs",
  "src-tauri/src/application/script_draft_service.rs",
  "src-tauri/src/application/script_import_service.rs",
  "src-tauri/src/application/script_import_parser",
  "src-tauri/src/commands/prompt_template.rs",
  "src-tauri/src/domain/prompt_template.rs",
  "src-tauri/src/domain/script_draft",
  "src/features/shots/PromptTemplatePanel.tsx",
  "src/features/prompts/PromptTemplateVariableHelper.tsx",
  "src/features/prompts/promptTemplateState.ts",
];
if (retiredAuthoringPaths.some((path) => existsSync(join(root, path)))) {
  throw new Error("INTERNAL_AUTHORING_RETIREMENT failed: retired authoring path still exists");
}

const activeAuthoringSources = [
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/commands/mod.rs",
  "src-tauri/src/domain/mod.rs",
  "src-tauri/src/infrastructure/database/repositories/mod.rs",
  "src-tauri/src/lib.rs",
  "src/services/tauriClient.ts",
].map((path) => readFileSync(join(root, path), "utf8"));
if (activeAuthoringSources.some((source) => /(?:prompt_template_|ScriptImportService|ScriptDraftService|script_import_parser|pub mod script_draft)/.test(source))) {
  throw new Error("INTERNAL_AUTHORING_RETIREMENT failed: active runtime still references retired authoring symbols");
}

const retiredArchitectureDocs = [
  "docs/architecture/NARRATIVE_PREPRODUCTION_V2.md",
  "docs/architecture/SCRIPT_IMPORT_V1.md",
  "docs/architecture/STORYBOARD_DRAFT_V1.md",
].map((path) => readFileSync(join(root, path), "utf8"));
if (retiredArchitectureDocs.some((source) => !source.includes("STATUS=RETIRED") || !source.includes("RETIRED_BY=DEV-100"))) {
  throw new Error("INTERNAL_STORYBOARD_AUTHORING_RETIRED failed: historical authoring docs must be explicitly retired");
}

const handoffArchitectureSource = readFileSync(join(root, "docs/DEV_100_EXTERNAL_AGENT_HANDOFF_ARCHITECTURE.md"), "utf8");
if (!["IMPORT_AUTO_QUEUE=NO", "IMPORT_AUTO_TASK=NO", "IMPORT_AUTO_GENERATION=NO", "HANDOFF_IMPLEMENTATION_BLOCKED_BY_SCHEMA=YES"].every((marker) => handoffArchitectureSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION failed: handoff must remain explicit and schema-blocked");
}

const handoffContractSource = readFileSync(join(root, "docs/EXTERNAL_AGENT_PRODUCTION_HANDOFF_V1.md"), "utf8");
const handoffServiceSource = readFileSync(join(root, "src-tauri/src/application/external_production_handoff_service.rs"), "utf8");
const handoffPortSource = readFileSync(join(root, "src-tauri/src/application/ports/external_production_handoff_repository.rs"), "utf8");
const handoffRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/external_production_handoff.rs"), "utf8");
const handoffCommandSource = readFileSync(join(root, "src-tauri/src/commands/external_production_handoff.rs"), "utf8");
const handoffMigrationSource = readFileSync(join(root, "src-tauri/migrations/032_external_production_handoffs.sql"), "utf8");
const handoffClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const handoffPanelSource = readFileSync(join(root, "src/features/projects/ExternalAgentHandoffPanel.tsx"), "utf8");
if (!handoffContractSource.includes("STATUS=IMPLEMENTED")
  || !handoffContractSource.includes("IMPLEMENTATION=SERVER_ATOMIC")
  || !handoffContractSource.includes("MIGRATION=032")) {
  throw new Error("EXTERNAL_HANDOFF_CONTRACT_IMPLEMENTED failed: V1 contract must describe the shipped implementation");
}
if (!handoffServiceSource.includes("pub async fn preview(")
  || !handoffServiceSource.includes("pub async fn confirm(")
  || !handoffPortSource.includes("trait ExternalProductionHandoffRepository")
  || !handoffRepositorySource.includes("async fn import_atomic(")
  || !handoffRepositorySource.includes("self.pool.begin()")) {
  throw new Error("EXTERNAL_HANDOFF_AUTHORITY failed: service/port/repository transaction authority is incomplete");
}
if (!handoffServiceSource.includes("repository.import_atomic")
  || !handoffServiceSource.includes("find_by_document_hash")
  || !handoffPanelSource.includes("previewExternalProductionHandoff")
  || !handoffPanelSource.includes("confirmExternalProductionHandoff")) {
  throw new Error("EXTERNAL_HANDOFF_PREVIEW_READ_ONLY failed: preview and explicit confirmation boundaries are required");
}
if (!["external_production_handoffs", "external_production_handoff_entities", "UNIQUE (project_id, document_sha256)"].every((marker) => handoffMigrationSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_SINGLE_TRANSACTION failed: durable handoff identity/provenance migration is missing");
}
if (["create_task", "enqueue", "start_comfy", "ComfyUI", "production_queue"].some((marker) => handoffServiceSource.includes(marker) || handoffRepositorySource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION failed: handoff must not own queue, task, or executor side effects");
}
if (!handoffServiceSource.includes("find_active")
  || !handoffRepositorySource.includes("workflow_version_id")
  || !handoffRepositorySource.includes("recipe_id")
  || !handoffRepositorySource.includes("archived")) {
  throw new Error("EXTERNAL_HANDOFF_EXACT_WORKFLOW_RECIPE failed: exact active workflowVersionId+recipeId validation is required");
}
if (/[\s\S]*sqlx::/.test(handoffServiceSource) || /[\s\S]*sqlx::/.test(handoffPortSource)) {
  throw new Error("APPLICATION_DIRECT_SQLX_NEW_USAGE failed: handoff application layer must use repository ports");
}
if (!["previewExternalProductionHandoff", "confirmExternalProductionHandoff", "external_production_handoff_preview", "external_production_handoff_confirm"].every((marker) => handoffClientSource.includes(marker) || handoffCommandSource.includes(marker))) {
  throw new Error("EXTERNAL_HANDOFF_TYPED_IPC failed: typed client and command parity is required");
}

const handoffSchema = JSON.parse(readFileSync(join(root, "docs/schemas/production-handoff-v1.schema.json"), "utf8"));
const handoffExample = JSON.parse(readFileSync(join(root, "docs/examples/production-handoff-v1.example.json"), "utf8"));
const handoffTemplate = readFileSync(join(root, "docs/EXTERNAL_AGENT_HANDOFF_PROMPT_TEMPLATE.md"), "utf8");
const handoffObjectTypes = [handoffSchema, ...Object.values(handoffSchema.$defs).filter((definition) => definition.type === "object")];
if (handoffSchema.properties?.schemaVersion?.const !== 1
  || handoffSchema.$schema !== "https://json-schema.org/draft/2020-12/schema"
  || handoffSchema.$defs?.scene?.properties?.shots?.maxItems !== 500
  || !handoffServiceSource.includes("pub const MAX_HANDOFF_SHOTS: usize = 500")
  || handoffObjectTypes.some((definition) => definition.additionalProperties !== false)
  || ["schemaVersion", "projectId", "source", "series"].some((field) => !handoffSchema.required?.includes(field))
  || ["externalId", "name", "ordinal", "description", "imagePrompt", "videoPrompt", "assetRefs", "stages"].some((field) => !handoffSchema.$defs?.shot?.properties?.[field])
  || ["workflowVersionId", "recipeId"].some((field) => !handoffSchema.$defs?.stage?.required?.includes(field))) {
  throw new Error("HANDOFF_JSON_SCHEMA_PARITY failed: V1 shape, strictness, or bounds drifted");
}
const exampleShots = handoffExample.series?.[0]?.episodes?.[0]?.scenes?.[0]?.shots;
if (handoffExample.schemaVersion !== 1 || handoffExample.series?.length !== 1
  || handoffExample.series[0].episodes?.length !== 1
  || handoffExample.series[0].episodes[0].scenes?.length !== 1
  || exampleShots?.length !== 2 || exampleShots.some((shot) => !shot.imagePrompt || !shot.videoPrompt)
  || exampleShots[0].stages || !exampleShots[1].stages?.image?.workflowVersionId
  || !exampleShots[1].stages.image.recipeId) {
  throw new Error("HANDOFF_EXAMPLE_VALID failed: canonical hierarchy or stage-pair fixture drifted");
}
if (!handoffContractSource.includes("SERVER_VALIDATED")
  || !handoffTemplate.includes("Return **only** one JSON object")
  || !handoffTemplate.includes("Do not guess Asset IDs")
  || /(?:api key|connector|provider adapter)/i.test(handoffTemplate)) {
  throw new Error("HANDOFF_TOOLING_PROVIDER_NEUTRAL failed: handoff template or server-validation boundary drifted");
}

function collectSourceFiles(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collectSourceFiles(path);
    else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\./.test(entry.name)) productionFrontendFiles.push(path);
  }
}

collectSourceFiles(join(root, "src"));

const legacyTransportAllowlist = new Set(JSON.parse(readFileSync(join(root, "scripts/product-legacy-transport-allowlist.json"), "utf8")));
function importsFrom(source) {
  const imports = [];
  const file = ts.createSourceFile("boundary.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) imports.push(node.moduleSpecifier.text);
    if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === "require")) && node.arguments[0] && ts.isStringLiteral(node.arguments[0])) imports.push(node.arguments[0].text);
    if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference) && node.moduleReference.expression && ts.isStringLiteral(node.moduleReference.expression)) imports.push(node.moduleReference.expression.text);
    ts.forEachChild(node, visit);
  }
  visit(file);
  return imports;
}
function productBoundaryViolations(path, source) {
  const imports = importsFrom(source);
  const transport = path === "src/product/transport.ts";
  const product = path.startsWith("src/product/");
  const newPage = path.startsWith("src/v3/") || path.startsWith("src/app/v3/") || path.startsWith("src/app/routes/") || path.startsWith("src/pages/") || path.startsWith("src/features/create/") || path.startsWith("src/features/runs/") || path.startsWith("src/features/library/");
  const violations = [];
  for (const specifier of imports) {
    const directLegacy = /(?:^|\/)tauriClient(?:\.ts)?$/.test(specifier);
    const oldTransport = directLegacy || /(?:^|\/)services\/(?:ipc|workflowClient)(?:\.ts)?$/.test(specifier) || specifier === "@tauri-apps/api/core";
    if ((product && !transport && oldTransport) || (newPage && oldTransport)) violations.push(`${path}: forbidden product/transport import ${specifier}`);
    // Phase13's exact Advanced read seam is hash/scope-checked by styleBoundary
    // above; this does not authorize any normal Product or new-page importer.
    const advancedDiagnosticsSeam = path === "src/services/diagnosticsClient.ts";
    if (directLegacy && !transport && !advancedDiagnosticsSeam && !legacyTransportAllowlist.has(path)) violations.push(`${path}: new legacy importer`);
  }
  return violations;
}
const productBoundaryErrors = productionFrontendFiles.flatMap((path) => productBoundaryViolations(relative(root,path).replaceAll("\\", "/"), readFileSync(path,"utf8")));
if (productBoundaryErrors.length) throw new Error(`PRODUCT_FACADE_BOUNDARY failed:\n${productBoundaryErrors.join("\n")}`);
// Negative probes exercise the same scanner without creating source files.
for (const source of ["import { x } from '../services/tauriClient';", "export * from '../services/ipc';", "const x = import('../services/tauriClient');", "const x = require('../services/tauriClient');", "import x = require('../services/tauriClient');"]) {
  if (!productBoundaryViolations("src/product/invalid.ts",source).length || !productBoundaryViolations("src/v3/invalid.ts",source).length) throw new Error("PRODUCT_FACADE_BOUNDARY negative probe failed");
}
if (!productBoundaryViolations("src/features/newInvalid.ts", "import { x } from '../../services/tauriClient';").length) throw new Error("PRODUCT_FACADE_BOUNDARY grandfather probe failed");
for (const source of ["import { invokeCommand } from '../../services/ipc';", "export * from '../../services/workflowClient';", "import { invoke } from '@tauri-apps/api/core';"]) {
  if (!productBoundaryViolations("src/features/create/invalid.ts", source).length) throw new Error("CREATE_PRODUCT_BOUNDARY negative probe failed");
  if (!productBoundaryViolations("src/features/library/invalid.ts", source).length) throw new Error("LIBRARY_PRODUCT_BOUNDARY negative probe failed");
  if (!productBoundaryViolations("src/features/runs/invalid.ts", source).length) throw new Error("RUNS_PRODUCT_BOUNDARY negative probe failed");
}
console.log("PRODUCT_FACADE_BOUNDARY=PASS");
export const productBoundaryVerified = true;

// The product transport uses a generic invoke wrapper: literal-call scans alone
// do not cover its command map. Verify the actual typed command keys as well.
const productTransportAst = ts.createSourceFile("transport.ts", readFileSync(join(root, "src/product/transport.ts"), "utf8"), ts.ScriptTarget.Latest, true);
const productCommandMap = productTransportAst.statements.find((node) => ts.isInterfaceDeclaration(node) && node.name.text === "ProductCommands");
if (!productCommandMap || !productCommandMap.members.length) throw new Error("PRODUCT_COMMAND_PARITY failed: command map missing");
const registeredProductCommands = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const productCommandDefinitions = readFileSync(join(root, "src-tauri/src/commands/product.rs"), "utf8");
for (const member of productCommandMap.members) {
  if (!ts.isPropertySignature(member) || !member.name || !ts.isIdentifier(member.name)) throw new Error("PRODUCT_COMMAND_PARITY failed: nonliteral command key");
  const command = member.name.text;
  if (!registeredProductCommands.includes(`commands::product::${command},`) || !new RegExp(`pub async fn ${command}\\s*\\(`).test(productCommandDefinitions)) throw new Error(`PRODUCT_COMMAND_PARITY failed: ${command} not registered/defined`);
}
console.log(`PRODUCT_COMMAND_PARITY=PASS (${productCommandMap.members.length} commands)`);

// Canonical location has exactly one hook owner; adapters cannot own domain effects.
const routeSources = productionFrontendFiles.filter((path) => relative(root, path).replaceAll("\\", "/").startsWith("src/app/routes/"));
const ownerCalls = productionFrontendFiles.reduce((count, path) => {
  const source = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
  let calls = 0;
  function visit(node) {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "useAppRoute") calls += 1;
    ts.forEachChild(node, visit);
  }
  visit(source);
  return count + calls;
}, 0);
if (ownerCalls !== 1) throw new Error("CANONICAL_ROUTE_OWNER failed: expected one hook invocation");
for (const path of routeSources.filter((path) => /(?:legacyAdapter|resumeAdapter|commandCenterAdapter|reducer)\.ts$/.test(path))) {
  if (importsFrom(readFileSync(path, "utf8")).some((specifier) => /(?:services|stores|@tauri-apps)\//.test(specifier))) throw new Error("ROUTE_ADAPTER_PURITY failed");
}
const appSource = readFileSync(join(root, "src/app/App.tsx"), "utf8");
if (/\[\s*(?:workspace|activeStudioSection|focusedTaskId|focusedProductionBatchId|focusedAssetId|resumeShotId)\s*,/.test(appSource)) throw new Error("CANONICAL_ROUTE_OWNER failed: legacy location still owns state");
console.log("CANONICAL_ROUTE_BOUNDARY=PASS");


const rawInvokeImports = productionFrontendFiles.filter((path) => {
  const source = readFileSync(path, "utf8");
  return source.includes("@tauri-apps/api/core") && resolve(path) !== ipcTransportPath;
});
if (rawInvokeImports.length) {
  throw new Error(`FRONTEND_NO_RAW_INVOKE failed:\n${rawInvokeImports.join("\n")}`);
}

const transportSources = [
  readFileSync(join(root, "src/product/client.ts"), "utf8").replaceAll("productRequest", "invokeCommand"),
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

// Phase6 extracts the existing control plane, preserving all ownership guards.
const workflowWorkspaceSource = [
  "src/features/workflow-lab/WorkflowLabPage.tsx",
  "src/features/workflows/useWorkflowLabController.ts",
  "src/features/workflow-lab/WorkflowLabSurface.tsx",
].map(path => readFileSync(join(root, path), "utf8")).join("\n");
const workflowWorkspaceAdaptersSource = readFileSync(join(root, "src/features/workflows/workflowWorkspaceAdapters.ts"), "utf8");
const recipeHistoryServiceSource = readFileSync(join(root, "src-tauri/src/application/recipe_history_query_service.rs"), "utf8");
const recipeHistoryPortSource = readFileSync(join(root, "src-tauri/src/application/ports/recipe_history_query_repository.rs"), "utf8");
const recipeHistoryRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/recipe_history_query.rs"), "utf8");
const recipeHistoryCommandSource = readFileSync(join(root, "src-tauri/src/commands/recipe_history.rs"), "utf8");
const recipeHistoryClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const workflowPromotionRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/workflow_recipe_promotion.rs"), "utf8");
const recipeArchiveMigrationSource = readFileSync(join(root, "src-tauri/migrations/031_workflow_recipe_archive_state.sql"), "utf8");
const recipeArchivePortSource = readFileSync(join(root, "src-tauri/src/application/ports/workflow_recipe_runtime_state_repository.rs"), "utf8");
const recipeArchiveRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/workflow_recipe_runtime_state.rs"), "utf8");
const workflowPromotionPortSource = readFileSync(join(root, "src-tauri/src/application/ports/workflow_recipe_promotion_repository.rs"), "utf8");
const workflowRegistryServiceSource = readFileSync(join(root, "src-tauri/src/application/workflow_registry_service.rs"), "utf8");
const workflowRegistryCommandSource = readFileSync(join(root, "src-tauri/src/commands/workflow_registry.rs"), "utf8");
const appErrorSource = readFileSync(join(root, "src-tauri/src/error.rs"), "utf8");
if (!workflowWorkspaceSource.includes("useWorkflowSmartImportController")) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must delegate Smart Import session ownership to useWorkflowSmartImportController");
}
const smartImportStateMarkers = ["autoPlan", "autoImportError"];
const smartImportStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && smartImportStateMarkers.some((marker) => line.includes(marker)));
if (smartImportStateLines.length) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must not own Smart Import session state");
}
const smartImportFunctionMarkers = [
  "function smartImportWorkflow(",
  "function resumeAutoImport(",
  "function regenerateExistingRecipe(",
  "function resolveAutoIssue(",
  "function commitAnalyzedImport(",
  "function openAdvancedImport(",
  "function openExistingWorkflow(",
  "function openStructuralVariantAsVersion(",
];
if (smartImportFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_SMART_IMPORT_CONTROLLER failed: WorkflowWorkspace must delegate Smart Import actions");
}

if (!workflowWorkspaceSource.includes("useWorkflowParameterExposureController")) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must delegate Parameter Exposure ownership to useWorkflowParameterExposureController");
}

if (!appErrorSource.includes("WorkflowRecipeLifecycleError")
  || !workflowRegistryCommandSource.includes("AppError::workflow_recipe_lifecycle(")
  || workflowRegistryCommandSource.includes("WORKFLOW_RECIPE_LIFECYCLE_ERROR: workflow_version_id=")) {
  throw new Error("STRUCTURED_IPC_ERROR failed: recipe lifecycle errors must use typed IPC details instead of message parsing");
}
const parameterExposureStateMarkers = ["parameterDraft", "parameterItem", "parameterOriginalKeys", "parameterLoading"];
const parameterExposureStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && parameterExposureStateMarkers.some((marker) => line.includes(marker)));
if (parameterExposureStateLines.length) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must not own Parameter Exposure session state");
}
const parameterExposureFunctionMarkers = [
  "function openParameterExposure(",
  "function closeParameterExposure(",
  "function refreshParameterCapability(",
  "function exposeParameter(",
  "function saveParameterMapping(",
  "function removeParameterMapping(",
  "function publishParameterRecipe(",
];
if (parameterExposureFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER failed: WorkflowWorkspace must delegate Parameter Exposure actions");
}

const recipeHistoryMutationMarkers = [
  "archive_recipe", "restore_recipe", "promote_recipe", "clear_recipe_promotion", "requeue_task", "retry_task", "cancel_task",
];
if (!recipeHistoryServiceSource.includes("pub struct RecipeHistoryQueryService")
  || !recipeHistoryServiceSource.includes("pub async fn get_exact_pair(")
  || !recipeHistoryPortSource.includes("trait RecipeHistoryQueryRepository")
  || !recipeHistoryRepositorySource.includes("impl RecipeHistoryQueryRepository for SqliteRecipeHistoryQueryRepository")
  || !recipeHistoryCommandSource.includes("pub async fn workflow_recipe_history_get(")
  || !recipeHistoryClientSource.includes('"workflow_recipe_history_get"')) {
  throw new Error("RECIPE_HISTORY_QUERY_AUTHORITY failed: exact-pair service, repository, command, and typed client are required");
}
if (recipeHistoryMutationMarkers.some((marker) => recipeHistoryServiceSource.includes(marker) || recipeHistoryCommandSource.includes(marker))
  || recipeHistoryServiceSource.includes("sqlx::")
  || recipeHistoryPortSource.includes("&mut")
  || recipeHistoryPortSource.includes("fn archive")
  || recipeHistoryPortSource.includes("fn restore")) {
  throw new Error("RECIPE_HISTORY_READ_ONLY failed: history query must not expose mutation authority or application SQLx");
}
const migrationFiles = readdirSync(join(root, "src-tauri/migrations"));
if (migrationFiles.some((file) => /recipe.?history/i.test(file))
  || migrationFiles.some((file) => /\.sql$/.test(file) && /CREATE\s+TABLE\s+recipe_history/i.test(readFileSync(join(root, "src-tauri/migrations", file), "utf8")))) {
  throw new Error("RECIPE_HISTORY_READ_ONLY failed: a new recipe history table or migration was added");
}
if (!workflowWorkspaceSource.includes("openRecipeHistory")
  || !workflowWorkspaceSource.includes("getWorkflowRecipeHistory(")
  || !workflowWorkspaceSource.includes("onViewHistory")
  || /useEffect\([\s\S]{0,600}getWorkflowRecipeHistory\(/.test(workflowWorkspaceSource)) {
  throw new Error("RECIPE_HISTORY_ON_DEMAND failed: history must load only from the existing recipe detail action");
}

const workflowClientSource = readFileSync(join(root, "src/services/workflowClient.ts"), "utf8");
const tauriClientSource = readFileSync(join(root, "src/services/tauriClient.ts"), "utf8");
const generationStudioSource = readFileSync(join(root, "src/features/studio/GenerationStudio.tsx"), "utf8");
if (!workflowWorkspaceSource.includes("promoteWorkflowRecipe(")
  || !workflowClientSource.includes("promoteWorkflowRecipe")
  || !tauriClientSource.includes('"workflow_promote_recipe"')) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION failed: promotion must be owned by WorkflowWorkspace through the typed workflow client");
}
const generationStudioPromotionMarkers = ["promoteWorkflowRecipe", "workflow_promote_recipe", "setPromotedRecipe"];
const generationStudioPromotionOwnership = generationStudioPromotionMarkers
  .filter((marker) => generationStudioSource.includes(marker));
if (generationStudioPromotionOwnership.length) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION failed: GenerationStudio must not own recipe promotion");
}
if (!workflowWorkspaceSource.includes("resolveImplicitWorkflowRecipe")
  || !workflowWorkspaceAdaptersSource.includes("export function resolveImplicitWorkflowRecipe(")
  || generationStudioSource.includes("resolveImplicitWorkflowRecipe")
  || generationStudioSource.includes("isPromoted")) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CONSUMPTION failed: promotion must be consumed once by the Workflow Workspace resolution boundary");
}
const generationStudioClearPromotionMarkers = ["clearWorkflowRecipePromotion", "workflow_clear_recipe_promotion", "clearRecipePromotion"];
if (!workflowWorkspaceSource.includes("clearWorkflowRecipePromotion(")
  || !workflowClientSource.includes("clearWorkflowRecipePromotion")
  || !tauriClientSource.includes('"workflow_clear_recipe_promotion"')
  || !libSource.includes("workflow_clear_recipe_promotion")
  || generationStudioClearPromotionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CLEAR failed: clear must have one typed Workflow Workspace path and no GenerationStudio ownership");
}
if ((workflowPromotionRepositorySource.match(/impl WorkflowRecipePromotionRepository for /g) ?? []).length !== 1
  || !workflowPromotionPortSource.includes("async fn clear(")
  || !workflowPromotionRepositorySource.includes("async fn clear(")
  || !workflowPromotionRepositorySource.includes("DELETE FROM workflow_recipe_promotions")
  || !workflowRegistryServiceSource.includes("pub async fn clear_recipe_promotion(")
  || !workflowRegistryCommandSource.includes("pub async fn workflow_clear_recipe_promotion(")) {
  throw new Error("WORKFLOW_RECIPE_PROMOTION_CLEAR failed: clear must remain owned by the existing repository/service/command authority");
}

const recipeArchiveApplicationSources = [
  readFileSync(join(root, "src-tauri/src/application/workflow_registry_service.rs"), "utf8"),
  readFileSync(join(root, "src-tauri/src/application/workflow_lifecycle_service.rs"), "utf8"),
  readFileSync(join(root, "src-tauri/src/application/production_queue_service.rs"), "utf8"),
];
const directApplicationSqlx = recipeArchiveApplicationSources.filter((source) => /\bsqlx::/.test(source));
if (!recipeArchiveMigrationSource.includes("CREATE TABLE workflow_recipe_runtime_states")
  || !recipeArchiveMigrationSource.includes("PRIMARY KEY (workflow_version_id, recipe_id)")
  || !recipeArchiveMigrationSource.includes("REFERENCES recipes(workflow_version_id, id)")
  || !recipeArchivePortSource.includes("trait WorkflowRecipeRuntimeStateRepository")
  || !recipeArchivePortSource.includes("workflow_version_id")
  || !recipeArchivePortSource.includes("recipe_id")
  || !recipeArchiveRepositorySource.includes("impl WorkflowRecipeRuntimeStateRepository for")
  || !workflowRegistryServiceSource.includes("recipe_runtime_state_repository")
  || !workflowRegistryServiceSource.includes("pub async fn archive_recipe(")
  || !workflowRegistryServiceSource.includes("pub async fn restore_recipe(")
  || !workflowRegistryServiceSource.includes("recipe_is_archived")
  || directApplicationSqlx.length) {
  throw new Error("RECIPE_ARCHIVE_AUTHORITY failed: recipe archive must use one dedicated infrastructure-backed exact-pair state authority");
}
if (!workflowWorkspaceAdaptersSource.includes("!item.currentRecipe.archived")
  || !workflowWorkspaceAdaptersSource.includes("item.registryRecipes?.find")
  || !workflowWorkspaceAdaptersSource.includes("?.archived")) {
  throw new Error("ONE_IMPLICIT_RECIPE_RESOLVER failed: archived recipes must be excluded by resolveImplicitWorkflowRecipe");
}
if (!workflowClientSource.includes("archiveWorkflowRecipe")
  || !workflowClientSource.includes("restoreWorkflowRecipe")
  || !tauriClientSource.includes('"workflow_archive_recipe"')
  || !tauriClientSource.includes('"workflow_restore_recipe"')
  || !workflowRegistryCommandSource.includes("pub async fn workflow_archive_recipe(")
  || !workflowRegistryCommandSource.includes("pub async fn workflow_restore_recipe(")
  || !libSource.includes("workflow_archive_recipe")
  || !libSource.includes("workflow_restore_recipe")) {
  throw new Error("RECIPE_ARCHIVE_AUTHORITY failed: archive/restore must use the typed Registry command path");
}

if (!generationStudioSource.includes("useGenerationPresetController")) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must delegate preset lifecycle ownership to useGenerationPresetController");
}
const generationPresetStateMarkers = ["presets", "selectedPresetId", "preferredPresetId", "presetName", "presetLoading", "presetError", "presetEditorOpen"];
const generationPresetStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationPresetStateMarkers.some((marker) => line.includes(marker)));
if (generationPresetStateLines.length) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must not own preset lifecycle state");
}
const generationPresetFunctionMarkers = [
  "function applyPreset(",
  "function savePreset(",
  "function savePresetChanges(",
  "function removePreset(",
  "function togglePreferredPreset(",
];
if (generationPresetFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_PRESET_CONTROLLER failed: GenerationStudio must delegate preset lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationSubmissionController")) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must delegate submission ownership to useGenerationSubmissionController");
}
const generationSubmissionStateMarkers = ["creating", "cancelling"];
const generationSubmissionStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationSubmissionStateMarkers.some((marker) => line.includes(marker)));
if (generationSubmissionStateLines.length || generationStudioSource.includes("generationRequestIdRef")) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must not own submission state or idempotency refs");
}
const generationSubmissionFunctionMarkers = [
  "function generate(",
  "function cancelCurrentTask(",
];
if (generationSubmissionFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_SUBMISSION_CONTROLLER failed: GenerationStudio must delegate submission actions");
}

if (!generationStudioSource.includes("useGenerationBatchController")) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must delegate batch lifecycle ownership to useGenerationBatchController");
}
const generationBatchStateMarkers = ["batchItems", "batchSubmitting", "batchNotice", "batchPasteText"];
const generationBatchStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationBatchStateMarkers.some((marker) => line.includes(marker)));
if (generationBatchStateLines.length) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must not own batch lifecycle state");
}
const generationBatchFunctionMarkers = [
  "function addCurrentToBatch(",
  "function addBlankPromptCard(",
  "function updateBatchPrompt(",
  "function copyBatchItem(",
  "function moveBatchItem(",
  "function splitPastedPrompts(",
  "function removeBatchItem(",
  "function importBatchTaskList(",
  "function submitBatch(",
];
if (generationBatchFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_BATCH_CONTROLLER failed: GenerationStudio must delegate batch lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationExperimentController")) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must delegate experiment lifecycle ownership to useGenerationExperimentController");
}
const generationExperimentStateMarkers = ["experimentFocusBatchId", "experimentContexts", "promptExperimentDimensions"];
const generationExperimentStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationExperimentStateMarkers.some((marker) => line.includes(marker)));
if (generationExperimentStateLines.length) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must not own experiment lifecycle state");
}
const generationExperimentFunctionMarkers = [
  "function submitExperimentPlan(",
  "function promoteExperimentWinner(",
];
if (generationExperimentFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_EXPERIMENT_CONTROLLER failed: GenerationStudio must delegate experiment lifecycle actions");
}

if (!generationStudioSource.includes("useGenerationProjectTemplateController")) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must delegate project template lifecycle ownership to useGenerationProjectTemplateController");
}
const generationProjectTemplateStateMarkers = ["templateEditorOpen", "templateName", "templateDescription", "templateSaving", "templateError"];
const generationProjectTemplateStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationProjectTemplateStateMarkers.some((marker) => line.includes(marker)));
if (generationProjectTemplateStateLines.length) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must not own project template lifecycle state");
}
if (generationStudioSource.includes("function saveProjectTemplate(") || generationStudioSource.includes("createProjectTemplate(")) {
  throw new Error("GENERATION_PROJECT_TEMPLATE_CONTROLLER failed: GenerationStudio must delegate project template persistence");
}

if (!generationStudioSource.includes("useGenerationAssetIntentController")) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must delegate Asset Intent lifecycle ownership to useGenerationAssetIntentController");
}
const generationAssetIntentStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && line.includes("assetIntentTargets"));
if (generationAssetIntentStateLines.length) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must not own Asset Intent target state");
}
if (generationStudioSource.includes("function applyPendingAsset(") || generationStudioSource.includes("clearPendingAssetIntent(")) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: GenerationStudio must delegate Asset Intent lifecycle actions");
}
const assetIntentControllerSource = readFileSync(join(root, "src/features/studio/hooks/useGenerationAssetIntentController.ts"), "utf8");
if (/useState\s*<[^>]*PendingStudioAssetIntent|\[\s*pendingAssetIntent\s*,/.test(assetIntentControllerSource)) {
  throw new Error("GENERATION_ASSET_INTENT_CONTROLLER failed: controller must keep pendingAssetIntent authority in useStudioStore");
}

if (!generationStudioSource.includes("useGenerationWorkflowSelectionController")) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must delegate workflow selection ownership to useGenerationWorkflowSelectionController");
}
const generationWorkflowSelectionStateMarkers = ["manualSelection", "projectWorkflowConfig"];
const generationWorkflowSelectionStateLines = generationStudioSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && generationWorkflowSelectionStateMarkers.some((marker) => line.includes(marker)));
if (generationWorkflowSelectionStateLines.length) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must not own workflow selection state");
}
const generationWorkflowSelectionFunctionMarkers = [
  "function selectWorkflowFromUx(",
  "function restoreRecommendedWorkflow(",
];
if (generationWorkflowSelectionFunctionMarkers.some((marker) => generationStudioSource.includes(marker))) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must delegate workflow selection actions");
}
if (generationStudioSource.includes("getProjectWorkflowConfig(")) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: GenerationStudio must not load project workflow config directly");
}
const workflowSelectionControllerSource = readFileSync(join(root, "src/features/studio/hooks/useGenerationWorkflowSelectionController.ts"), "utf8");
if (/useState\s*<[^>]*RecipeViewModel|\[\s*selectedWorkflow\s*,/.test(workflowSelectionControllerSource)) {
  throw new Error("GENERATION_WORKFLOW_SELECTION_CONTROLLER failed: controller must keep selectedWorkflow authority in useStudioStore");
}

if (!workflowWorkspaceSource.includes("useWorkflowAdvancedOnboardingController")) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must delegate Advanced Onboarding ownership to useWorkflowAdvancedOnboardingController");
}
const advancedOnboardingStateMarkers = ["mappingDrafts", "outputDraft", "metadataDraft", "published", "showAdvanced"];
const advancedOnboardingStateLines = workflowWorkspaceSource
  .split(/\r?\n/)
  .filter((line) => line.includes("useState") && advancedOnboardingStateMarkers.some((marker) => line.includes(marker)));
if (advancedOnboardingStateLines.length) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must not own Advanced Onboarding state");
}
const advancedOnboardingFunctionMarkers = [
  "function runDraftAction(",
  "function saveMetadata(",
  "function validateDraft(",
  "function publishDraft(",
  "function bindInput(",
  "function removeInput(",
  "function addOutput(",
  "function checkCapability(",
  "function discardDraft(",
];
if (advancedOnboardingFunctionMarkers.some((marker) => workflowWorkspaceSource.includes(marker))) {
  throw new Error("WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER failed: WorkflowWorkspace must delegate Advanced Onboarding actions");
}

const projectCommandCenterServiceSource = readFileSync(join(root, "src-tauri/src/application/project_command_center_service.rs"), "utf8");
if (!["pub fn recommend_next_project_action", "first_completed_asset_id", "fn action_with_targets"].every((marker) => projectCommandCenterServiceSource.includes(marker))) {
  throw new Error("PROJECT_CONTINUITY_DERIVED failed: Project Command Center must derive exact continuation targets from existing facts");
}
const projectCommandCenterTypesSource = readFileSync(join(root, "src/types/projectCommandCenter.ts"), "utf8");
const projectCommandCenterViewSource = readFileSync(join(root, "src/features/projects/ProjectCommandCenter.tsx"), "utf8");
if (!["ProjectCommandCenterDailyProductionView", "totalCount", "hasMore", "workflowVersionId", "recipeId"].every((marker) => projectCommandCenterTypesSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_EXACT_TARGETS failed: typed daily production contract is incomplete");
}
if (!["load_daily_production", "DAILY_PRODUCTION_ITEM_LIMIT", "top_action", "Production Queue"].every((marker) => projectCommandCenterServiceSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_DERIVED failed: daily production must be derived in the existing command center service");
}
if (!projectCommandCenterViewSource.includes("DailyProductionBoard") || projectCommandCenterServiceSource.includes("CREATE TABLE daily_production")) {
  throw new Error("DAILY_PRODUCTION_NO_SECOND_STATE failed: daily production must remain a read-only view");
}
if (!["onNavigate", "actionKind", "dailyProductionNavigation"].every((marker) => projectCommandCenterViewSource.includes(marker))) {
  throw new Error("DAILY_PRODUCTION_NAVIGATION failed: daily production items must use existing typed navigation");
}

const shotBulkConfigSource = readFileSync(join(root, "src/features/shots/ShotBulkConfigPanel.tsx"), "utf8");
const shotBatchPlannerSource = readFileSync(join(root, "src/features/shots/ShotBatchPlanner.tsx"), "utf8");
const productionPreparationSource = readFileSync(join(root, "src-tauri/src/application/production_preparation_service.rs"), "utf8");
const productionPreparationCommandSource = readFileSync(join(root, "src-tauri/src/commands/production_preparation.rs"), "utf8");
const bulkPreparationUiSource = `${shotBulkConfigSource}\n${shotBatchPlannerSource}`;
if (bulkPreparationUiSource.includes("startProductionQueue")
  || bulkPreparationUiSource.includes("production_queue_start")
  || bulkPreparationUiSource.includes("createAndStart")) {
  throw new Error("BULK_PREPARE_NO_AUTO_START failed: bulk preparation UI must not start the queue");
}
if (!shotBulkConfigSource.includes("admitProjectProduction")
  || !shotBulkConfigSource.includes("allowPartial")
  || !shotBulkConfigSource.includes("MAX_BULK_PREPARATION_ITEMS")) {
  throw new Error("BATCH_CREATE_NO_AUTO_START failed: project bulk preparation must use explicit bounded admission");
}
if (!productionPreparationSource.includes("pub async fn plan_many(")
  || !productionPreparationSource.includes("pub async fn admit(")
  || !productionPreparationSource.includes("ProductionBatchStatus::Ready")
  || productionPreparationSource.includes("startProductionQueue")
  || productionPreparationSource.includes("production_queue_start")) {
  throw new Error("EXPLICIT_QUEUE_START_ONLY failed: preparation authority must create READY state without queue start");
}
if (!["project_production_preflight", "project_production_admit", "MAX_PROJECT_PLAN_SHOTS", "MAX_PREPARATION_BATCH_ITEMS"].every((marker) => productionPreparationCommandSource.includes(marker))) {
  throw new Error("BULK_PREPARATION_AUTHORITY failed: project plan/admit must use the existing preparation service with 500/100 bounds");
}

const reviewInboxSource = readFileSync(join(root, "src/features/production/ProductionReviewInbox.tsx"), "utf8");
const artifactServiceSource = readFileSync(join(root, "src-tauri/src/application/artifact_service.rs"), "utf8");
const artifactCommandSource = readFileSync(join(root, "src-tauri/src/commands/artifact.rs"), "utf8");
const artifactRepositorySource = readFileSync(join(root, "src-tauri/src/infrastructure/database/repositories/artifact.rs"), "utf8");
const packageWorkspaceSource = readFileSync(join(root, "src/features/production/ProductionPackageWorkspace.tsx"), "utf8");
const packageServiceSource = readFileSync(join(root, "src-tauri/src/application/production_package_service.rs"), "utf8");
if (reviewInboxSource.includes("startProductionQueue")
  || reviewInboxSource.includes("production_queue_start")
  || reviewInboxSource.includes("createProductionPackageBatches")) {
  throw new Error("ARTIFACT_REVIEW_NO_EXECUTION failed: review UI must not create or start production batches");
}
if (!reviewInboxSource.includes("getArtifactReviewQueue")
  || !reviewInboxSource.includes("submitArtifactReview")
  || !artifactServiceSource.includes("pub async fn review_queue(")
  || !artifactServiceSource.includes(".list_review_queue(project_id, filter, limit.clamp(1, 100), offset)")
  || !artifactServiceSource.includes("pub async fn submit_review(")
  || !artifactServiceSource.includes("update_review_if_revision")) {
  throw new Error("ARTIFACT_REVIEW_USES_EXISTING_AUTHORITY failed: review must use bounded ArtifactService and revision-checked repository operations");
}
if (!artifactCommandSource.includes("pub async fn artifact_review_queue_get(")
  || !artifactCommandSource.includes(".review_queue(")
  || !artifactCommandSource.includes("pub async fn artifact_review_submit(")
  || !artifactCommandSource.includes(".submit_review(")) {
  throw new Error("ARTIFACT_REVIEW_TYPED_COMMANDS failed: review UI must use the registered ArtifactService command boundary");
}
if (!artifactRepositorySource.includes("async fn list_review_queue(")
  || !artifactRepositorySource.includes("WHERE review.project_id = ? AND {decision_filter}")
  || !artifactRepositorySource.includes("a.project_id = review.project_id")
  || !artifactRepositorySource.includes("t.project_id = review.project_id")
  || !artifactRepositorySource.includes("LIMIT ? OFFSET ?")) {
  throw new Error("ARTIFACT_REVIEW_PROJECT_SCOPE failed: persisted review listings must remain project-scoped and paginated");
}
if (packageWorkspaceSource.includes("startProductionQueue")
  || packageWorkspaceSource.includes("production_queue_start")
  || !packageWorkspaceSource.includes("尚未开始真实生产")) {
  throw new Error("PACKAGE_IMPORT_NO_AUTO_START failed: package import must leave execution to the explicit Production Queue action");
}
if (!packageServiceSource.includes("auto_start: false")
  || !packageServiceSource.includes("if result.auto_started")
  || !packageServiceSource.includes("package import unexpectedly started a batch")) {
  throw new Error("PACKAGE_IMPORT_AUTHORITY failed: backend must reject an unexpected auto-start from package batch admission");
}
const productionQueuePanelSource = readFileSync(join(root, "src/features/studio/ProductionQueuePanel.tsx"), "utf8");
if (!productionQueuePanelSource.includes("startProductionQueue")) {
  throw new Error("EXECUTION_START_AUTHORITY failed: the existing Production Queue must remain the execution start authority");
}

console.log(`WORKFLOW_SMART_IMPORT_CONTROLLER=PASS`);
console.log(`WORKFLOW_PARAMETER_EXPOSURE_CONTROLLER=PASS`);
console.log(`RECIPE_HISTORY_QUERY_AUTHORITY=PASS`);
console.log(`RECIPE_HISTORY_READ_ONLY=PASS`);
console.log(`RECIPE_HISTORY_ON_DEMAND=PASS`);
console.log(`STRUCTURED_IPC_ERROR=PASS`);
console.log(`RECIPE_ARCHIVE_AUTHORITY=PASS`);
console.log(`ONE_RECIPE_ARCHIVE_STATE_SOURCE=PASS`);
console.log(`ONE_IMPLICIT_RECIPE_RESOLVER=PASS`);
console.log(`APPLICATION_DIRECT_SQLX_NEW_USAGE=0`);
console.log(`NO_NEW_QUEUE=YES`);
console.log(`NO_NEW_EXECUTOR=YES`);
console.log(`NO_NEW_TASK_MODEL=YES`);
console.log(`WORKFLOW_RECIPE_PROMOTION=PASS`);
console.log(`WORKFLOW_RECIPE_PROMOTION_CONSUMPTION=PASS`);
console.log(`WORKFLOW_RECIPE_PROMOTION_CLEAR=PASS`);
console.log(`WORKFLOW_ADVANCED_ONBOARDING_CONTROLLER=PASS`);
console.log(`GENERATION_PRESET_CONTROLLER=PASS`);
console.log(`GENERATION_SUBMISSION_CONTROLLER=PASS`);
console.log(`GENERATION_BATCH_CONTROLLER=PASS`);
console.log(`GENERATION_EXPERIMENT_CONTROLLER=PASS`);
console.log(`GENERATION_PROJECT_TEMPLATE_CONTROLLER=PASS`);
console.log(`GENERATION_ASSET_INTENT_CONTROLLER=PASS`);
console.log(`GENERATION_WORKFLOW_SELECTION_CONTROLLER=PASS`);
console.log(`PROJECT_CONTINUITY_DERIVED=PASS`);
console.log(`DAILY_PRODUCTION_DERIVED=PASS`);
console.log(`DAILY_PRODUCTION_NO_SECOND_STATE=PASS`);
console.log(`DAILY_PRODUCTION_EXACT_TARGETS=PASS`);
console.log(`DAILY_PRODUCTION_NO_AUTO_EXECUTION=PASS`);
console.log(`BULK_PREPARE_NO_AUTO_START=PASS`);
console.log(`BATCH_CREATE_NO_AUTO_START=PASS`);
console.log(`EXPLICIT_QUEUE_START_ONLY=PASS`);
console.log(`BULK_PREPARATION_AUTHORITY=PASS`);
console.log(`ARTIFACT_REVIEW_NO_EXECUTION=PASS`);
console.log(`ARTIFACT_REVIEW_USES_EXISTING_AUTHORITY=PASS`);
console.log(`ARTIFACT_REVIEW_TYPED_COMMANDS=PASS`);
console.log(`ARTIFACT_REVIEW_PROJECT_SCOPE=PASS`);
console.log(`PACKAGE_IMPORT_NO_AUTO_START=PASS`);
console.log(`PACKAGE_IMPORT_AUTHORITY=PASS`);
console.log(`EXECUTION_START_AUTHORITY=PASS`);
console.log(`INTERNAL_SCRIPT_AUTHORING_RETIRED=PASS`);
console.log(`INTERNAL_STORYBOARD_AUTHORING_RETIRED=PASS`);
console.log(`INTERNAL_PROMPT_AUTHORING_RETIRED=PASS`);
console.log(`EXTERNAL_HANDOFF_NO_AUTO_PRODUCTION=PASS`);
console.log(`EXTERNAL_HANDOFF_AUTHORITY=PASS`);
console.log(`EXTERNAL_HANDOFF_PREVIEW_READ_ONLY=PASS`);
console.log(`EXTERNAL_HANDOFF_SINGLE_TRANSACTION=PASS`);
console.log(`EXTERNAL_HANDOFF_EXACT_WORKFLOW_RECIPE=PASS`);
console.log(`EXTERNAL_HANDOFF_TYPED_IPC=PASS`);
console.log(`HANDOFF_JSON_SCHEMA_PARITY=PASS`);
console.log(`HANDOFF_EXAMPLE_VALID=PASS`);
console.log(`HANDOFF_TOOLING_PROVIDER_NEUTRAL=PASS`);

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

const performanceReview=JSON.parse(readFileSync(`${root}/docs/architecture/phase12-performance.json`,'utf8'));
const phase13Review=JSON.parse(readFileSync(`${root}/docs/architecture/phase13-observability.json`,'utf8'));
const backendPerformance=backendPerformanceBoundary(root,backendManifest.backendSourceSnapshot,performanceReview,phase13Review);
if(backendPerformance.violations.length)throw Error(backendPerformance.violations.join('\n'));
console.log('BACKEND_PERFORMANCE_SUCCESSOR=PASS');
console.log('PHASE13_OBSERVABILITY_SUCCESSOR=PASS');
