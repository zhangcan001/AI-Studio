import { SavedVersionDetailsPane,VersionDiffPane } from "../workflow-lab/LabHistoryPanes";
import { ParameterExposurePane } from "../workflow-lab/LabMappingPane";
import { CompatibilityPane,InputsPane,InspectPane,MetadataPane,OutputsPane,PublishPane,ValidatePane } from "../workflow-lab/LabOnboardingPanes";

import { UiErrorNotice } from "../../i18n/UiErrorNotice";
import { cleanWorkflowStaging, exportWorkflowPackage } from "../../services/workflowLabClient";
import { RecipeHistoryPane } from "../workflows/RecipeHistoryPane";
import { WorkflowCenterOverview } from "../workflows/WorkflowCenterOverview";
import { WorkflowDeleteDialog } from "../workflows/WorkflowDeleteDialog";
import { WorkflowExecutionConfiguration } from "../workflows/WorkflowExecutionConfiguration";
import { WorkflowImportController } from "../workflows/WorkflowImportController";
import { WorkflowRegistryActions } from "../workflows/WorkflowRegistryActions";
import { WorkflowWorkspaceList } from "../workflows/WorkflowWorkspaceList";

import { steps, type useWorkflowLabController } from "../workflows/useWorkflowLabController";
export function WorkflowLabSurface({ controller }: { controller: ReturnType<typeof useWorkflowLabController> }) {
 const { loading, workspaceLoading, checkingAll, importBusyRef, loadWorkspace, smartImportController, recheckAllVersions, importWorkflow, importBackup, centerSummary, productionProfiles, projectId, comfyConnected, projectWorkflowLoading, projectWorkflowError, runtimeProfilesLoading, runtimeProfilesError, onOpenProjectSettings, onOpenStudio, workspaceError, error, notice, selectedQuickTestModelVersionId, setSelectedQuickTestModelVersionId, quickTestModelLoading, quickTestModelOptions, quickTestModelError, draft, onUseInProject, restoreExistingArchivedWorkflow, returnToWorkflowList, items, staging, catalog, search, filter, selectedVersions, quickTestingId, setSearch, setFilter, compareSelected, toggleSelected, setSelectedVersions, quickTest, inspectForDeletion, restoreArchivedWorkflow, openRename, recheckVersion, duplicateRecipe, parameterExposureController, openRecipeHistory, toggleVersion, inspectForPurge, repairBuiltinPackage, setCurrentVersion, deleteVersion, openSavedVersionDetails, promoteRecipe, clearRecipePromotion, archiveRecipe, restoreRecipe, diff, setDiff, savedVersionDetailsLoadingId, savedVersionDetailsError, savedVersionDetails, setSavedVersionDetails, setShowExecutionConfig, showExecutionConfig, onOpenTask, recipeHistoryLoading, recipeHistory, recipeHistoryError, closeRecipeHistory, loadMoreRecipeHistory, advancedController, returnToSmartImport, step, setStep, outputCandidates, deletionTarget, deleting, setDeletionTarget, confirmWorkflowDeletion, renameTarget, renameValue, setRenameValue, setRenameTarget, saveRename } = controller;
 return (
    <section className="workspace-panel workflow-workspace" aria-busy={loading || workspaceLoading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">工作区</span>
          <h2>工作流管理</h2>
          <p className="section-description">导入 ComfyUI API 工作流，配置安全输入后发布工作流运行包。</p>
        </div>
        <WorkflowRegistryActions
          loading={loading}
          workspaceLoading={workspaceLoading}
          checkingAll={checkingAll}
          importBusy={importBusyRef.current}
          onRefresh={() => void loadWorkspace("refresh")}
          onAdd={() => void smartImportController.smartImport()}
          onCheckAll={() => void recheckAllVersions()}
          onManualImport={() => void importWorkflow()}
          onImportBackup={() => void importBackup()}
        />
      </div>

      <WorkflowCenterOverview
        summary={centerSummary}
        profiles={productionProfiles}
        projectId={projectId}
        comfyConnected={comfyConnected}
        workspaceLoading={workspaceLoading}
        projectConfigLoading={projectWorkflowLoading}
        projectConfigError={projectWorkflowError}
        runtimeProfilesLoading={runtimeProfilesLoading}
        runtimeProfilesError={runtimeProfilesError}
        onOpenProjectSettings={onOpenProjectSettings ?? (() => undefined)}
        onManageParameters={(recipe) => void onOpenStudio(recipe.workflowId, recipe.recipeId)}
      />

      <section className="workflow-import-quality" aria-label="工作流导入质量门">
        <div>
          <span className="section-label">导入质量门</span>
          <strong>选择 JSON → 自动识别 → 检查环境 → 确认添加</strong>
          <p>仅支持 ComfyUI “Export API”得到的 JSON；编辑器直接保存的 nodes/links 文件不能导入。</p>
        </div>
        <ul>
          <li>校验 JSON 根结构、节点类型与输入对象</li>
          <li>已连接 ComfyUI 时自动读取 /object_info</li>
          <li>含子图的节点 ID（如 105:11）按当前版本支持；首尾冒号、空段和字母会被拒绝</li>
          <li>不会自动提交 GPU 生成任务，快速测试仍由用户主动触发</li>
        </ul>
      </section>

      {workspaceError && <UiErrorNotice error={workspaceError} />}
      {error && <UiErrorNotice error={error} />}
      {notice && <p className="workflow-notice" role="status">{notice}</p>}

      <section className="model-version-selector" aria-label="快速测试模型版本">
        <h3>高级验证 · Quick Test</h3>
        <label>
          <span>快速测试模型版本（可选）</span>
          <select
            value={selectedQuickTestModelVersionId}
            onChange={(event) => setSelectedQuickTestModelVersionId(event.target.value)}
            disabled={quickTestModelLoading}
          >
            <option value="">不记录模型版本</option>
            {quickTestModelOptions.map((option) => <option key={option.id} value={option.id}>{option.label}</option>)}
          </select>
        </label>
        {quickTestModelLoading && <p className="disabled-note" aria-live="polite">正在加载模型版本…</p>}
        {quickTestModelError && <p className="disabled-note" aria-live="polite">模型版本：{quickTestModelError}</p>}
        {!quickTestModelLoading && !quickTestModelError && !quickTestModelOptions.length && <p className="disabled-note" aria-live="polite">暂无可用模型版本；快速测试仍可创建，但不会记录模型版本。</p>}
      </section>

      <WorkflowImportController
        plan={smartImportController.plan}
        importError={smartImportController.importError}
        draft={draft}
        projectId={projectId}
        loading={loading}
        onResolve={(issue, candidate) => void smartImportController.resolveIssue(issue, candidate)}
        onResume={() => void smartImportController.resume()}
        onOpenAdvanced={() => void smartImportController.openAdvanced()}
        onOpenExisting={() => void smartImportController.openExisting()}
        onOpenExistingVersion={() => void smartImportController.openExistingVersion()}
        onUseInProject={(workflowId, recipeId) => void onUseInProject(workflowId, recipeId)}
        onRegenerateRecipe={() => void smartImportController.regenerateRecipe()}
        onRestoreExisting={() => void restoreExistingArchivedWorkflow()}
        onCommitImport={(action) => void smartImportController.commit(action)}
        onSaveReviewMetadata={(metadata) => void smartImportController.updateReviewMetadata(metadata)}
        onOpenStudio={(workflowId, recipeId) => void onOpenStudio(workflowId, recipeId)}
        onRetry={() => void smartImportController.smartImport()}
        onReturnToList={() => void returnToWorkflowList()}
      />

      <WorkflowWorkspaceList
        items={items}
        staging={staging}
        catalog={catalog}
        projectId={projectId}
        search={search}
        filter={filter}
        selectedVersions={selectedVersions}
        workspaceLoading={workspaceLoading}
        quickTestingId={quickTestingId}
        onSearchChange={setSearch}
        onFilterChange={setFilter}
        onCompareSelected={() => void compareSelected()}
        onToggleSelected={toggleSelected}
        onToggleVersionSelection={(workflowVersionId) => setSelectedVersions((current) => current.includes(workflowVersionId) ? current.filter((id) => id !== workflowVersionId) : current.length < 2 ? [...current, workflowVersionId] : [current[1], workflowVersionId])}
        onUseInProject={(workflowId, recipeId) => void onUseInProject(workflowId, recipeId)}
        onQuickTest={(item) => void quickTest(item)}
        onInspectForDeletion={(item) => void inspectForDeletion(item)}
        onRestore={(item) => void restoreArchivedWorkflow(item)}
        onRename={openRename}
        onReidentify={(item) => void smartImportController.reidentify(item)}
        onRecheck={(item) => void recheckVersion(item)}
        onDuplicateRecipe={(item) => void duplicateRecipe(item)}
        onOpenParameters={(item) => void parameterExposureController.open(item)}
        onViewHistory={(item, recipe) => void openRecipeHistory(item, recipe)}
        onExport={(item) => { if (item.currentVersionId) void exportWorkflowPackage(item.currentVersionId); }}
        onToggle={(item) => void toggleVersion(item)}
        onPurge={(item) => void inspectForPurge(item)}
        onRepairBuiltinPackage={(item) => void repairBuiltinPackage(item)}
        onSetCurrentVersion={(item, version) => void setCurrentVersion(item, version)}
        onDeleteVersion={(item, version) => void deleteVersion(item, version)}
        onViewSavedVersion={(workflowVersionId) => void openSavedVersionDetails(workflowVersionId)}
        onPromoteRecipe={(item, recipe) => void promoteRecipe(item, recipe)}
        onClearPromotion={(item, recipe) => void clearRecipePromotion(item, recipe)}
        onArchiveRecipe={(item, recipe) => void archiveRecipe(item, recipe)}
        onRestoreRecipe={(item, recipe) => void restoreRecipe(item, recipe)}
        onCleanStaging={(stagingId) => void cleanWorkflowStaging(stagingId)}
      />
      {diff && <VersionDiffPane diff={diff} onClose={() => setDiff(undefined)} />}

      {savedVersionDetailsLoadingId && <p className="loading-state" role="status">正在从工作流库读取版本 {savedVersionDetailsLoadingId}…</p>}
      {savedVersionDetailsError && <p className="error-message" role="alert">{savedVersionDetailsError}</p>}
      {savedVersionDetails && <SavedVersionDetailsPane details={savedVersionDetails} onClose={() => { setSavedVersionDetails(undefined); setShowExecutionConfig(false); }} onRun={() => setShowExecutionConfig(true)} />}
      {savedVersionDetails && showExecutionConfig && <WorkflowExecutionConfiguration
        key={savedVersionDetails.workflowVersionId}
        details={savedVersionDetails}
        catalog={catalog}
        projectId={projectId}
        comfyConnected={comfyConnected}
        onOpenTask={onOpenTask}
      />}

      {parameterExposureController.draft && parameterExposureController.item && (
        <ParameterExposurePane
          draft={parameterExposureController.draft}
          workflow={parameterExposureController.item}
          originalKeys={parameterExposureController.originalKeys}
          loading={parameterExposureController.loading}
          onClose={() => void parameterExposureController.close()}
          onRefresh={() => void parameterExposureController.refreshCapability()}
          onExpose={(nodeId, input) => void parameterExposureController.exposeParameter(nodeId, input)}
          onSaveMapping={(mapping, nodeId, inputName) => void parameterExposureController.saveMapping(mapping, nodeId, inputName)}
          onRemove={(mapping) => void parameterExposureController.removeMapping(mapping)}
          onSave={(edits) => void parameterExposureController.publish(edits)}
        />
      )}

      {recipeHistoryLoading && !recipeHistory && <p className="loading-state" role="status">正在读取 Recipe 历史…</p>}
      {recipeHistoryError && <p className="error-message" role="alert">{recipeHistoryError}</p>}
      {recipeHistory && <RecipeHistoryPane history={recipeHistory} loading={recipeHistoryLoading} onClose={closeRecipeHistory} onLoadMore={() => void loadMoreRecipeHistory()} onOpenTask={onOpenTask} />}

      {advancedController.showAdvanced && draft && (
        <div className="workflow-onboarding-panel">
          <div className="workflow-onboarding-heading">
            <div>
              <span className="section-label">高级工作流编辑</span>
              <h3>{draft.manifest.name}</h3>
              <p className="section-description">{draft.originalFilename} · {draft.nodeCount} 个节点 · {draft.uniqueClassCount} 种节点类型</p>
            </div>
            <div className="workflow-smart-actions">
              <button type="button" className="quiet-button" onClick={() => void returnToSmartImport()}>返回智能导入</button>
              <button type="button" className="quiet-button" onClick={() => void advancedController.discardDraft()} disabled={loading}>丢弃草稿</button>
            </div>
          </div>
          <div className="workflow-step-tabs" role="tablist" aria-label="工作流导入步骤">
            {steps.map((item) => (
              <button
                type="button"
                role="tab"
                key={item.value}
                aria-selected={step === item.value}
                className={step === item.value ? "workflow-step-active" : ""}
                onClick={() => setStep(item.value)}
              >
                {item.label}
              </button>
            ))}
          </div>

          {step === "inspect" && <InspectPane draft={draft} onContinue={() => setStep("compatibility")} />}
          {step === "compatibility" && (
            <CompatibilityPane draft={draft} loading={loading} onCheck={() => void advancedController.checkCapability()} onContinue={() => setStep("inputs")} />
          )}
          {step === "inputs" && (
            <InputsPane
              draft={draft}
              mappingDrafts={advancedController.mappingDrafts}
              onPatch={advancedController.patchMapping}
              onBind={(nodeId, input) => void advancedController.bindInput(nodeId, input)}
              onRemove={(mapping) => void advancedController.removeInput(mapping)}
              onContinue={() => setStep("outputs")}
            />
          )}
          {step === "outputs" && (
            <OutputsPane
              draft={draft}
              candidates={outputCandidates.length ? outputCandidates : draft.nodes}
              outputDraft={advancedController.outputDraft}
              onChange={advancedController.setOutputDraft}
              onAdd={() => void advancedController.addOutput()}
              onContinue={() => setStep("metadata")}
            />
          )}
          {step === "metadata" && advancedController.metadataDraft && (
            <MetadataPane draft={advancedController.metadataDraft} onChange={advancedController.setMetadataDraft} onSave={() => void advancedController.saveMetadata()} onContinue={() => setStep("validate")} />
          )}
          {step === "validate" && (
            <ValidatePane draft={draft} loading={loading} onValidate={() => void advancedController.validateDraft()} onPublish={() => setStep("publish")} />
          )}
          {step === "publish" && (
            <PublishPane
              draft={draft}
              published={advancedController.published}
              loading={loading}
              onPublish={() => void advancedController.publishDraft()}
              onOpenStudio={advancedController.published ? () => void onOpenStudio(advancedController.published!.workflowId, advancedController.published!.recipeId) : undefined}
            />
          )}
        </div>
      )}

      {deletionTarget && (
        <WorkflowDeleteDialog
          item={deletionTarget.item}
          inspection={deletionTarget.inspection}
          mode={deletionTarget.mode}
          deleting={deleting}
          onClose={() => setDeletionTarget(undefined)}
          onConfirm={() => void confirmWorkflowDeletion()}
        />
      )}
      {renameTarget && (
        <div className="workflow-rename-dialog" role="dialog" aria-modal="true" aria-label="重命名工作流">
          <div className="workflow-rename-card">
            <h3>重命名工作流</h3>
            <input aria-label="工作流名称" value={renameValue} onChange={(event) => setRenameValue(event.target.value)} autoFocus />
            <div className="workflow-smart-actions">
              <button type="button" className="quiet-button" onClick={() => setRenameTarget(undefined)}>取消</button>
              <button type="button" onClick={() => void saveRename()} disabled={!renameValue.trim()}>保存</button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
