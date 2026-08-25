import type { AssetView } from "../types/asset";

const TASK_STATUS_LABELS: Record<string, string> = {
  CREATED: "已创建",
  VALIDATING: "正在校验",
  PREPARING: "正在准备",
  QUEUED: "已排队",
  RUNNING: "生成中",
  CANCEL_REQUESTED: "正在取消",
  COLLECTING: "正在收集结果",
  SUCCEEDED: "已完成",
  FAILED: "失败",
  CANCELLED: "已取消",
};

const PRODUCTION_STATUS_LABELS: Record<string, string> = {
  READY: "待开始",
  RUNNING: "运行中",
  PAUSED: "已暂停",
  COMPLETED: "已完成",
};

const PRODUCTION_ITEM_STATUS_LABELS: Record<string, string> = {
  PENDING: "等待中",
  DISPATCHING: "正在提交",
  DISPATCHED: "执行中",
  SUCCEEDED: "已完成",
  FAILED: "失败",
  CANCELLED: "已取消",
  SKIPPED: "已跳过",
};

const ASSET_CATEGORY_LABELS: Record<string, string> = {
  ALL: "全部",
  source_image: "源图片",
  generated_image: "生成图片",
  source_video: "源视频",
  generated_video: "生成视频",
  source_audio: "源音频",
};

const WORKFLOW_MODE_LABELS: Record<string, string> = {
  text_to_image: "文生图",
  image_to_image: "图生图",
  text_to_video: "文生视频",
  image_to_video: "图生视频",
  reference_to_video: "参考素材生成视频",
};

const FIELD_LABELS: Record<string, string> = {
  prompt: "提示词",
  negative_prompt: "负面提示词",
  seed: "随机种子",
  width: "宽度",
  height: "高度",
  steps: "采样步数",
  duration: "时长（秒）",
  duration_seconds: "时长（秒）",
  length: "帧数",
  first_frame: "首帧图片",
  last_frame: "尾帧图片",
  reference_image: "参考图片",
  reference_images: "参考图片",
  reference_video: "参考视频",
  reference_videos: "参考视频",
  reference_audio: "参考音频",
  reference_audios: "参考音频",
};

const WORKFLOW_ALIASES: Record<string, string> = {
  wfl_kera2_t2i_local_v2: "Krea2 文生图",
  wfl_minimax_h3_reference_video: "H3 参考图生视频",
};

const WORKFLOW_NAME_ALIASES: Record<string, string> = {
  "krea2 t2i local": "Krea2 文生图",
  "kera2 t2i local": "Krea2 文生图",
  "minimax h3 reference video": "H3 参考图生视频",
};

const EVENT_LABELS: Record<string, string> = {
  TASK_CREATED: "任务已创建",
  TASK_VALIDATING: "正在校验任务",
  TASK_PREPARING: "正在准备任务",
  TASK_SUBMISSION_PREPARED: "提交内容已准备",
  TASK_QUEUED: "任务已排队",
  TASK_RUNNING: "任务正在生成",
  TASK_NODE_STARTED: "节点开始执行",
  TASK_PROGRESS_UPDATED: "生成进度已更新",
  TASK_COLLECTING: "正在收集结果",
  TASK_SUCCEEDED: "任务已完成",
  TASK_FAILED: "任务失败",
  TASK_CANCEL_REQUESTED: "已请求取消任务",
  TASK_CANCELLED: "任务已取消",
  TASK_RECOVERY_STARTED: "开始恢复任务",
  TASK_RECOVERY_SUCCEEDED: "任务恢复完成",
};

const STAGING_STATUS_LABELS: Record<string, string> = {
  STALE_STAGING: "过期暂存",
};

export function taskStatusLabel(status: string): string {
  return TASK_STATUS_LABELS[status] ?? "未知状态";
}

export function productionStatusLabel(status: string): string {
  return PRODUCTION_STATUS_LABELS[status] ?? "未知状态";
}

export function productionItemStatusLabel(status: string): string {
  return PRODUCTION_ITEM_STATUS_LABELS[status] ?? "未知状态";
}

export function assetCategoryLabel(category: string): string {
  return ASSET_CATEGORY_LABELS[category] ?? "其他资产";
}

export function assetTypeLabel(asset: Pick<AssetView, "assetType" | "category">): string {
  if (asset.category in ASSET_CATEGORY_LABELS) return ASSET_CATEGORY_LABELS[asset.category];
  if (asset.assetType === "image") return "图片";
  if (asset.assetType === "video") return "视频";
  if (asset.assetType === "audio") return "音频";
  return "媒体";
}

export function assetDisplayName(asset: Pick<AssetView, "category" | "name">, name = asset.name): string {
  if (asset.category === "generated_image" && /^generated image(?:\s+\d+)?$/i.test(name.trim())) {
    const suffix = name.trim().match(/\d+$/)?.[0];
    return suffix ? `生成图片 ${suffix}` : "生成图片";
  }
  if (asset.category === "generated_video" && /^generated video(?:\s+\d+)?$/i.test(name.trim())) {
    const suffix = name.trim().match(/\d+$/)?.[0];
    return suffix ? `生成视频 ${suffix}` : "生成视频";
  }
  return name;
}

export function workflowModeLabel(mode: string): string {
  return WORKFLOW_MODE_LABELS[mode] ?? "其他模式";
}

export function workflowDescription(mode: string): string {
  if (mode === "text_to_image") return "快速生成高质量图片";
  if (mode === "reference_to_video") return "使用参考图生成带声音的短视频";
  if (mode === "image_to_video") return "将图片转换为动态视频";
  if (mode === "text_to_video") return "根据文字描述生成视频";
  if (mode === "image_to_image") return "基于参考图生成新的图片";
  return "根据当前工作流生成内容";
}

export function workflowDisplayName(workflowId: string | undefined, name: string): string {
  return (workflowId && WORKFLOW_ALIASES[workflowId])
    ?? WORKFLOW_NAME_ALIASES[name.trim().toLowerCase()]
    ?? name;
}

export function projectDisplayName(projectId: string, name: string): string {
  return projectId === "prj_default" ? "默认项目" : name;
}

export function comfyStatusLabel(status: string | undefined): string {
  if (status === "CONNECTED") return "已连接";
  if (status === "INCOMPATIBLE") return "不兼容";
  return "离线";
}

export function fieldLabel(semanticKey: string, runtimeLabel?: string): string {
  return FIELD_LABELS[semanticKey] ?? FIELD_LABELS[semanticKey.toLowerCase()] ?? runtimeLabel ?? semanticKey;
}

export function eventLabel(code: string): string {
  return EVENT_LABELS[code] ?? "其他事件";
}

export function stagingStatusLabel(status: string): string {
  return STAGING_STATUS_LABELS[status] ?? "未知状态";
}

export function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "时间未知";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

export function formatDurationMs(durationMs: number | null | undefined): string {
  if (durationMs === undefined || durationMs === null) return "—";
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes ? `${minutes}分${seconds ? ` ${seconds}秒` : ""}` : `${seconds}秒`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}
