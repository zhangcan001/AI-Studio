import { toUserMessage } from "../../i18n/errorMessages";
import type { WorkflowFieldType,WorkflowOnboardingDraftView } from "../../types/workflowOnboarding";
import { type MappingDraft } from "../workflows/workflowParameterExposureModel";
export function IssueList({ issues }: { issues: WorkflowOnboardingDraftView["capability"]["issues"] }) {
  return <ul className="workflow-issue-list">{issues.map((issue) => <li key={`${issue.code}:${issue.nodeId ?? ""}:${issue.inputName ?? ""}`}>{toUserMessage({ code: issue.code, message: issue.message })}</li>)}</ul>;
}

export function formatCapability(value: string): string {
  return {
    READY: "可用",
    MISSING_NODES: "缺少节点",
    INCOMPATIBLE_INPUT_VALUES: "输入值不兼容",
    COMFY_OFFLINE: "ComfyUI 离线",
    UNKNOWN_OUTPUT_ROOT: "无法确定输出节点",
    AMBIGUOUS_OUTPUT_ROOT: "输出节点有歧义",
    PARTIALLY_SUPPORTED: "部分输出根不可用",
    NOT_CHECKED: "尚未检查",
  }[value] ?? "未知状态";
}

export function fieldTypeLabel(value: WorkflowFieldType): string {
  return {
    textarea: "多行文本",
    integer: "整数",
    number: "小数",
    seed: "随机种子",
    image: "图片",
    images: "多张图片",
    video: "视频",
    videos: "多个视频",
    audio: "音频",
    audios: "多个音频",
  }[value];
}

/** W-22: fixed reuses the literal seed as default; random draws a new seed per run. */
export function SeedModeSelect({ value, defaultValue, onChange }: { value?: MappingDraft["seedMode"]; defaultValue: string; onChange: (value: MappingDraft["seedMode"]) => void }) {
  const effective = value || (/^\d+$/.test(defaultValue.trim()) ? "fixed" : "random");
  return (
    <label>种子模式<select aria-label="种子模式" value={effective} onChange={(event) => onChange(event.target.value as MappingDraft["seedMode"])}>
      <option value="fixed">固定（使用默认值）</option>
      <option value="random">随机（每次生成新种子）</option>
    </select></label>
  );
}
