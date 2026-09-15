import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getPromptLibraryEntry,
  listModelVersions,
  listModels,
  listPromptLibrary,
  listGenerationAssetVersionLinks,
  listGenerationToolUsages,
  taskHistoryPage,
} from "../../services/tauriClient";
import { formatDateTime, taskStatusLabel } from "../../i18n/statusLabels";
import { toUserMessage } from "../../i18n/errorMessages";
import type { ModelVersionView, ModelView } from "../../types/model";
import type { PromptEntryView, PromptKind } from "../../types/prompt";
import type { PageCursor } from "../../types/asset";
import type { TaskHistoryItem } from "../../types/history";
import type { GenerationAssetVersionView, GenerationToolUsageView } from "../../types/provenance";

interface Props {
  projectId: string;
  onOpenTaskHistory?: () => void;
}

interface PromptGenerationProvenance {
  task: TaskHistoryItem;
  toolUsages: GenerationToolUsageView[];
  assetVersionLinks: GenerationAssetVersionView[];
}

const promptKindLabels: Record<PromptKind, string> = {
  prompt: "提示词",
  snippet: "片段",
};

function latestVersion(entry?: PromptEntryView): PromptEntryView["versions"][number] | undefined {
  if (!entry?.versions.length) return undefined;
  return entry.versions[entry.versions.length - 1];
}

function formatJson(value: unknown): string {
  if (value === undefined || value === null) return "暂无记录";
  try {
    return JSON.stringify(value, null, 2) ?? "暂无记录";
  } catch {
    return "数据暂不可读取";
  }
}

function mergeEntries(current: readonly PromptEntryView[], next: readonly PromptEntryView[], reset: boolean): PromptEntryView[] {
  if (reset) return [...next];
  const byId = new Map(current.map((entry) => [entry.id, entry]));
  next.forEach((entry) => byId.set(entry.id, entry));
  return [...byId.values()];
}

export function PromptStudio({ projectId, onOpenTaskHistory }: Props) {
  const [kind, setKind] = useState<PromptKind>("prompt");
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [tagInput, setTagInput] = useState("");
  const [tag, setTag] = useState("");
  const [entries, setEntries] = useState<PromptEntryView[]>([]);
  const [detailsById, setDetailsById] = useState<Record<string, PromptEntryView>>({});
  const [cursor, setCursor] = useState<PageCursor>();
  const [selectedPromptId, setSelectedPromptId] = useState<string>();
  const [selectedVersionId, setSelectedVersionId] = useState<string>();
  const [models, setModels] = useState<ModelView[]>([]);
  const [versionsByModel, setVersionsByModel] = useState<Record<string, ModelVersionView[]>>({});
  const [inspectedModelId, setInspectedModelId] = useState<string>();
  const [history, setHistory] = useState<TaskHistoryItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [modelLoading, setModelLoading] = useState(false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [generationProvenanceById, setGenerationProvenanceById] = useState<Record<string, PromptGenerationProvenance>>({});
  const [provenanceLoading, setProvenanceLoading] = useState(false);
  const [provenanceError, setProvenanceError] = useState<string>();
  const [error, setError] = useState<string>();
  const [detailError, setDetailError] = useState<string>();
  const [detailErrorsById, setDetailErrorsById] = useState<Record<string, string>>({});
  const [modelError, setModelError] = useState<string>();
  const [historyError, setHistoryError] = useState<string>();
  const promptRequest = useRef(0);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setKeyword(keywordInput.trim());
      setTag(tagInput.trim());
    }, 250);
    return () => window.clearTimeout(timer);
  }, [keywordInput, tagInput]);

  const loadPromptDetail = useCallback(async (promptId: string) => {
    setDetailLoading(true);
    setDetailError(undefined);
    try {
      const detail = await getPromptLibraryEntry(projectId, promptId);
      setDetailsById((current) => ({ ...current, [detail.id]: detail }));
      setDetailError(undefined);
      setDetailErrorsById((current) => {
        const next = { ...current };
        delete next[promptId];
        return next;
      });
    } catch (value: unknown) {
      const message = toUserMessage(value);
      setDetailError(message);
      setDetailErrorsById((current) => ({ ...current, [promptId]: message }));
    } finally {
      setDetailLoading(false);
    }
  }, [projectId]);

  const loadPromptPage = useCallback(async (requestedCursor: PageCursor | undefined, reset: boolean) => {
    const request = ++promptRequest.current;
    setLoading(true);
    setError(undefined);
    setDetailError(undefined);
    try {
      const page = await listPromptLibrary(projectId, {
        kind,
        keyword: keyword || undefined,
        tag: tag || undefined,
        cursor: requestedCursor,
        limit: 30,
      });
      if (promptRequest.current !== request) return;
      setEntries((current) => mergeEntries(current, page.items, reset));
      setCursor(page.nextCursor);
      if (reset) {
        setSelectedPromptId(page.items[0]?.id);
        setSelectedVersionId(undefined);
        setDetailsById({});
      }

      const detailResults = await Promise.allSettled(
        page.items.map((entry) => getPromptLibraryEntry(projectId, entry.id)),
      );
      if (promptRequest.current !== request) return;
      const firstDetailError = detailResults.find((item) => item.status === "rejected");
      if (firstDetailError?.status === "rejected") setDetailError(toUserMessage(firstDetailError.reason));
      const details: Record<string, PromptEntryView> = {};
      detailResults.forEach((item) => {
        if (item.status === "fulfilled") details[item.value.id] = item.value;
      });
      setDetailsById((current) => (reset ? details : { ...current, ...details }));
      setDetailErrorsById((current) => {
        const next = reset ? {} : { ...current };
        detailResults.forEach((item, index) => {
          const entry = page.items[index];
          if (!entry) return;
          if (item.status === "fulfilled") delete next[entry.id];
          else next[entry.id] = toUserMessage(item.reason);
        });
        return next;
      });
    } catch (value: unknown) {
      if (promptRequest.current === request) setError(toUserMessage(value));
    } finally {
      if (promptRequest.current === request) setLoading(false);
    }
  }, [kind, keyword, projectId, tag]);

  useEffect(() => {
    setEntries([]);
    setDetailsById({});
    setDetailErrorsById({});
    setCursor(undefined);
    setSelectedPromptId(undefined);
    setSelectedVersionId(undefined);
    void loadPromptPage(undefined, true);
    return () => {
      promptRequest.current += 1;
    };
  }, [loadPromptPage]);

  useEffect(() => {
    let active = true;
    setModelLoading(true);
    setModelError(undefined);
    void listModels()
      .then(async (nextModels) => {
        if (!active) return;
        setModels(nextModels);
        const versionResults = await Promise.allSettled(nextModels.map((model) => listModelVersions(model.id)));
        if (!active) return;
        const nextVersions = versionResults.reduce<Record<string, ModelVersionView[]>>((result, item, index) => {
          if (item.status === "fulfilled") result[nextModels[index].id] = item.value;
          return result;
        }, {});
        setVersionsByModel(nextVersions);
        setInspectedModelId((current) => nextModels.some((model) => model.id === current) ? current : nextModels[0]?.id);
      })
      .catch((value: unknown) => {
        if (active) setModelError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setModelLoading(false);
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  useEffect(() => {
    let active = true;
    const visibleHistory = history.slice(0, 6);
    setGenerationProvenanceById({});
    setProvenanceError(undefined);
    if (!visibleHistory.length) {
      setProvenanceLoading(false);
      return () => { active = false; };
    }
    setProvenanceLoading(true);
    void Promise.all(
      visibleHistory.map(async (task): Promise<PromptGenerationProvenance> => {
        const [toolUsages, assetVersionLinks] = await Promise.all([
          listGenerationToolUsages(projectId, task.id),
          listGenerationAssetVersionLinks(projectId, task.id),
        ]);
        return { task, toolUsages, assetVersionLinks };
      }),
    )
      .then((records) => {
        if (!active) return;
        setGenerationProvenanceById(Object.fromEntries(records.map((record) => [record.task.id, record])));
      })
      .catch((value: unknown) => {
        if (active) setProvenanceError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setProvenanceLoading(false);
      });
    return () => { active = false; };
  }, [history, projectId]);

  useEffect(() => {
    let active = true;
    setHistory([]);
    setGenerationProvenanceById({});
    setProvenanceError(undefined);
    setHistoryLoading(true);
    setHistoryError(undefined);
    void taskHistoryPage({ projectId, filter: "ALL", timeFilter: "ALL", limit: 20 })
      .then((page) => {
        if (active) setHistory(page.items);
      })
      .catch((value: unknown) => {
        if (active) setHistoryError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setHistoryLoading(false);
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  const selectedDetail = selectedPromptId ? detailsById[selectedPromptId] : undefined;
  const selectedVersion = selectedDetail?.versions.find((version) => version.id === selectedVersionId)
    ?? latestVersion(selectedDetail);
  const selectedPromptDetailError = selectedPromptId ? detailErrorsById[selectedPromptId] : detailError;
  const modelById = useMemo(() => new Map(models.map((model) => [model.id, model])), [models]);
  const modelVersionById = useMemo(() => {
    const result = new Map<string, ModelVersionView>();
    Object.values(versionsByModel).forEach((versions) => versions.forEach((version) => result.set(version.id, version)));
    return result;
  }, [versionsByModel]);
  const linkedModelVersion = selectedVersion?.modelVersionId
    ? modelVersionById.get(selectedVersion.modelVersionId)
    : undefined;
  const linkedModel = linkedModelVersion ? modelById.get(linkedModelVersion.modelId) : undefined;
  const inspectedModel = inspectedModelId ? modelById.get(inspectedModelId) : undefined;
  const inspectedVersions = inspectedModelId ? versionsByModel[inspectedModelId] ?? [] : [];
  const visibleProvenanceRecords = useMemo(
    () => history.slice(0, 6).map((task) => generationProvenanceById[task.id]).filter((record): record is PromptGenerationProvenance => Boolean(record)),
    [generationProvenanceById, history],
  );
  const toolUsageRecords = useMemo(
    () => visibleProvenanceRecords.flatMap((record) => record.toolUsages.map((usage) => ({ task: record.task, usage }))),
    [visibleProvenanceRecords],
  );
  const assetVersionLineageRecords = useMemo(
    () => visibleProvenanceRecords.flatMap((record) => record.assetVersionLinks.map((link) => ({ task: record.task, link }))),
    [visibleProvenanceRecords],
  );

  useEffect(() => {
    if (linkedModelVersion?.modelId) {
      setInspectedModelId(linkedModelVersion.modelId);
    } else if (!inspectedModelId && models.length) {
      setInspectedModelId(models[0].id);
    }
  }, [inspectedModelId, linkedModelVersion, models]);

  useEffect(() => {
    const versions = selectedDetail?.versions ?? [];
    if (!versions.some((version) => version.id === selectedVersionId)) {
      setSelectedVersionId(latestVersion(selectedDetail)?.id);
    }
  }, [selectedDetail, selectedVersionId]);

  function selectPrompt(promptId: string) {
    setSelectedPromptId(promptId);
    const detail = detailsById[promptId];
    setDetailError(undefined);
    setSelectedVersionId(latestVersion(detail)?.id);
    if (!detail) void loadPromptDetail(promptId);
  }

  function modelLabel(entry: PromptEntryView): string {
    if (detailErrorsById[entry.id]) return "加载失败";
    if (!detailsById[entry.id] && loading) return "读取中…";
    const version = latestVersion(detailsById[entry.id]);
    if (!version?.modelVersionId) return "未绑定模型";
    const modelVersion = modelVersionById.get(version.modelVersionId);
    const model = modelVersion ? modelById.get(modelVersion.modelId) : undefined;
    if (model && modelVersion) return `${model.provider} / ${model.name} · ${modelVersion.version}`;
    if (modelLoading) return "读取中…";
    if (modelError) return "模型加载失败";
    return "模型版本缺失";
  }

  return (
    <section className="workspace-panel prompt-studio-workspace" aria-label="Prompt Studio 提示词工作台" aria-busy={loading || detailLoading || modelLoading || historyLoading || provenanceLoading}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">v2 工作台</span>
          <h2>提示词工作台</h2>
          <p className="section-description">管理提示词版本、模型参数和生成来源；这里不会自动提交生成任务。</p>
        </div>
        <div className="prompt-studio-project-scope" aria-label={`当前项目：${projectId}`}>
          <span>当前项目</span>
          <strong>{projectId}</strong>
        </div>
      </div>

      <div className="prompt-studio-toolbar" aria-label="提示词筛选">
        <label>
          <span>类型</span>
          <select aria-label="提示词类型" value={kind} onChange={(event) => setKind(event.target.value as PromptKind)}>
            <option value="prompt">提示词</option>
            <option value="snippet">片段</option>
          </select>
        </label>
        <label>
          <span>搜索名称或标签</span>
          <input aria-label="搜索名称或标签" value={keywordInput} onChange={(event) => setKeywordInput(event.target.value)} placeholder="输入关键词" />
        </label>
        <label>
          <span>标签</span>
          <input aria-label="提示词标签" value={tagInput} onChange={(event) => setTagInput(event.target.value)} placeholder="例如：人物" />
        </label>
      </div>

      {error && <p className="error-message" role="alert">提示词列表加载失败：{error}</p>}

      <div className="prompt-studio-layout">
        <section className="prompt-studio-list-panel" aria-label="提示词列表">
          <div className="prompt-studio-panel-heading">
            <div><span className="section-label">提示词库</span><h3>提示词列表</h3></div>
            <span className="status-pill">{entries.length} 条</span>
          </div>
          {loading && !entries.length && <p className="disabled-note" role="status">正在加载提示词…</p>}
          {!loading && !entries.length && <p className="empty-state">当前项目暂无提示词。</p>}
          {entries.length > 0 && (
            <div className="prompt-studio-table-wrap">
              <table className="prompt-studio-table">
                <caption className="sr-only">提示词列表</caption>
                <thead><tr><th scope="col">名称</th><th scope="col">类型</th><th scope="col">当前版本</th><th scope="col">模型</th><th scope="col">更新时间</th></tr></thead>
                <tbody>
                  {entries.map((entry) => (
                    <tr key={entry.id} className={entry.id === selectedPromptId ? "active" : undefined}>
                      <th scope="row"><button type="button" className="prompt-studio-row-button" onClick={() => selectPrompt(entry.id)} aria-pressed={entry.id === selectedPromptId}>{entry.name}</button><small>{entry.tags.join(" · ") || "无标签"}</small></th>
                      <td>{promptKindLabels[entry.kind]}</td>
                      <td>{entry.versionCount > 0 ? `v${entry.versionCount}` : "—"}</td>
                      <td>{modelLabel(entry)}</td>
                      <td>{formatDateTime(entry.updatedAt)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {cursor && <button type="button" className="load-more-button" onClick={() => void loadPromptPage(cursor, false)} disabled={loading}>{loading ? "正在加载…" : "加载更多"}</button>}
        </section>

        <div className="prompt-studio-detail-column">
          <section className="prompt-studio-detail-panel" aria-label="提示词详情">
            <div className="prompt-studio-panel-heading">
              <div><span className="section-label">当前条目</span><h3>{selectedDetail?.name ?? "提示词详情"}</h3></div>
              {selectedDetail && <span className="status-pill">{promptKindLabels[selectedDetail.kind]}</span>}
            </div>
            {!selectedDetail && (detailLoading || loading) && <p className="disabled-note" role="status">正在读取提示词详情…</p>}
            {!selectedDetail && !detailLoading && !loading && entries.length > 0 && <p className="empty-state">请选择一个提示词查看详情。</p>}
            {selectedPromptDetailError && (
              <div>
                <p className="error-message" role="alert">提示词详情加载失败：{selectedPromptDetailError}</p>
                {selectedPromptId && <button type="button" className="quiet-button" onClick={() => void loadPromptDetail(selectedPromptId)} disabled={detailLoading}>{detailLoading ? "正在重试…" : "重试读取详情"}</button>}
              </div>
            )}
            {selectedDetail && (
              <>
                <div className="prompt-studio-version-layout">
                  <nav className="prompt-studio-version-history" aria-label="提示词版本历史">
                    <strong>版本历史</strong>
                    {[...selectedDetail.versions].reverse().map((version) => (
                      <button type="button" key={version.id} className={version.id === selectedVersion?.id ? "active" : undefined} onClick={() => setSelectedVersionId(version.id)} aria-pressed={version.id === selectedVersion?.id}>
                        <span>v{version.version}</span>
                        <small>{formatDateTime(version.createdAt)}</small>
                      </button>
                    ))}
                  </nav>
                  <div className="prompt-studio-detail-content">
                    <section className="prompt-studio-subpanel" aria-label="提示词模板">
                      <div className="prompt-studio-panel-heading"><h4>提示词模板</h4>{selectedVersion && <span className="status-pill">v{selectedVersion.version}</span>}</div>
                      <pre className="prompt-studio-template">{selectedVersion?.text ?? "暂无模板内容"}</pre>
                    </section>
                    <dl className="prompt-studio-metadata">
                      <div><dt>版本</dt><dd>{selectedVersion ? `v${selectedVersion.version}` : "—"}</dd></div>
                      <div><dt>更新时间</dt><dd>{selectedVersion ? formatDateTime(selectedVersion.createdAt) : "—"}</dd></div>
                      <div><dt>模型版本</dt><dd>{!selectedVersion?.modelVersionId ? "未绑定模型版本" : linkedModel && linkedModelVersion ? `${linkedModel.provider} / ${linkedModel.name} · ${linkedModelVersion.version}` : modelLoading ? "读取中…" : modelError ? "模型加载失败" : "模型版本缺失"}</dd></div>
                    </dl>
                    <div className="prompt-studio-two-column">
                      <section className="prompt-studio-subpanel" aria-label="参数">
                        <h4>参数</h4>
                        {linkedModelVersion ? <pre className="prompt-studio-json">{formatJson(linkedModelVersion.parameterSchema)}</pre> : <p className="empty-state">当前版本未记录参数规范。</p>}
                      </section>
                      <section className="prompt-studio-subpanel" aria-label="参考素材">
                        <h4>参考素材</h4>
                        <p className="empty-state">当前版本未记录参考素材关联。</p>
                      </section>
                    </div>
                    <section className="prompt-studio-subpanel" aria-label="生成历史">
                      <div className="prompt-studio-panel-heading"><h4>生成历史</h4>{onOpenTaskHistory && <button type="button" className="quiet-button" onClick={onOpenTaskHistory}>打开任务历史</button>}</div>
                      {historyLoading && <p className="disabled-note" role="status">正在读取生成历史…</p>}
                      {historyError && <p className="error-message" role="alert">生成历史加载失败：{historyError}</p>}
                      {!historyLoading && !historyError && !history.length && <p className="empty-state">当前项目暂无生成任务。</p>}
                      {!historyLoading && !historyError && history.length > 0 && <ul className="prompt-studio-history-list">{history.slice(0, 6).map((task) => <li key={task.id}><strong>{task.workflowName}</strong><span>{taskStatusLabel(task.status)} · {formatDateTime(task.createdAt)} · 输出 {task.outputCount} 个</span></li>)}</ul>}
                      <p className="prompt-studio-note">任务历史和结果资产继续由现有 Task / Result 权威提供；本页面只读展示，不会自动创建任务。</p>
                    </section>
                    <section className="prompt-studio-subpanel" aria-label="Prompt Studio 跨模块溯源">
                      <div className="prompt-studio-panel-heading"><h4>跨模块溯源</h4><span className="status-pill">只读</span></div>
                      <p className="prompt-studio-note">只有带有显式 PromptVersionId 的任务才会建立提示词归因；旧记录不会根据文本、名称或时间推断。</p>
                      {historyLoading && <p className="disabled-note" role="status">等待生成历史后读取溯源…</p>}
                      {!historyLoading && provenanceLoading && <p className="disabled-note" role="status">正在读取跨模块溯源…</p>}
                      {provenanceError && <p className="error-message" role="alert">跨模块溯源加载失败：{provenanceError}</p>}
                      {!historyLoading && !provenanceLoading && !provenanceError && (
                        <div className="prompt-studio-two-column">
                          <section className="prompt-studio-subpanel" aria-label="相关生成">
                            <h4>相关生成</h4>
                            <p className="prompt-studio-note">以下是当前项目任务历史参考；只有任务保存了当前提示词版本 ID 时，才视为当前提示词的直接使用。</p>
                            {history.length > 0 ? (
                              <ul className="prompt-studio-history-list">
                                {history.slice(0, 6).map((task) => <li key={task.id}><strong>{task.id}</strong><span>{task.workflowName} · {taskStatusLabel(task.status)} · {formatDateTime(task.createdAt)}</span></li>)}
                              </ul>
                            ) : <p className="empty-state">暂无生成记录。</p>}
                          </section>
                          <section className="prompt-studio-subpanel" aria-label="生成资产">
                            <h4>生成资产</h4>
                            {assetVersionLineageRecords.length > 0 ? (
                              <ul className="prompt-studio-history-list">
                                {assetVersionLineageRecords.map(({ task, link }) => <li key={link.id}><strong>{link.assetVersionId}</strong><span>{task.id} · {link.relationType} · 输出 {link.outputId} · 第 {link.ordinal + 1} 项</span></li>)}
                              </ul>
                            ) : history.some((task) => task.outputCount > 0) ? (
                              <p className="empty-state">任务有输出，但尚未建立生成任务 → 资产版本显式关系。</p>
                            ) : <p className="empty-state">暂无已建立的结果资产溯源。</p>}
                          </section>
                          <section className="prompt-studio-subpanel" aria-label="模型版本">
                            <h4>模型版本</h4>
                            {selectedVersion?.modelVersionId ? (
                              <ul className="prompt-studio-history-list"><li><strong>{linkedModel && linkedModelVersion ? `${linkedModel.provider} / ${linkedModel.name}` : selectedVersion.modelVersionId}</strong><span>提示词版本 v{selectedVersion.version} · {linkedModelVersion?.version ?? "历史版本未加载"}</span></li></ul>
                            ) : <p className="empty-state">当前提示词版本未记录模型版本关联。</p>}
                          </section>
                          <section className="prompt-studio-subpanel" aria-label="工具使用">
                            <h4>工具使用</h4>
                            {toolUsageRecords.length > 0 ? (
                              <ul className="prompt-studio-history-list">
                                {toolUsageRecords.map(({ task, usage }) => <li key={usage.id}><strong>{usage.toolVersionId ?? "工具版本未记录"}</strong><span>{task.id} · 实例 {usage.toolInstanceId} · {formatDateTime(usage.createdAt)}</span></li>)}
                              </ul>
                            ) : <p className="empty-state">暂无显式工具使用记录；历史生成可能未保存工具关系。</p>}
                          </section>
                        </div>
                      )}
                    </section>
                    <section className="prompt-studio-subpanel" aria-label="生成来源">
                      <h4>生成来源</h4>
                      <ol className="prompt-studio-provenance">
                        <li><strong>提示词版本</strong><span>{selectedVersion ? `v${selectedVersion.version}` : "未选择"}</span></li>
                        <li><strong>模型版本</strong><span>{linkedModelVersion ? `${linkedModel?.name ?? "未知模型"} · ${linkedModelVersion.version}` : "未绑定模型版本"}</span></li>
                        <li><strong>生成快照</strong><span>{history.length ? "由现有任务历史保留" : "尚无直接快照"}</span></li>
                        <li><strong>结果资产</strong><span>{history.some((task) => task.outputCount > 0) ? "可从任务历史查看输出" : "尚无结果资产"}</span></li>
                      </ol>
                    </section>
                  </div>
                </div>
              </>
            )}
          </section>

          <section className="prompt-studio-model-panel" aria-label="模型视图">
            <div className="prompt-studio-panel-heading">
              <div><span className="section-label">模型注册表</span><h3>模型视图</h3></div>
              {models.length > 0 && <span className="status-pill">{models.length} 个模型</span>}
            </div>
            {modelLoading && <p className="disabled-note" role="status">正在加载模型注册表…</p>}
            {modelError && <p className="error-message" role="alert">模型加载失败：{modelError}</p>}
            {!modelLoading && !modelError && !models.length && <p className="empty-state">暂无模型注册记录。</p>}
            {models.length > 0 && (
              <>
                <label className="prompt-studio-model-picker"><span>查看模型</span><select aria-label="查看模型" value={inspectedModelId ?? ""} onChange={(event) => setInspectedModelId(event.target.value)}>{models.map((model) => <option key={model.id} value={model.id}>{model.provider} / {model.name}</option>)}</select></label>
                {inspectedModel && (
                  <div className="prompt-studio-model-detail">
                    <dl className="prompt-studio-metadata">
                      <div><dt>模型</dt><dd>{inspectedModel.name}</dd></div>
                      <div><dt>提供方</dt><dd>{inspectedModel.provider}</dd></div>
                      <div><dt>类型</dt><dd>{inspectedModel.type}</dd></div>
                      <div><dt>说明</dt><dd>{inspectedModel.description || "—"}</dd></div>
                    </dl>
                    <div className="prompt-studio-model-versions"><h4>模型版本</h4>{!inspectedVersions.length && <p className="empty-state">暂无模型版本。</p>}{inspectedVersions.map((version) => <article key={version.id} className="prompt-studio-model-version"><div className="prompt-studio-panel-heading"><strong>v{version.version}</strong><small>{formatDateTime(version.createdAt)}</small></div><div className="prompt-studio-two-column"><div><span className="prompt-studio-field-label">能力</span><pre className="prompt-studio-json">{formatJson(version.capabilities)}</pre></div><div><span className="prompt-studio-field-label">参数规范</span><pre className="prompt-studio-json">{formatJson(version.parameterSchema)}</pre></div></div></article>)}</div>
                  </div>
                )}
              </>
            )}
          </section>
        </div>
      </div>
    </section>
  );
}
