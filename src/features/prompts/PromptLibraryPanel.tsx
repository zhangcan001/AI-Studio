import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addPromptLibraryVersion,
  createPromptLibraryEntry,
  deletePromptLibraryEntry,
  getPromptLibraryEntry,
  listPromptLibrary,
  updatePromptLibraryMetadata,
} from "../../services/tauriClient";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { PageCursor } from "../../types/asset";
import type { PromptEntryView, PromptKind, PromptVersionView } from "../../types/prompt";
import { toUserMessage } from "../../i18n/errorMessages";
import {
  applyPromptSnippetToStudio,
  applyPromptVersionToStudio,
  comparePromptVersions,
  selectPromptTargetField,
  type PromptSnippetMode,
} from "./promptLibrary";

interface Props {
  projectId: string;
  recipe: RecipeViewModel;
  values: GenerationValues;
  onApplyValues: (values: GenerationValues) => void;
  onUseForExperiment: (fieldKey: string, versions: PromptVersionView[]) => void;
}

export function PromptLibraryPanel({ projectId, recipe, values, onApplyValues, onUseForExperiment }: Props) {
  const textFields = useMemo(() => recipe.fields.filter((field) => field.type === "textarea"), [recipe]);
  const [kind, setKind] = useState<PromptKind>("prompt");
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [tagFilter, setTagFilter] = useState("");
  const [tagQuery, setTagQuery] = useState("");
  const [entries, setEntries] = useState<PromptEntryView[]>([]);
  const [cursor, setCursor] = useState<PageCursor>();
  const [selectedId, setSelectedId] = useState<string>();
  const [detail, setDetail] = useState<PromptEntryView>();
  const [selectedVersionId, setSelectedVersionId] = useState<string>();
  const [compareLeftId, setCompareLeftId] = useState<string>();
  const [compareRightId, setCompareRightId] = useState<string>();
  const [experimentVersionIds, setExperimentVersionIds] = useState<Set<string>>(new Set());
  const [targetFieldKey, setTargetFieldKey] = useState("");
  const [newName, setNewName] = useState("");
  const [newTags, setNewTags] = useState("");
  const [newKind, setNewKind] = useState<PromptKind>("prompt");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();
  const requestVersion = useRef(0);

  useEffect(() => {
    setTargetFieldKey(textFields.length === 1 ? textFields[0].key : "");
    setExperimentVersionIds(new Set());
  }, [recipe, textFields]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setKeyword(keywordInput.trim());
      setTagQuery(tagFilter.trim());
    }, 300);
    return () => window.clearTimeout(timer);
  }, [keywordInput, tagFilter]);

  const loadPage = useCallback(async (requestedCursor: PageCursor | undefined, reset: boolean) => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(undefined);
    try {
      const page = await listPromptLibrary(projectId, {
        kind,
        keyword: keyword || undefined,
        tag: tagQuery || undefined,
        cursor: requestedCursor,
        limit: 30,
      });
      if (requestVersion.current !== version) return;
      setEntries((current) => {
        if (reset) return page.items;
        const byId = new Map(current.map((entry) => [entry.id, entry]));
        page.items.forEach((entry) => byId.set(entry.id, entry));
        return [...byId.values()];
      });
      setCursor(page.nextCursor);
      if (reset) {
        setSelectedId(page.items[0]?.id);
        setDetail(undefined);
      }
    } catch (value: unknown) {
      if (requestVersion.current === version) setError(toUserMessage(value));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [kind, keyword, projectId, tagQuery]);

  useEffect(() => {
    setEntries([]);
    setCursor(undefined);
    setSelectedId(undefined);
    void loadPage(undefined, true);
    return () => { requestVersion.current += 1; };
  }, [kind, keyword, projectId, tagQuery, loadPage]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(undefined);
      return;
    }
    let active = true;
    setLoading(true);
    void getPromptLibraryEntry(projectId, selectedId)
      .then((next) => {
        if (!active) return;
        setDetail(next);
        setSelectedVersionId(next.versions[next.versions.length - 1]?.id);
        setCompareLeftId(next.versions[next.versions.length - 2]?.id ?? next.versions[0]?.id);
        setCompareRightId(next.versions[next.versions.length - 1]?.id);
        setNewName(next.name);
        setNewTags(next.tags.join(", "));
      })
      .catch((value: unknown) => {
        if (active) setError(toUserMessage(value));
      })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [projectId, selectedId]);

  const selectedVersion = detail?.versions.find((version) => version.id === selectedVersionId);
  const compareLeft = detail?.versions.find((version) => version.id === compareLeftId);
  const compareRight = detail?.versions.find((version) => version.id === compareRightId);
  const diff = compareLeft && compareRight ? comparePromptVersions(compareLeft, compareRight) : undefined;
  const targetValue = targetFieldKey ? values[targetFieldKey] : undefined;
  const currentText = targetValue?.type === "string" ? targetValue.value : "";
  const explicitTargetRequired = textFields.length > 1;

  function selectedTarget(): string | undefined {
    const result = selectPromptTargetField(recipe, targetFieldKey || undefined);
    if (result.issue) {
      setNotice(result.issue);
      return undefined;
    }
    return result.fieldKey;
  }

  function parseTags(value: string): string[] {
    return value.split(/[,，]/).map((item) => item.trim()).filter(Boolean);
  }

  async function saveCurrentAsEntry() {
    const fieldKey = selectedTarget();
    if (!fieldKey || !newName.trim()) {
      setError("请输入名称，并选择要保存的文字字段。");
      return;
    }
    setSaving(true); setError(undefined); setNotice(undefined);
    try {
      const created = await createPromptLibraryEntry({
        projectId,
        kind: newKind,
        name: newName,
        tags: parseTags(newTags),
        text: values[fieldKey]?.type === "string" ? values[fieldKey].value : "",
      });
      setEntries((current) => [created, ...current.filter((entry) => entry.id !== created.id)]);
      setSelectedId(created.id);
      setNotice(`${newKind === "prompt" ? "提示词" : "片段"}已保存为 v1。未创建生成任务。`);
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally { setSaving(false); }
  }

  async function saveCurrentAsVersion() {
    const fieldKey = selectedTarget();
    if (!detail || !fieldKey) return;
    setSaving(true); setError(undefined); setNotice(undefined);
    try {
      const version = await addPromptLibraryVersion(projectId, detail.id, currentText);
      setDetail((current) => current ? { ...current, versions: [...current.versions, version], versionCount: current.versionCount + 1, updatedAt: version.createdAt } : current);
      setSelectedVersionId(version.id);
      setNotice(`已追加 v${version.version}。未创建生成任务。`);
    } catch (value: unknown) { setError(toUserMessage(value)); } finally { setSaving(false); }
  }

  async function saveMetadata() {
    if (!detail) return;
    setSaving(true); setError(undefined);
    try {
      const updated = await updatePromptLibraryMetadata({ projectId, promptId: detail.id, name: newName, tags: parseTags(newTags) });
      setDetail((current) => current ? { ...updated, versions: current.versions } : updated);
      setEntries((current) => current.map((entry) => entry.id === updated.id ? { ...entry, ...updated, versions: [] } : entry));
      setNotice("提示词库元数据已更新。");
    } catch (value: unknown) { setError(toUserMessage(value)); } finally { setSaving(false); }
  }

  async function removeEntry() {
    if (!detail || !window.confirm(`确定删除“${detail.name}”及其版本吗？`)) return;
    setSaving(true); setError(undefined);
    try {
      await deletePromptLibraryEntry(projectId, detail.id);
      setEntries((current) => current.filter((entry) => entry.id !== detail.id));
      setSelectedId(undefined); setDetail(undefined); setNotice("提示词库条目已删除。");
    } catch (value: unknown) { setError(toUserMessage(value)); } finally { setSaving(false); }
  }

  function applyVersion(version: PromptVersionView, mode: PromptSnippetMode = "replace") {
    const fieldKey = selectedTarget();
    if (!fieldKey) return;
    if (mode === "replace" && currentText && !window.confirm("目标文字输入已有内容，是否替换？")) return;
    const result = detail?.kind === "snippet"
      ? applyPromptSnippetToStudio(recipe, values, fieldKey, version.text, mode)
      : applyPromptVersionToStudio(recipe, values, fieldKey, version);
    if (!result.values) {
      setError(result.issue ?? "无法应用当前版本。");
      return;
    }
    onApplyValues(result.values);
    setNotice(detail?.kind === "snippet" ? `片段已${mode === "prepend" ? "插入开头" : mode === "append" ? "追加到末尾" : "替换"}；未自动生成。` : "提示词版本已应用到 Studio；未自动生成。");
  }

  function toggleExperimentVersion(versionId: string) {
    setExperimentVersionIds((current) => {
      const next = new Set(current);
      if (next.has(versionId)) next.delete(versionId);
      else if (next.size < 8) next.add(versionId);
      return next;
    });
  }

  function useVersionsForExperiment() {
    const fieldKey = selectedTarget();
    if (!fieldKey || !detail || detail.kind !== "prompt") return;
    const versions = detail.versions.filter((version) => experimentVersionIds.has(version.id));
    if (versions.length < 2 || versions.length > 8) {
      setError("实验必须明确选择 2–8 个提示词版本。");
      return;
    }
    onUseForExperiment(fieldKey, versions);
    setNotice(`已将 ${versions.length} 个版本送入实验规划器；尚未创建任务。`);
  }

  return (
    <details className="prompt-library-panel" open>
      <summary><span><span className="section-label">Prompt Library</span><strong>提示词库与片段</strong></span><small>版本、比较、应用、实验</small></summary>
      <div className="prompt-library-toolbar">
        <label><span>类型</span><select value={kind} onChange={(event) => setKind(event.target.value as PromptKind)}><option value="prompt">Prompt</option><option value="snippet">Snippet</option></select></label>
        <label><span>搜索名称/标签</span><input value={keywordInput} onChange={(event) => setKeywordInput(event.target.value)} placeholder="中文、英文或技术词" /></label>
        <label><span>标签筛选</span><input value={tagFilter} onChange={(event) => setTagFilter(event.target.value)} placeholder="例如：人物" /></label>
      </div>
      <div className="prompt-library-layout">
        <div className="prompt-library-list" aria-label="提示词库条目">
          {loading && <p className="disabled-note">正在加载提示词库...</p>}
          {!loading && !entries.length && <p className="disabled-note">暂无匹配条目。</p>}
          {entries.map((entry) => <button type="button" key={entry.id} className={entry.id === selectedId ? "prompt-library-list-item active" : "prompt-library-list-item"} onClick={() => setSelectedId(entry.id)}><strong>{entry.name}</strong><span>{entry.kind === "prompt" ? "Prompt" : "Snippet"} · {entry.versionCount} 个版本</span><small>{entry.tags.join(" · ") || "无标签"}</small></button>)}
        </div>
        <div className="prompt-library-detail">
          <div className="prompt-library-save-current">
            <strong>保存当前 Studio 文字</strong>
            <div className="prompt-library-form-row"><select value={targetFieldKey} onChange={(event) => setTargetFieldKey(event.target.value)} aria-label="提示词目标字段"><option value="">{explicitTargetRequired ? "多个文字字段，请明确选择" : "选择文字字段"}</option>{textFields.map((field) => <option key={field.key} value={field.key}>{field.label} · {field.key}</option>)}</select><select value={newKind} onChange={(event) => setNewKind(event.target.value as PromptKind)}><option value="prompt">Prompt</option><option value="snippet">Snippet</option></select></div>
            <div className="prompt-library-form-row"><input value={newName} onChange={(event) => setNewName(event.target.value)} maxLength={120} placeholder="名称" /><input value={newTags} onChange={(event) => setNewTags(event.target.value)} placeholder="标签，逗号分隔" /><button type="button" onClick={() => void saveCurrentAsEntry()} disabled={saving || !textFields.length}>保存 v1</button></div>
          </div>
          {detail && (
            <>
              <div className="prompt-library-detail-heading"><div><strong>{detail.name}</strong><span>{detail.kind === "prompt" ? "Prompt" : "Snippet"} · {detail.versionCount} 个版本</span></div><button type="button" className="quiet-button" onClick={() => void removeEntry()} disabled={saving}>删除</button></div>
              <div className="prompt-library-form-row"><input value={newName} onChange={(event) => setNewName(event.target.value)} maxLength={120} aria-label="提示词名称" /><input value={newTags} onChange={(event) => setNewTags(event.target.value)} aria-label="提示词标签" placeholder="标签，逗号分隔" /><button type="button" className="quiet-button" onClick={() => void saveMetadata()} disabled={saving}>更新元数据</button></div>
              <div className="prompt-library-versions"><div className="prompt-library-version-list" aria-label="提示词版本">{detail.versions.map((version) => <button type="button" key={version.id} className={version.id === selectedVersionId ? "active" : ""} onClick={() => setSelectedVersionId(version.id)}>v{version.version}<small>{new Date(version.createdAt).toLocaleString()}</small></button>)}</div><div className="prompt-library-version-actions"><button type="button" onClick={() => void saveCurrentAsVersion()} disabled={saving || !targetFieldKey}>保存当前为新版本</button>{selectedVersion && <><button type="button" className="quiet-button" onClick={() => applyVersion(selectedVersion)} disabled={!targetFieldKey}>应用到 Studio</button>{detail.kind === "snippet" && <><button type="button" className="quiet-button" onClick={() => applyVersion(selectedVersion, "prepend")} disabled={!targetFieldKey}>开头插入</button><button type="button" className="quiet-button" onClick={() => applyVersion(selectedVersion, "append")} disabled={!targetFieldKey}>末尾追加</button></>}</>}</div></div>
              <pre className="prompt-library-version-text">{selectedVersion?.text ?? "请选择版本"}</pre>
              <div className="prompt-library-compare"><strong>版本比较</strong><div className="prompt-library-form-row"><select value={compareLeftId ?? ""} onChange={(event) => setCompareLeftId(event.target.value)}>{detail.versions.map((version) => <option key={version.id} value={version.id}>v{version.version}</option>)}</select><span>→</span><select value={compareRightId ?? ""} onChange={(event) => setCompareRightId(event.target.value)}>{detail.versions.map((version) => <option key={version.id} value={version.id}>v{version.version}</option>)}</select></div>{diff && <div className="prompt-library-diff"><span>删除：{diff.removedLines.join(" / ") || "—"}</span><span>新增：{diff.addedLines.join(" / ") || "—"}</span></div>}</div>
              {detail.kind === "prompt" && <div className="prompt-library-experiment"><strong>用于实验（2–8 个版本）</strong><div className="prompt-library-version-checks">{detail.versions.map((version) => <label key={version.id}><input type="checkbox" checked={experimentVersionIds.has(version.id)} onChange={() => toggleExperimentVersion(version.id)} />v{version.version}</label>)}</div><button type="button" onClick={useVersionsForExperiment} disabled={experimentVersionIds.size < 2 || experimentVersionIds.size > 8 || !targetFieldKey}>送入实验规划器</button></div>}
            </>
          )}
        </div>
      </div>
      {cursor && <button type="button" className="load-more-button" onClick={() => void loadPage(cursor, false)} disabled={loading}>{loading ? "正在加载..." : "加载更多（每页 30 条）"}</button>}
      {error && <p className="error-message" role="alert">提示词库：{error}</p>}
      {notice && <p className="disabled-note" role="status">提示词库：{notice}</p>}
    </details>
  );
}
