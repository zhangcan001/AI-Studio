import type {
  ReadinessCheckView,
  ReadinessGateView,
  ResolvedProfileView,
  ShotProductionPlanDetail,
} from "../../types/productionPreparation";
import {
  preparationGateLabel,
  preparationStatusLabel,
  readinessStateLabel,
} from "../../types/productionPreparation";

export interface ShotReadinessInspectorProps {
  detail?: ShotProductionPlanDetail;
  loading?: boolean;
  error?: string;
  onRetry?: () => void;
}

const GATE_KEYS: readonly string[] = [
  "CHARACTER",
  "SCENE",
  "REFERENCE",
  "PROMPT",
  "WORKFLOW",
  "OUTPUT",
  "COMFY_CAPABILITY",
];

export function ShotReadinessInspector({ detail, loading = false, error, onRetry }: ShotReadinessInspectorProps) {
  if (loading) {
    return <aside className="shot-readiness-inspector" aria-label="镜头就绪度检查"><div className="shot-readiness-inspector-empty"><strong>正在读取镜头详情</strong><span>按需解析上下文与七项 Gate…</span></div></aside>;
  }

  if (error) {
    return <aside className="shot-readiness-inspector" aria-label="镜头就绪度检查"><div className="shot-readiness-inspector-empty"><strong>详情读取失败</strong><span>{error}</span>{onRetry && <button type="button" className="quiet-button" onClick={onRetry}>重试</button>}</div></aside>;
  }

  if (!detail) {
    return <aside className="shot-readiness-inspector" aria-label="镜头就绪度检查"><div className="shot-readiness-inspector-empty"><strong>选择一个镜头</strong><span>右侧会显示完整上下文、七 Gate 与冻结前检查摘要。</span></div></aside>;
  }

  const readiness = detail.readiness;
  const gateMap = new Map((readiness?.gates ?? []).map((gate) => [gate.key, gate]));
  const profiles = profileEntries(detail);
  const references = referenceEntries(detail);
  const blockers = uniqueStrings([
    ...detail.blockers,
    ...gateMessages(readiness?.gates ?? [], ["BLOCKER"]),
  ]);
  const warnings = uniqueStrings([
    ...detail.warnings,
    ...gateMessages(readiness?.gates ?? [], ["WARNING"]),
  ]);
  const status = readiness?.status ?? "INCOMPLETE";
  const legacy = detail.resolvedContext.legacy;

  return (
    <aside className="shot-readiness-inspector" aria-label="镜头就绪度检查">
      <div className="shot-readiness-inspector-heading">
        <div>
          <span className="section-label">Readiness / Context</span>
          <h3>{detail.name || detail.shotId}</h3>
          <small>{detail.shotId} · {detail.stage === "image" ? "图片" : "视频"} · #{detail.ordinal + 1}</small>
        </div>
        <StatusBadge status={status} />
      </div>

      <div className="shot-readiness-score-row">
        <strong>{readiness?.score ?? "—"}</strong>
        <span>就绪度评分</span>
        {detail.alreadyPrepared && <em>已准备</em>}
        {detail.stalePreparedBatchIds.length > 0 && <em className="shot-readiness-stale">旧上下文</em>}
      </div>

      <section className="shot-readiness-inspector-section" aria-label="七项 Gate">
        <div className="shot-readiness-inspector-section-heading"><h4>七项 Gate</h4><span>{preparationStatusLabel(status)}</span></div>
        <div className="shot-readiness-gates">
          {GATE_KEYS.map((key) => <GateRow key={key} gate={gateMap.get(key)} gateKey={key} />)}
        </div>
      </section>

      <section className="shot-readiness-inspector-section" aria-label="阻塞与警告">
        <div className="shot-readiness-inspector-section-heading"><h4>检查摘要</h4><span>{blockers.length} 个阻塞 · {warnings.length} 个警告</span></div>
        {blockers.length > 0 && <MessageList title="阻塞" messages={blockers} tone="blocker" />}
        {warnings.length > 0 && <MessageList title="警告" messages={warnings} tone="warning" />}
        {!blockers.length && !warnings.length && <p className="shot-readiness-inspector-muted">没有额外阻塞或警告。</p>}
      </section>

      <section className="shot-readiness-inspector-section" aria-label="上下文摘要">
        <div className="shot-readiness-inspector-section-heading"><h4>上下文摘要</h4><span className="shot-readiness-hash" title={detail.contextHash}>Hash {shortHash(detail.contextHash)}</span></div>
        <dl className="shot-readiness-context-list">
          <div><dt>Profile 来源</dt><dd>{profiles.length ? profiles.length + " 个" : "无新 Profile（可能使用 Legacy）"}</dd></div>
          <div><dt>ReferenceSet</dt><dd>{references.sets.length ? references.sets.length + " 个 · " + references.assetCount + " 个素材" : "无 ReferenceSet"}</dd></div>
          <div><dt>工作流</dt><dd>{detail.resolvedContext.workflow?.workflowVersionId ?? "未配置"}{detail.resolvedContext.workflow?.recipeId ? " · " + detail.resolvedContext.workflow.recipeId : ""}</dd></div>
          <div><dt>上下文 Hash</dt><dd className="shot-readiness-breakable">{detail.contextHash || "—"}</dd></div>
        </dl>
        {legacy && <div className="shot-readiness-legacy-note"><strong>Legacy Shot</strong><span>沿用旧 Shot prompt / stage config / reference 关系，无需先创建 Profile。</span></div>}
      </section>

      <section className="shot-readiness-inspector-section" aria-label="Profile 与 ReferenceSet 来源">
        <div className="shot-readiness-inspector-section-heading"><h4>Profile / ReferenceSet</h4></div>
        {profiles.length > 0 && <div className="shot-readiness-profile-list">{profiles.map((profile) => <div className="shot-readiness-profile-row" key={profile.key}><span className="shot-readiness-profile-type">{profile.type}</span><strong>{profile.name}</strong><small>{profile.source}</small></div>)}</div>}
        {references.sets.length > 0 && <div className="shot-readiness-reference-list">{references.sets.map((referenceSet) => <div className="shot-readiness-reference-row" key={referenceSet.key}><span>{referenceSet.role}</span><strong>{referenceSet.name}</strong><small>{referenceSet.assetCount} 个素材 · {referenceSet.source}</small></div>)}</div>}
        {!profiles.length && !references.sets.length && <p className="shot-readiness-inspector-muted">当前镜头没有可展示的新一致性档案或参考集。</p>}
      </section>

      {detail.resolvedContext.stageInput?.selectedImageAssetId && <div className="shot-readiness-stage-input"><span>视频关键帧</span><strong>{detail.resolvedContext.stageInput.selectedImageAssetId}</strong><small>{detail.resolvedContext.stageInput.selectedImageSha256 ?? "未提供校验和"}</small></div>}
    </aside>
  );
}

function GateRow({ gate, gateKey }: { gate?: ReadinessGateView; gateKey: string }) {
  const state = gate?.state ?? "INCOMPLETE";
  const detail = gate?.checks?.find((check) => check.state !== "PASS")?.message;
  return <div className={"shot-readiness-gate shot-readiness-gate-" + state.toLowerCase()}><span className="shot-readiness-gate-dot" aria-hidden="true" /><div><strong>{preparationGateLabel(gateKey)}</strong>{detail && <small>{detail}</small>}</div><span className="shot-readiness-gate-state">{readinessStateLabel(state)}</span></div>;
}

function StatusBadge({ status }: { status: string }) {
  return <span className={"shot-preparation-status shot-preparation-status-" + status.toLowerCase()}>{preparationStatusLabel(status)}</span>;
}

function MessageList({ title, messages, tone }: { title: string; messages: string[]; tone: "blocker" | "warning" }) {
  return <div className={"shot-readiness-message-group shot-readiness-message-" + tone}><strong>{title}</strong><ul>{messages.slice(0, 6).map((message) => <li key={message}>{message}</li>)}</ul></div>;
}

interface ProfileEntry {
  key: string;
  type: string;
  name: string;
  source: string;
}

function profileEntries(detail: ShotProductionPlanDetail): ProfileEntry[] {
  const profiles = detail.resolvedContext.profiles;
  if (!profiles) return [];
  const entries: Array<{ profile?: ResolvedProfileView | null; type: string }> = [
    ...(profiles.characters ?? []).map((profile) => ({ profile, type: "角色" })),
    { profile: profiles.scene, type: "场景" },
    ...(profiles.props ?? []).map((profile) => ({ profile, type: "道具" })),
    { profile: profiles.style, type: "风格" },
  ];
  return entries.filter(({ profile }) => profile).map(({ profile, type }) => ({
    key: type + ":" + (profile!.profileId ?? profile!.id ?? profile!.name ?? "profile"),
    type,
    name: profile!.name ?? profile!.profileId ?? profile!.id ?? "未命名 Profile",
    source: sourceLabel(profile!.source),
  }));
}

interface ReferenceSummary {
  sets: Array<{ key: string; role: string; name: string; assetCount: number; source: string }>;
  assetCount: number;
}

function referenceEntries(detail: ShotProductionPlanDetail): ReferenceSummary {
  const context = detail.resolvedContext;
  const sets = context.referencePack?.referenceSets ?? [];
  const assets = context.referenceAssets ?? context.referencePack?.referenceAssets ?? [];
  return {
    sets: sets.map((referenceSet) => ({
      key: referenceSet.referenceSetId + ":" + (referenceSet.ordinal ?? 0),
      role: referenceSet.role ?? "参考",
      name: referenceSet.name ?? referenceSet.referenceSetId,
      assetCount: referenceSet.assets?.length ?? assets.filter((asset) => asset.sourceReferenceSetId === referenceSet.referenceSetId).length,
      source: sourceLabel(referenceSet.source),
    })),
    assetCount: assets.length || sets.reduce((total, referenceSet) => total + (referenceSet.assets?.length ?? 0), 0),
  };
}

function sourceLabel(source: unknown): string {
  if (!source) return "未知来源";
  if (typeof source === "string") return source;
  if (typeof source !== "object") return "未知来源";
  const trace = source as { scope?: string; scopeId?: string };
  const scope = trace.scope ?? "未知来源";
  const labels: Record<string, string> = { PROJECT: "Project", SERIES: "Series", EPISODE: "Episode", SCENE: "Scene", SHOT: "Shot", LEGACY: "Legacy" };
  return (labels[scope] ?? scope) + (trace.scopeId ? " · " + trace.scopeId : "");
}

function gateMessages(gates: ReadinessGateView[], states: string[]): string[] {
  return gates.flatMap((gate) => (gate.checks ?? [])
    .filter((check) => states.includes(check.state ?? ""))
    .map((check) => check.message || check.code || preparationGateLabel(gate.key) + "未通过"));
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))];
}

function shortHash(value: string): string {
  if (!value) return "—";
  return value.length > 16 ? value.slice(0, 8) + "…" + value.slice(-6) : value;
}

export function readinessGateEntries(gates: ReadinessGateView[]): string[] {
  return GATE_KEYS.map((key) => gates.find((gate) => gate.key === key)?.state ?? "INCOMPLETE");
}

export function readinessCheckMessages(checks: ReadinessCheckView[] = []): string[] {
  return uniqueStrings(checks.map((check) => check.message || check.code || "").filter(Boolean));
}
