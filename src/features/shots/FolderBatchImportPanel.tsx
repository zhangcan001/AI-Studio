import { useEffect, useMemo, useRef, useState } from "react";
import { MAX_SHOT_DURATION_SECONDS } from "../../product/shotDuration";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { invalidateRuns } from "../../product/runInvalidation";
import { invalidateVideoInputs } from "../../product/videoInputInvalidation";
import { defaultGenerationValues } from "../../stores/studioStore";
import type { GeneratorOption } from "../../product/types";
import { toMediaByteArray, type LibraryItem } from "../../product/libraryTypes";
import type { ShotInputValues } from "../../types/shot";
import type { VideoInputAsset } from "../../types/shotVideoInput";
import { useFolderBatchImportController } from "./useFolderBatchImportController";

export interface FolderBatchImportProps {
  projectId: string;
  initialTab?: "board" | "folder";
  onImported: () => unknown;
  onCancel: () => void;
  onOpenBatch?: () => void;
  onOpenQueue?: (batchId: string) => void;
  onOpenShot?: (shotId: string) => unknown;
  onOpenLegacy?: () => void;
}
const targetLabels: Record<string, string> = { batch: "批次", shot: "镜头", entity: "角色/场景", asset: "素材", "source-files": "素材文件" };
const modes: Record<string, string> = { text: "文生视频", image: "单图视频", firstLast: "首尾帧", reference: "混合参考" };

interface BoardImage {
  assetId: string;
  name: string;
}

interface BoardRow {
  key: string;
  checked: boolean;
  name: string;
  prompt: string;
  durationSeconds?: number;
  firstFrame?: BoardImage;
  lastFrame?: BoardImage;
  references: BoardImage[];
}

function naturalCompare(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
}

function fileStem(name: string): string {
  const base = name.replace(/^.*[\\/]/, "");
  return base.replace(/\.[^.]+$/, "").trim() || base.trim();
}

let boardRowSeq = 1;
function nextBoardKey(): string {
  return `brow_${boardRowSeq++}`;
}

function BoardImageThumb(props: {
  projectId: string;
  assetId: string;
  name: string;
  onZoom?: (url: string, name: string) => void;
}) {
  const { projectId, assetId, name, onZoom } = props;
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let alive = true;
    let objectUrl: string | undefined;
    const load = async () => {
      try {
        const loader = typeof productClient.library?.thumbnailGet === "function"
          ? productClient.library.thumbnailGet
          : productClient.library?.imageGet;
        if (typeof loader !== "function") return;
        const bytes = await loader(projectId, { kind: "asset", id: assetId });
        if (!alive) return;
        objectUrl = URL.createObjectURL(new Blob([toMediaByteArray(bytes)], { type: "image/png" }));
        setUrl(objectUrl);
      } catch {
        try {
          if (typeof productClient.library?.imageGet !== "function") return;
          const bytes = await productClient.library.imageGet(projectId, { kind: "asset", id: assetId });
          if (!alive) return;
          objectUrl = URL.createObjectURL(new Blob([toMediaByteArray(bytes)], { type: "image/png" }));
          setUrl(objectUrl);
        } catch { /* fallback placeholder */ }
      }
    };
    void load();
    return () => { alive = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [projectId, assetId]);

  if (!url) {
    return <span className="board-import-thumb-placeholder" title={name}>{name.slice(0, 6)}</span>;
  }
  if (!onZoom) {
    return <span className="board-import-thumb" title={name}><img src={url} alt={`${name}缩略图`} /></span>;
  }
  return (
    <button type="button" className="board-import-thumb" title={`点击放大查看 ${name}`} aria-label={`查看 ${name}`} onClick={() => onZoom(url, name)}>
      <img src={url} alt={`${name}缩略图`} />
    </button>
  );
}

interface ParsedPasteShot {
  name: string;
  prompt: string;
}

function parsePastedPrompts(raw: string, lineMode: boolean, prefix: string): ParsedPasteShot[] {
  const unfenced = raw.trim().replace(/^```(?:json|text|markdown)?\s*\r?\n/i, "").replace(/\r?\n```\s*$/, "").trim();
  if (!unfenced) return [];
  const basePrefix = prefix.trim() || "导入镜头";
  if (unfenced.startsWith("[") || unfenced.startsWith("{")) {
    try {
      const parsed = JSON.parse(unfenced) as unknown;
      const list = Array.isArray(parsed)
        ? parsed
        : parsed && typeof parsed === "object"
          ? ((parsed as Record<string, unknown>).shots ?? (parsed as Record<string, unknown>).items ?? (parsed as Record<string, unknown>).prompts)
          : undefined;
      if (Array.isArray(list)) {
        const out: ParsedPasteShot[] = [];
        list.forEach((item, idx) => {
          if (typeof item === "string" && item.trim()) {
            out.push({ name: `${basePrefix} ${String(idx + 1).padStart(2, "0")}`, prompt: item.trim() });
          } else if (item && typeof item === "object") {
            const rec = item as Record<string, unknown>;
            const text = String(rec.prompt ?? rec.description ?? rec.text ?? "").trim();
            if (!text) return;
            const name = String(rec.name ?? rec.title ?? "").trim() || `${basePrefix} ${String(idx + 1).padStart(2, "0")}`;
            out.push({ name, prompt: text });
          }
        });
        if (out.length) return out;
      }
    } catch { /* fall through to text parsing */ }
  }
  const chunks = lineMode
    ? unfenced.split(/\r?\n+/).map(s => s.trim()).filter(Boolean)
    : unfenced.split(/\r?\n\s*\r?\n+/).map(s => s.trim()).filter(Boolean);
  return chunks.map((block, idx) => {
    const clean = block.replace(/^(?:#{1,4}\s*)?(?:\*\*)?(?:(?:镜头|分镜|Shot)\s*#?\d+\s*[:：\-—.]?\s*|\d+\s*[.)、:：]\s*)(?:\*\*)?/i, "").trim();
    const headerMatch = block.match(/^(?:#{1,4}\s*)?(?:\*\*)?(?:镜头|分镜|Shot)\s*#?(\d+)(?:\s*[:：\-—]\s*([^\n*]{1,24}))?/i);
    const autoName = headerMatch?.[2]?.trim() && block.includes("\n")
      ? headerMatch[2].trim()
      : `${basePrefix} ${String(idx + 1).padStart(2, "0")}`;
    return { name: autoName, prompt: clean || block };
  }).filter(item => item.prompt.length > 0);
}

function decodeSelectionRef(ref: string): { workflowVersionId: string; recipeId: string } | undefined {
  if (!ref.startsWith("generator:v1:")) return undefined;
  const hex = ref.slice("generator:v1:".length);
  if (!hex || hex.length % 2 !== 0 || !/^[0-9a-f]+$/i.test(hex)) return undefined;
  try {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    const pair = JSON.parse(new TextDecoder().decode(bytes)) as unknown;
    if (Array.isArray(pair) && pair.length === 2 && typeof pair[0] === "string" && typeof pair[1] === "string") {
      return { workflowVersionId: pair[0], recipeId: pair[1] };
    }
  } catch { /* ignore */ }
  return undefined;
}

function resolveBatchModeKey(g: { mode: string; fields?: { key: string; type: string }[] }): "text" | "image" | "firstLast" | "reference" | undefined {
  const m = (g.mode ?? "").toUpperCase();
  if (m === "T2V") return "text";
  if (m === "I2V") return "image";
  if (m === "FIRST_LAST" || m === "FL2V") return "firstLast";
  if (m === "REF2VA") return "reference";
  const fields = g.fields ?? [];
  if (fields.some(f => ["reference_images", "reference_videos", "reference_audios"].includes(f.key))) return "reference";
  if (fields.some(f => f.key === "first_frame") && fields.some(f => f.key === "last_frame")) return "firstLast";
  if (fields.some(f => f.key === "first_frame")) return "image";
  return "text";
}

function buildAiManifestGuide(
  generators: { name: string; mode: string; version: string; selectionRef: string; availability: boolean; recommended?: boolean; fields?: { key: string; type: string }[] }[],
  entities: { name: string; kind: string }[] = [],
): string {
  const available = generators.filter(g => g.availability);
  const sorted = [...available].sort((a, b) => Number(Boolean(b.recommended)) - Number(Boolean(a.recommended)));
  const modeMap: Record<string, { workflowVersionId: string; recipeId: string }> = {};
  const variantLines: string[] = [];
  for (const g of sorted) {
    const key = resolveBatchModeKey(g);
    const pair = decodeSelectionRef(g.selectionRef);
    if (!key || !pair) continue;
    if (!modeMap[key]) modeMap[key] = pair;
    variantLines.push(`  - [${key}] ${g.name} (v${g.version}${g.recommended ? "，推荐" : ""}): workflowVersionId="${pair.workflowVersionId}", recipeId="${pair.recipeId}"`);
  }
  const availableModes = Object.keys(modeMap);
  const defaultMode = availableModes.includes("reference")
    ? "reference"
    : availableModes[0] ?? "text";

  const firstExistingEntity = entities[0];
  const sampleShots: Record<string, unknown>[] = [];
  if (defaultMode === "reference") {
    const refEntityName = firstExistingEntity?.name ?? "主角A";
    sampleShots.push({
      name: "镜01 开场",
      mode: "reference",
      prompt: `@${refEntityName} walking through neon-lit rain street, cinematic slow push in...`,
      durationSeconds: 5,
    });
  } else if (defaultMode === "firstLast") {
    sampleShots.push({
      name: "镜01 开场",
      mode: "firstLast",
      prompt: "Smooth camera transition from opening frame to final frame...",
      firstFrame: "frames/01_first.png",
      lastFrame: "frames/01_last.png",
      durationSeconds: 5,
    });
  } else if (defaultMode === "image") {
    sampleShots.push({
      name: "镜01 开场",
      mode: "image",
      prompt: "Subtle natural motion, cinematic camera push in...",
      firstFrame: "frames/01.png",
      durationSeconds: 5,
    });
  } else {
    sampleShots.push({
      name: "镜01 开场",
      mode: "text",
      prompt: "Cinematic wide shot, golden hour light across misty mountains...",
      durationSeconds: 5,
    });
  }

  const template = {
    format: "ai-studio-shot-batch",
    version: 1,
    name: "我的分镜批次",
    generators: Object.keys(modeMap).length ? modeMap : {
      text: { workflowVersionId: "<请在系统设置确认可用生成器>", recipeId: "<RECIPE_ID>" },
    },
    defaults: { mode: defaultMode, durationSeconds: 5, aspectRatio: "16:9", resolutionPreset: "0.98" },
    characters: defaultMode === "reference" && !firstExistingEntity ? [
      {
        name: "主角A",
        kind: "character",
        description: "young woman in red coat, cinematic realism",
        images: ["characters/hero.png"],
        cover: "characters/hero.png",
      },
    ] : [],
    shots: sampleShots,
    queue: { create: true, autoStart: false },
  };
  const entityLines = entities.length
    ? [
        "- 当前项目已存在的角色/场景/道具/构图（可直接用 @实体名 引用，无需重复在 characters 中声明）：",
        ...entities.slice(0, 40).map(e => `  - @${e.name} (${e.kind})`),
      ]
    : [];
  return [
    "请严格按照以下 JSON Schema 生成 manifest.json（开启了 deny_unknown_fields，严禁添加未列出的额外字段）：",
    "- 支持的 mode 及必填规则：",
    "  * text（纯文生视频）：禁止填 firstFrame / lastFrame / references / characters，提示词不能含 @",
    "  * image（单图视频）：必填 firstFrame，禁止填 lastFrame / references / characters，提示词不能含 @",
    "  * firstLast（首尾帧）：必填 firstFrame + lastFrame，禁止填 references / characters，提示词不能含 @",
    "  * reference（混合参考）：禁止填 firstFrame / lastFrame；必须填 references（图片相对路径数组）或在 prompt 中用 @实体名 引用已声明/已有实体",
    "- 可选的 shot 字段：id, name, mode, prompt, style, summary, soundscape, music, firstFrame, lastFrame, references, characters, openingComposition, endingComposition, durationSeconds (1-11), aspectRatio (16:9/9:16/1:1), resolutionPreset (0.5/0.98), seed",
    "- characters 中每个实体支持字段：name, kind (character/scene/prop/composition), description（建议英文）, images（相对路径数组，至少1张）, cover（需在 images 内）",
    "- 所有图片路径必须为当前文件夹内的相对路径（仅支持 png/jpg/jpeg/webp，使用正斜杠 /）",
    ...entityLines,
    ...(variantLines.length ? ["- 当前项目已检测到的全部可用生成器配对（可直接替换到 generators 中）：", ...variantLines] : []),
    "",
    JSON.stringify(template, null, 2),
  ].join("\n");
}

type PickerTarget =
  | { kind: "bulk" }
  | { kind: "slot"; rowKey: string; slot: "firstFrame" | "lastFrame" | "reference" };

function VisualBatchBoard(props: {
  projectId: string;
  onImported: () => unknown;
  onCancel: () => void;
  onOpenBatch?: () => void;
  onBusyChange: (busy: boolean) => void;
}) {
  const { projectId, onImported, onCancel, onOpenBatch, onBusyChange } = props;
  const [generators, setGenerators] = useState<GeneratorOption[]>([]);
  const [selectionRef, setSelectionRef] = useState("");
  const [defaultDuration, setDefaultDuration] = useState(5);
  const [presetId, setPresetId] = useState("");
  const [rows, setRows] = useState<BoardRow[]>([]);
  const [importingImages, setImportingImages] = useState(false);
  const [pasteOpen, setPasteOpen] = useState(false);
  const [pasteText, setPasteText] = useState("");
  const [pasteLineMode, setPasteLineMode] = useState(false);
  const [pastePrefix, setPastePrefix] = useState("镜头");
  const [pickerTarget, setPickerTarget] = useState<PickerTarget | null>(null);
  const [libraryItems, setLibraryItems] = useState<LibraryItem[]>([]);
  const [libraryLoading, setLibraryLoading] = useState(false);
  const [libraryQuery, setLibraryQuery] = useState("");
  const [librarySelected, setLibrarySelected] = useState<BoardImage[]>([]);
  const [committing, setCommitting] = useState(false);
  const [progress, setProgress] = useState<string>();
  const [error, setError] = useState<string>();
  const [createdCount, setCreatedCount] = useState<number>();
  const [zoomed, setZoomed] = useState<{ url: string; name: string } | null>(null);

  useEffect(() => {
    onBusyChange(committing || importingImages);
  }, [committing, importingImages, onBusyChange]);

  useEffect(() => {
    setRows([]); setError(undefined); setCreatedCount(undefined); setProgress(undefined);
    setPickerTarget(null); setLibrarySelected([]); setPasteText(""); setPasteOpen(false);
    let alive = true;
    if (typeof productClient.creation?.generatorsList !== "function") return;
    void productClient.creation.generatorsList(projectId, "video").then(list => {
      if (!alive) return;
      const videoList = list.filter(g => (g.mediaKind ?? "video") === "video");
      setGenerators(videoList);
      const available = videoList.filter(g => g.availability);
      const preferred =
        available.find(g => g.recommended && ["I2V", "REF2VA", "FIRST_LAST"].includes((g.mode ?? "").toUpperCase()))
        ?? available.find(g => ["I2V", "REF2VA", "FIRST_LAST"].includes((g.mode ?? "").toUpperCase()))
        ?? available[0];
      if (preferred) {
        setSelectionRef(preferred.selectionRef);
        setPresetId(preferred.resolutionPresets?.[0]?.id ?? "");
        const durField = preferred.fields.find(f => f.key === "duration_seconds" && (f.type === "integer" || f.type === "number"));
        const def = durField && "default" in durField && typeof durField.default === "number" ? Math.round(durField.default) : 5;
        setDefaultDuration(Math.min(MAX_SHOT_DURATION_SECONDS, Math.max(1, def)));
      }
    }).catch(cause => {
      if (alive) setError(normalizeProductError(cause).message);
    });
    return () => { alive = false; };
  }, [projectId]);

  const selectedGenerator = generators.find(g => g.selectionRef === selectionRef && g.availability);
  const modeKey = selectedGenerator ? (resolveBatchModeKey(selectedGenerator) ?? "image") : "image";
  const parsedPaste = useMemo(() => parsePastedPrompts(pasteText, pasteLineMode, pastePrefix), [pasteText, pasteLineMode, pastePrefix]);

  const attachImagesInOrder = (incoming: BoardImage[]) => {
    if (!incoming.length) return;
    setCreatedCount(undefined);
    setError(undefined);
    setRows(prev => {
      const next = prev.map(r => ({ ...r, references: [...r.references] }));
      let cursor = 0;
      for (let i = 0; i < next.length && cursor < incoming.length; i++) {
        const row = next[i];
        if (modeKey === "reference") {
          if (row.references.length === 0) {
            row.references = [incoming[cursor]];
            if (!row.name.trim() || /^镜头\s*\d+$/.test(row.name.trim())) row.name = fileStem(incoming[cursor].name) || row.name;
            cursor++;
          }
        } else if (modeKey === "firstLast") {
          if (!row.firstFrame) {
            row.firstFrame = incoming[cursor];
            if (!row.name.trim() || /^镜头\s*\d+$/.test(row.name.trim())) row.name = fileStem(incoming[cursor].name) || row.name;
            cursor++;
          } else if (!row.lastFrame) {
            row.lastFrame = incoming[cursor];
            cursor++;
          }
        } else if (!row.firstFrame) {
          row.firstFrame = incoming[cursor];
          if (!row.name.trim() || /^镜头\s*\d+$/.test(row.name.trim())) row.name = fileStem(incoming[cursor].name) || row.name;
          cursor++;
        }
      }
      while (cursor < incoming.length) {
        const img = incoming[cursor++];
        const stem = fileStem(img.name) || `镜头 ${String(next.length + 1).padStart(2, "0")}`;
        next.push({
          key: nextBoardKey(),
          checked: true,
          name: stem,
          prompt: "",
          firstFrame: modeKey === "reference" ? undefined : img,
          references: modeKey === "reference" ? [img] : [],
        });
      }
      return next;
    });
  };

  const importLocalImages = async (folder: boolean) => {
    if (importingImages || committing) return;
    setImportingImages(true);
    setError(undefined);
    try {
      const res = await productClient.creation.assetsImport(projectId, folder);
      if (res.cancelled) return;
      const images: BoardImage[] = res.imported
        .filter(a => a.mediaKind === "image")
        .map(a => ({ assetId: a.id, name: a.name }));
      // Recover already-existing project images skipped by SHA-256 deduplication so re-picking works seamlessly.
      const dupNames = res.failed
        .filter(f => f.error.includes("重复导入") || f.error.includes("已存在"))
        .map(f => f.displayName);
      if (dupNames.length > 0 && typeof productClient.library?.list === "function") {
        const lib = await productClient.library.list(projectId, {
          category: "images", keyword: null, favoriteOnly: null, tagId: null, sourceKind: "source", cursor: null, limit: 200,
        }).catch(() => ({ items: [] as LibraryItem[] }));
        const seen = new Set(images.map(i => i.assetId));
        for (const name of dupNames) {
          const hit = lib.items.find(item => item.resourceRef.kind === "asset" && (item.title === name || fileStem(item.title) === fileStem(name)));
          if (hit && hit.resourceRef.kind === "asset" && !seen.has(hit.resourceRef.id)) {
            seen.add(hit.resourceRef.id);
            images.push({ assetId: hit.resourceRef.id, name: hit.title });
          }
        }
      }
      images.sort((a, b) => naturalCompare(a.name, b.name));
      if (!images.length) {
        setError(res.failed.length ? `未导入可用图片：${res.failed[0].displayName}（${res.failed[0].error}）` : "所选位置未发现可用的图片文件（png/jpg/jpeg/webp）。");
        return;
      }
      attachImagesInOrder(images);
      invalidateRuns(projectId, { library: true });
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: `已载入 ${images.length} 张分镜图（按自然顺序排列）`, tone: "success" } }));
    } catch (cause) {
      setError(`导入图片失败：${normalizeProductError(cause).message}`);
    } finally {
      setImportingImages(false);
    }
  };

  const openLibraryPicker = async (target: PickerTarget) => {
    setPickerTarget(target);
    setLibrarySelected([]);
    setLibraryQuery("");
    if (typeof productClient.library?.list !== "function") return;
    setLibraryLoading(true);
    try {
      const res = await productClient.library.list(projectId, {
        category: "images", keyword: null, favoriteOnly: null, tagId: null, sourceKind: null, cursor: null, limit: 200,
      });
      setLibraryItems(res.items.filter(item => item.resourceRef.kind === "asset"));
    } catch (cause) {
      setError(`读取素材库失败：${normalizeProductError(cause).message}`);
    } finally {
      setLibraryLoading(false);
    }
  };

  const pickFromLibraryItem = (item: LibraryItem) => {
    if (item.resourceRef.kind !== "asset" || !pickerTarget) return;
    const img: BoardImage = { assetId: item.resourceRef.id, name: item.title };
    if (pickerTarget.kind === "slot") {
      setRows(prev => prev.map(r => {
        if (r.key !== pickerTarget.rowKey) return r;
        if (pickerTarget.slot === "firstFrame") return { ...r, firstFrame: img };
        if (pickerTarget.slot === "lastFrame") return { ...r, lastFrame: img };
        if (r.references.some(x => x.assetId === img.assetId)) return r;
        return { ...r, references: [...r.references, img] };
      }));
      setPickerTarget(null);
      return;
    }
    setLibrarySelected(prev => prev.some(x => x.assetId === img.assetId) ? prev.filter(x => x.assetId !== img.assetId) : [...prev, img]);
  };

  const applyLibraryBulk = () => {
    if (!librarySelected.length) return;
    attachImagesInOrder(librarySelected);
    setPickerTarget(null);
    setLibrarySelected([]);
  };

  const applyPastedPrompts = (appendOnly: boolean) => {
    if (!parsedPaste.length) return;
    setCreatedCount(undefined);
    setError(undefined);
    setRows(prev => {
      const next = prev.map(r => ({ ...r, references: [...r.references] }));
      let cursor = 0;
      if (!appendOnly) {
        for (let i = 0; i < next.length && cursor < parsedPaste.length; i++) {
          const item = parsedPaste[cursor++];
          next[i].prompt = item.prompt;
          if (!next[i].name.trim()) next[i].name = item.name;
        }
      }
      while (cursor < parsedPaste.length) {
        const item = parsedPaste[cursor++];
        next.push({ key: nextBoardKey(), checked: true, name: item.name, prompt: item.prompt, references: [] });
      }
      return next;
    });
    setPasteText("");
    setPasteOpen(false);
    window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: `已按顺序填入 ${parsedPaste.length} 条提示词`, tone: "success" } }));
  };

  const moveRow = (index: number, delta: -1 | 1) => {
    const target = index + delta;
    setRows(prev => {
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  };

  const shiftImagesUpFrom = (index: number) => {
    setRows(prev => {
      const next = prev.map(r => ({ ...r, references: [...r.references] }));
      for (let i = index; i < next.length - 1; i++) {
        if (modeKey === "reference") {
          next[i].references = next[i + 1].references;
        } else {
          next[i].firstFrame = next[i + 1].firstFrame;
        }
      }
      if (next.length > 0) {
        if (modeKey === "reference") next[next.length - 1].references = [];
        else next[next.length - 1].firstFrame = undefined;
      }
      return next;
    });
  };

  const rowIssue = (row: BoardRow): string | undefined => {
    if (!row.checked) return undefined;
    if (!row.prompt.trim()) return "缺少提示词";
    if (modeKey === "image" && !row.firstFrame) return "单图视频需配首帧图";
    if (modeKey === "firstLast" && (!row.firstFrame || !row.lastFrame)) return "首尾帧模式需配齐首帧与尾帧";
    if (modeKey === "reference" && row.references.length === 0 && !row.prompt.includes("@")) return "混合参考需配参考图或 @实体";
    return undefined;
  };

  const checkedRows = rows.filter(r => r.checked);
  const invalidCheckedCount = checkedRows.filter(r => Boolean(rowIssue(r))).length;
  const canCommitBoard = Boolean(!committing && !importingImages && selectedGenerator && checkedRows.length > 0 && invalidCheckedCount === 0);

  const commitBoard = async () => {
    if (!canCommitBoard || !selectedGenerator) return;
    setCommitting(true);
    setError(undefined);
    setCreatedCount(undefined);
    const createdShotIds: string[] = [];
    const durationGroups = new Map<number, string[]>();
    try {
      for (let i = 0; i < checkedRows.length; i++) {
        const row = checkedRows[i];
        setProgress(`正在创建并绑定第 ${i + 1} / ${checkedRows.length} 个镜头…`);
        const shot = await productClient.creation.createShot(projectId);
        const shotName = row.name.trim() || `镜头 ${String(i + 1).padStart(2, "0")}`;
        if (typeof productClient.creation.updateShot === "function") {
          await productClient.creation.updateShot(projectId, {
            shotId: shot.id,
            name: shotName,
            stage: "video",
            prompt: row.prompt.trim(),
          });
        } else {
          await productClient.creation.shotsPromptAssign(projectId, [shot.id], row.prompt.trim());
        }
        createdShotIds.push(shot.id);
        const dur = Math.min(MAX_SHOT_DURATION_SECONDS, Math.max(1, Math.round(row.durationSeconds ?? defaultDuration)));
        const bucket = durationGroups.get(dur) ?? [];
        bucket.push(shot.id);
        durationGroups.set(dur, bucket);

        const inputs: VideoInputAsset[] = [];
        if (modeKey === "image" && row.firstFrame) {
          inputs.push({ inputKey: "first_frame", ordinal: 0, assetId: row.firstFrame.assetId });
        } else if (modeKey === "firstLast") {
          if (row.firstFrame) inputs.push({ inputKey: "first_frame", ordinal: 0, assetId: row.firstFrame.assetId });
          if (row.lastFrame) inputs.push({ inputKey: "last_frame", ordinal: 0, assetId: row.lastFrame.assetId });
        } else if (modeKey === "reference" && row.references.length > 0) {
          row.references.forEach((img, ordinal) => inputs.push({ inputKey: "reference_images", ordinal, assetId: img.assetId }));
        }
        if (inputs.length > 0 && typeof productClient.creation.inputsSave === "function") {
          await productClient.creation.inputsSave({
            selection: { projectId, shotId: shot.id, selectionRef: selectedGenerator.selectionRef },
            expected: null,
            inputs,
          });
        }
      }
      if (typeof productClient.creation.shotsConfigure === "function") {
        setProgress("正在统一写入视频生成器与参数…");
        const preset = selectedGenerator.resolutionPresets?.find(pr => pr.id === presetId) ?? selectedGenerator.resolutionPresets?.[0];
        const durationField = selectedGenerator.fields.find(f => f.key === "duration_seconds" && (f.type === "integer" || f.type === "number"));
        for (const [dur, ids] of durationGroups.entries()) {
          const baseValues = Object.fromEntries(
            Object.entries(defaultGenerationValues({ fields: selectedGenerator.fields }))
              .filter(([, v]) => ["integer", "number", "seed_fixed", "seed_random"].includes(v.type)),
          ) as ShotInputValues;
          if (durationField) {
            baseValues.duration_seconds = { type: durationField.type === "number" ? "number" : "integer", value: dur };
          }
          if (preset) {
            baseValues.width = { type: "integer", value: preset.width };
            baseValues.height = { type: "integer", value: preset.height };
          }
          await productClient.creation.shotsConfigure(projectId, {
            shotIds: ids,
            selectionRef: selectedGenerator.selectionRef,
            values: baseValues,
          });
        }
      }
      invalidateVideoInputs(projectId);
      invalidateRuns(projectId, { library: false });
      setCreatedCount(createdShotIds.length);
      setRows([]);
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: `已创建 ${createdShotIds.length} 个分镜并完成绑图与配参`, tone: "success" } }));
      await onImported();
    } catch (cause) {
      setError(`已创建 ${createdShotIds.length} 个镜头，后续失败：${normalizeProductError(cause).message}`);
    } finally {
      setCommitting(false);
      setProgress(undefined);
    }
  };

  const filteredLibrary = useMemo(() => {
    const q = libraryQuery.trim().toLowerCase();
    return q ? libraryItems.filter(i => i.title.toLowerCase().includes(q)) : libraryItems;
  }, [libraryItems, libraryQuery]);

  return (
    <div className="board-import" aria-label="可视化批量配对表">
      <div className="board-import-config">
        <label>
          视频生成器
          <select
            aria-label="配对表视频生成器"
            disabled={committing}
            value={selectionRef}
            onChange={e => {
              const g = generators.find(x => x.selectionRef === e.target.value);
              setSelectionRef(e.target.value);
              setPresetId(g?.resolutionPresets?.[0]?.id ?? "");
              if (g) {
                const nextMode = resolveBatchModeKey(g) ?? "image";
                setRows(prev => prev.map(r => {
                  if (nextMode === "reference" && r.references.length === 0 && r.firstFrame) {
                    return { ...r, references: [r.firstFrame] };
                  }
                  if (nextMode !== "reference" && !r.firstFrame && r.references[0]) {
                    return { ...r, firstFrame: r.references[0] };
                  }
                  return r;
                }));
              }
            }}
          >
            <option value="">请选择视频生成器</option>
            {generators.map(g => (
              <option key={g.selectionRef} value={g.selectionRef} disabled={!g.availability}>
                {g.name} · {modes[resolveBatchModeKey(g) ?? "image"]} · v{g.version}{g.availability ? "" : "（不可用）"}
              </option>
            ))}
          </select>
        </label>
        <label>
          默认时长（秒）
          <input
            aria-label="配对表默认时长"
            type="number"
            min={1}
            max={MAX_SHOT_DURATION_SECONDS}
            disabled={committing}
            value={defaultDuration}
            onChange={e => setDefaultDuration(Math.min(MAX_SHOT_DURATION_SECONDS, Math.max(1, Number(e.target.value) || 5)))}
          />
        </label>
        {selectedGenerator?.resolutionPresets?.length ? (
          <label>
            画幅与分辨率
            <select aria-label="配对表分辨率" disabled={committing} value={presetId} onChange={e => setPresetId(e.target.value)}>
              {selectedGenerator.resolutionPresets.map(pr => (
                <option key={pr.id} value={pr.id}>{pr.label} · {pr.width} × {pr.height}</option>
              ))}
            </select>
          </label>
        ) : null}
      </div>

      <div className="board-import-toolbar">
        <div className="board-import-toolbar-group">
          {modeKey !== "text" && <>
            <button type="button" className={rows.length ? undefined : "primary"} disabled={importingImages || committing} onClick={() => void importLocalImages(true)}>
              {importingImages ? "正在导入图片…" : "+ 导入图片文件夹"}
            </button>
            <button type="button" disabled={importingImages || committing} onClick={() => void importLocalImages(false)}>+ 选多张图片</button>
            <button type="button" disabled={importingImages || committing} onClick={() => void openLibraryPicker({ kind: "bulk" })}>从素材库挑选</button>
          </>}
          <button type="button" className={modeKey === "text" || rows.some(r => !r.prompt.trim()) ? "primary" : undefined} disabled={committing} onClick={() => setPasteOpen(v => !v)}>
            {pasteOpen ? "收起粘贴提示词" : "粘贴批量提示词"}
          </button>
        </div>
        <div className="board-import-toolbar-group">
          <button type="button" disabled={committing} onClick={() => setRows(prev => [...prev, { key: nextBoardKey(), checked: true, name: `镜头 ${String(prev.length + 1).padStart(2, "0")}`, prompt: "", references: [] }])}>+ 加一行</button>
          {rows.length > 0 && <button type="button" disabled={committing} onClick={() => { setRows([]); setCreatedCount(undefined); }}>清空表格</button>}
        </div>
      </div>

      {pasteOpen && (
        <div className="board-import-drawer" aria-label="批量粘贴提示词区">
          <div className="board-import-drawer-head">
            <label>默认镜名前缀<input value={pastePrefix} maxLength={40} onChange={e => setPastePrefix(e.target.value)} /></label>
            <label><input type="checkbox" checked={pasteLineMode} onChange={e => setPasteLineMode(e.target.checked)} /> 按每行一条拆分（默认空行分段 / JSON / Markdown）</label>
          </div>
          <textarea
            aria-label="粘贴批量提示词"
            className="folder-import-paste-input"
            rows={6}
            value={pasteText}
            onChange={e => setPasteText(e.target.value)}
            placeholder={'直接粘贴 AI 生成的多条提示词，会按顺序自动填到上方分镜图右侧：\n• 支持空行分段、每行一条、编号列表（1. / 镜头1：）\n• 支持 JSON 数组、{"shots":[...]} 或 Markdown 代码块'}
          />
          <div className="board-import-toolbar">
            <span>已识别 {parsedPaste.length} 条提示词</span>
            <div className="board-import-toolbar-group">
              {rows.length > 0 && <button type="button" disabled={!parsedPaste.length} onClick={() => applyPastedPrompts(true)}>追加为新行</button>}
              <button type="button" className="primary" disabled={!parsedPaste.length} onClick={() => applyPastedPrompts(false)}>
                按顺序填入配对表（{parsedPaste.length} 条）
              </button>
            </div>
          </div>
        </div>
      )}

      {pickerTarget && (
        <div className="board-import-drawer" aria-label="素材库图片选择">
          <div className="board-import-drawer-head">
            <strong>{pickerTarget.kind === "bulk" ? "从素材库按点击顺序多选分镜图" : "点击一张素材图填入该槽位"}</strong>
            <div className="board-import-toolbar-group">
              <input type="search" value={libraryQuery} onChange={e => setLibraryQuery(e.target.value)} placeholder="搜索素材文件名…" />
              {pickerTarget.kind === "bulk" && (
                <button type="button" className="primary" disabled={!librarySelected.length} onClick={applyLibraryBulk}>
                  填入配对表（{librarySelected.length} 张）
                </button>
              )}
              <button type="button" onClick={() => setPickerTarget(null)}>关闭</button>
            </div>
          </div>
          {libraryLoading ? <p role="status">正在读取项目素材库…</p> : filteredLibrary.length === 0 ? (
            <p>素材库中暂无匹配图片，可先点「+ 导入图片文件夹」。</p>
          ) : (
            <div className="board-import-lib-grid">
              {filteredLibrary.map(item => {
                if (item.resourceRef.kind !== "asset") return null;
                const selIdx = librarySelected.findIndex(x => x.assetId === item.resourceRef.id);
                return (
                  <button
                    key={item.resourceRef.id}
                    type="button"
                    className="board-import-lib-item"
                    aria-pressed={selIdx >= 0}
                    onClick={() => pickFromLibraryItem(item)}
                  >
                    {selIdx >= 0 && <em className="board-import-lib-badge">{selIdx + 1}</em>}
                    <BoardImageThumb projectId={projectId} assetId={item.resourceRef.id} name={item.title} />
                    <span title={item.title}>{item.title}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}

      {error && <p className="folder-import-error" role="alert">{error}</p>}
      {progress && <p role="status">{progress}</p>}
      {createdCount !== undefined && (
        <div className="board-import-receipt" role="status">
          <strong>已完成：创建 {createdCount} 个分镜镜头，并绑定分镜图、视频生成器与参数（未自动启动 GPU）。</strong>
          <div className="board-import-toolbar-group">
            {onOpenBatch && <button type="button" className="primary" onClick={onOpenBatch}>去批量准备视频 / 加入生产队列</button>}
            <button type="button" onClick={onCancel}>返回分镜工作台</button>
          </div>
        </div>
      )}

      {rows.length === 0 && createdCount === undefined ? (
        <div className="board-import-empty">
          <p><strong>无需改文件名，也无需写 JSON —— 所见即所得配对分镜</strong></p>
          <p>① 点上方「+ 导入图片文件夹」载入分镜图（直接看缩略图） → ② 点「粘贴批量提示词」一键按顺序灌入右侧 → ③ 发现错位直接行内改词或「删图前移」对齐 → ④ 一键创建。</p>
        </div>
      ) : rows.length > 0 && (
        <div className="board-import-table">
          <table>
            <thead>
              <tr>
                <th>选</th>
                {modeKey !== "text" && <th>分镜图（点击放大）</th>}
                <th>镜头名称与提示词（可直接修改）</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, idx) => {
                const issue = rowIssue(row);
                return (
                  <tr key={row.key} className={issue ? "board-import-row-invalid" : undefined}>
                    <td>
                      <label>
                        <input
                          type="checkbox"
                          aria-label={`勾选第 ${idx + 1} 行`}
                          checked={row.checked}
                          disabled={committing}
                          onChange={e => setRows(prev => prev.map(r => r.key === row.key ? { ...r, checked: e.target.checked } : r))}
                        />
                        {idx + 1}
                      </label>
                    </td>
                    {modeKey !== "text" && (
                      <td>
                        <div className="board-import-media-cell">
                          {modeKey === "reference" ? (
                            <>
                              {row.references.map(img => (
                                <div key={img.assetId} className="board-import-slot">
                                  <BoardImageThumb projectId={projectId} assetId={img.assetId} name={img.name} onZoom={(u, n) => setZoomed({ url: u, name: n })} />
                                  <small title={img.name}>{img.name}</small>
                                  <div className="board-import-slot-actions">
                                    <button type="button" disabled={committing} onClick={() => setRows(prev => prev.map(r => r.key === row.key ? { ...r, references: r.references.filter(x => x.assetId !== img.assetId) } : r))}>移除</button>
                                  </div>
                                </div>
                              ))}
                              <button type="button" className="board-import-slot-empty" disabled={committing} onClick={() => void openLibraryPicker({ kind: "slot", rowKey: row.key, slot: "reference" })}>+ 参考图</button>
                            </>
                          ) : (
                            <>
                              <div className="board-import-slot">
                                <span className="board-import-slot-label">{modeKey === "firstLast" ? "首帧" : "首帧图"}</span>
                                {row.firstFrame ? (
                                  <>
                                    <BoardImageThumb projectId={projectId} assetId={row.firstFrame.assetId} name={row.firstFrame.name} onZoom={(u, n) => setZoomed({ url: u, name: n })} />
                                    <small title={row.firstFrame.name}>{row.firstFrame.name}</small>
                                    <div className="board-import-slot-actions">
                                      <button type="button" disabled={committing} onClick={() => void openLibraryPicker({ kind: "slot", rowKey: row.key, slot: "firstFrame" })}>换图</button>
                                      <button type="button" disabled={committing} onClick={() => setRows(prev => prev.map(r => r.key === row.key ? { ...r, firstFrame: undefined } : r))}>清</button>
                                    </div>
                                  </>
                                ) : (
                                  <button type="button" className="board-import-slot-empty" disabled={committing} onClick={() => void openLibraryPicker({ kind: "slot", rowKey: row.key, slot: "firstFrame" })}>+ 选首帧</button>
                                )}
                              </div>
                              {modeKey === "firstLast" && (
                                <div className="board-import-slot">
                                  <span className="board-import-slot-label">尾帧</span>
                                  {row.lastFrame ? (
                                    <>
                                      <BoardImageThumb projectId={projectId} assetId={row.lastFrame.assetId} name={row.lastFrame.name} onZoom={(u, n) => setZoomed({ url: u, name: n })} />
                                      <small title={row.lastFrame.name}>{row.lastFrame.name}</small>
                                      <div className="board-import-slot-actions">
                                        <button type="button" disabled={committing} onClick={() => void openLibraryPicker({ kind: "slot", rowKey: row.key, slot: "lastFrame" })}>换图</button>
                                        <button type="button" disabled={committing} onClick={() => setRows(prev => prev.map(r => r.key === row.key ? { ...r, lastFrame: undefined } : r))}>清</button>
                                      </div>
                                    </>
                                  ) : (
                                    <button type="button" className="board-import-slot-empty" disabled={committing} onClick={() => void openLibraryPicker({ kind: "slot", rowKey: row.key, slot: "lastFrame" })}>+ 选尾帧</button>
                                  )}
                                </div>
                              )}
                            </>
                          )}
                        </div>
                      </td>
                    )}
                    <td>
                      <div className="board-import-prompt-cell">
                        <div className="board-import-name-row">
                          <input
                            aria-label={`第 ${idx + 1} 镜名称`}
                            value={row.name}
                            maxLength={80}
                            disabled={committing}
                            onChange={e => setRows(prev => prev.map(r => r.key === row.key ? { ...r, name: e.target.value } : r))}
                            placeholder={`镜头 ${String(idx + 1).padStart(2, "0")}`}
                          />
                          <input
                            aria-label={`第 ${idx + 1} 镜时长`}
                            type="number"
                            min={1}
                            max={MAX_SHOT_DURATION_SECONDS}
                            style={{ width: 62 }}
                            disabled={committing}
                            value={row.durationSeconds ?? defaultDuration}
                            title="单镜时长（秒）"
                            onChange={e => {
                              const n = Number(e.target.value);
                              setRows(prev => prev.map(r => r.key === row.key ? { ...r, durationSeconds: Number.isFinite(n) && n >= 1 ? Math.min(MAX_SHOT_DURATION_SECONDS, Math.round(n)) : undefined } : r));
                            }}
                          />
                        </div>
                        <textarea
                          aria-label={`第 ${idx + 1} 镜提示词`}
                          value={row.prompt}
                          disabled={committing}
                          onChange={e => setRows(prev => prev.map(r => r.key === row.key ? { ...r, prompt: e.target.value } : r))}
                          placeholder="输入或粘贴该镜头的视频提示词…"
                        />
                        {issue && <p className="board-import-row-err">{issue}</p>}
                      </div>
                    </td>
                    <td>
                      <div className="board-import-row-ops">
                        <button type="button" disabled={committing || idx === 0} title="整行上移" onClick={() => moveRow(idx, -1)}>↑ 上移</button>
                        <button type="button" disabled={committing || idx === rows.length - 1} title="整行下移" onClick={() => moveRow(idx, 1)}>↓ 下移</button>
                        {modeKey !== "text" && modeKey !== "firstLast" && idx < rows.length - 1 && (
                          <button type="button" disabled={committing} title="去掉当前行图片，把后面所有行的图片往前顺移一格（快速修正错位）" onClick={() => shiftImagesUpFrom(idx)}>删图前移</button>
                        )}
                        {modeKey === "firstLast" && (row.firstFrame || row.lastFrame) && (
                          <button type="button" disabled={committing} onClick={() => setRows(prev => prev.map(r => r.key === row.key ? { ...r, firstFrame: r.lastFrame, lastFrame: r.firstFrame } : r))}>互换首尾</button>
                        )}
                        <button type="button" disabled={committing} onClick={() => setRows(prev => prev.filter(r => r.key !== row.key))}>删除行</button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {rows.length > 0 && (
        <div className="board-import-foot">
          <span>
            已勾选 {checkedRows.length} / {rows.length} 个镜头
            {invalidCheckedCount > 0 ? `（其中 ${invalidCheckedCount} 行待补全）` : " · 全部就绪"}
          </span>
          <button type="button" className="primary" disabled={!canCommitBoard} onClick={() => void commitBoard()}>
            {committing ? "正在批量创建…" : `一键创建 ${checkedRows.length} 个分镜并配参`}
          </button>
        </div>
      )}

      {zoomed && (
        <div className="board-import-zoom" role="dialog" aria-label={`预览 ${zoomed.name}`} onClick={() => setZoomed(null)}>
          <div className="board-import-zoom-card" onClick={e => e.stopPropagation()}>
            <div className="board-import-drawer-head">
              <strong>{zoomed.name}</strong>
              <button type="button" onClick={() => setZoomed(null)}>关闭</button>
            </div>
            <img src={zoomed.url} alt={zoomed.name} />
          </div>
        </div>
      )}
    </div>
  );
}

function buildAiScriptGuide(entities: { name: string; kind: string }[] = []): string {
  const entityLines = entities.length
    ? [
        "- 项目已有角色/场景实体（如需引用可直接写 @实体名）：",
        ...entities.slice(0, 30).map(e => `  - @${e.name} (${e.kind})`),
      ]
    : [];
  return [
    "请按以下极简文本格式生成分镜脚本，保存为 prompts.txt 并和分镜图片放在同一个文件夹中（无需 JSON，无需任何 ID）：",
    "- 第一行可写全局默认参数：全局: 5s | 16:9 | 高清",
    "- 每个镜头用 ## 开头命名，下一行写 参数:（支持时长 1-11s / 画幅 16:9、9:16、1:1 / 清晰度 标清(0.5)、高清(0.98)、超清(2.0) 或 1344x768 / 图片名）",
    "- 图片绑定规则：",
    "  * 写 参考图: 01.png, hero.png → 自动走多图混合参考模式",
    "  * 写 首帧: 01_a.png | 尾帧: 01_b.png → 自动走首尾帧模式",
    "  * 写 首帧: 01.png 或省略图片名 → 自动按自然数字顺序配对文件夹里的图片",
    "  * 写 文生 → 纯文生视频（不绑图）",
    "- 也可直接写成行内标签：[6s 16:9 高清 参考:01.png,hero.png] 提示词正文…（导入时会自动剔除标签，只把纯提示词发给模型）",
    ...entityLines,
    "",
    "示例 prompts.txt：",
    "全局: 5s | 16:9 | 高清",
    "",
    "## 镜01 雨夜出场",
    "参数: 6s | 16:9 | 参考图: 01.png, hero.png",
    "A young woman in a red coat walks slowly through a neon-lit rain street, cinematic slow push in, moody atmospheric lighting.",
    "",
    "## 镜02 人物特写",
    "参数: 4s | 9:16 | 首帧: 02.png",
    "Close-up shot, she turns gently toward the camera, raindrops shimmering under glowing reflections.",
    "",
    "## 镜03 镜头拉远",
    "参数: 8s | 16:9 | 首帧: 03_a.png | 尾帧: 03_b.png",
    "Smooth camera pull back transitioning from the opening frame to reveal the entire glowing street.",
  ].join("\n");
}

export function FolderBatchImportPanel(props: FolderBatchImportProps) {
  const c = useFolderBatchImportController(props.projectId, props.onImported);
  const { preview: p, result: r } = c;
  const survivingBatch=r?.outcome==="completed"?r.batchId:r?.residual.batchId;
  const [tab, setTab] = useState<"board" | "folder">(props.initialTab ?? "folder");
  const [boardBusy, setBoardBusy] = useState(false);
  const [receiptPage,setReceiptPage]=useState(0);useEffect(()=>setReceiptPage(0),[r,props.projectId]);
  const [rowFilter,setRowFilter]=useState<"all"|"errors"|"selected">("all");
  const [requestedPage,setPage]=useState(0);useEffect(()=>{setPage(0);setRowFilter("all");},[p?.previewVersion,p?.sessionId]);
  const [copyingTemplate, setCopyingTemplate] = useState(false);

  const visibleRows=p?(rowFilter==="errors"?p.rows.filter(row=>row.errors.length>0):rowFilter==="selected"?p.rows.filter(row=>c.selected.has(row.rowKey)):p.rows):[];
  const pageCount=Math.ceil(visibleRows.length/50);
  const page = Math.min(requestedPage, Math.max(0, pageCount - 1));
  useEffect(() => { if (page !== requestedPage) setPage(page); }, [page, requestedPage]);
  const pageRows = visibleRows.slice(page * 50, (page + 1) * 50);
  const validPageRows = pageRows.filter(row => row.errors.length === 0);
  const allValidKeys = p ? p.rows.filter(row => row.errors.length === 0).map(row => row.rowKey) : [];
  const hasDuplicateNames = Boolean(p?.rows.some(row => row.errors.some(e => e.code === "DUPLICATE_SHOT_NAME")));
  const selectedTotalSeconds = p
    ? p.rows.filter(row => c.selected.has(row.rowKey)).reduce((sum, row) => sum + Math.max(0, row.durationSeconds || 0), 0)
    : 0;
  const modeBreakdown = useMemo(() => {
    if (!p?.rows.length) return "";
    const counts = new Map<string, number>();
    for (const row of p.rows) counts.set(row.mode, (counts.get(row.mode) ?? 0) + 1);
    return [...counts.entries()].map(([m, n]) => `${modes[m] ?? m} ${n}`).join(" · ");
  }, [p]);
  const generator=c.generators.find(g=>g.selectionRef===c.legacy.selectionRef);
  const receiptNames = new Map(r?.createdRows.map(row => [row.shotId, row.name]));

  const copyAiScriptRule = async () => {
    setCopyingTemplate(true);
    try {
      const entities = typeof productClient.storyEntities?.list === "function"
        ? await productClient.storyEntities.list(props.projectId, null).catch(() => [])
        : [];
      const guide = buildAiScriptGuide(entities);
      await navigator.clipboard?.writeText(guide);
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已复制 AI 分镜脚本规则（可直接发给 AI 生成 prompts.txt）", tone: "success" } }));
    } catch {
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "复制失败，请检查剪贴板权限", tone: "warning" } }));
    } finally {
      setCopyingTemplate(false);
    }
  };

  const copyAiTemplate = async () => {
    setCopyingTemplate(true);
    try {
      const [list, entities] = await Promise.all([
        typeof productClient.creation?.generatorsList === "function"
          ? productClient.creation.generatorsList(props.projectId, "video").catch(() => [])
          : Promise.resolve([]),
        typeof productClient.storyEntities?.list === "function"
          ? productClient.storyEntities.list(props.projectId, null).catch(() => [])
          : Promise.resolve([]),
      ]);
      const guide = buildAiManifestGuide(list, entities);
      await navigator.clipboard?.writeText(guide);
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已复制 JSON 清单模板（含可用生成器与已有实体）", tone: "success" } }));
    } catch {
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "复制失败，请检查剪贴板权限", tone: "warning" } }));
    } finally {
      setCopyingTemplate(false);
    }
  };

  const autoMatchedCleanly = Boolean(c.autoFolderMode && p && p.validCount > 0 && !p.rows.some(row => row.errors.some(e => e.code === "GENERATOR_UNAVAILABLE")));
  const legacyFieldset = c.legacyMode ? (
    <fieldset disabled={Boolean(c.busy)||Boolean(r)}>
      <legend>{c.autoFolderMode ? "手动统一覆盖视频生成器与参数（可选）" : "旧版清单：请选择生成器和默认参数"}</legend>
      <label>生成器<select value={c.legacy.selectionRef} onChange={e=>{const option=c.generators.find(g=>g.selectionRef===e.target.value);const preset=option?.resolutionPresets?.[0];c.setLegacy({...c.legacy,selectionRef:e.target.value,width:preset?.width??0,height:preset?.height??0});}}><option value="">请选择生成器</option>{c.generators.map(g=><option key={g.selectionRef} value={g.selectionRef} disabled={!g.availability}>{g.name} · {g.version} · {g.mode}{g.availability?"":"（不可用）"}</option>)}</select></label>
      <label>默认时长（秒）<input type="number" min={1} max={MAX_SHOT_DURATION_SECONDS} value={c.legacy.durationSeconds} onChange={e=>c.setLegacy({...c.legacy,durationSeconds:Number(e.target.value)})}/></label>
      <label>分辨率<select value={generator?.resolutionPresets?.find(v=>v.width===c.legacy.width&&v.height===c.legacy.height)?.id??""} onChange={e=>{const preset=generator?.resolutionPresets?.find(v=>v.id===e.target.value);if(preset)c.setLegacy({...c.legacy,width:preset.width,height:preset.height});}}><option value="">请选择分辨率</option>{generator?.resolutionPresets?.map(v=><option key={v.id} value={v.id}>{v.label} · {v.width} × {v.height}</option>)}</select></label>
      <p>{c.autoFolderMode ? "不修改则保留每条提示词各自写明的时长、画幅与模式；若需全部强制统一，选择后点“重新校验”。" : "选择后点“重新校验”。"}</p>
    </fieldset>
  ) : null;

  return <section className="folder-import" aria-label="文件夹批量导入" aria-busy={Boolean(c.busy) || boardBusy}>
    <header>
      <div>
        <h2>批量导入分镜与提示词</h2>
        <p>选中一个包含参考图与提示词的文件夹即可自动识别每镜时长、画幅、参考图并批量入队生成。</p>
      </div>
      <button type="button" onClick={props.onCancel} disabled={c.busy === "commit" || boardBusy}>关闭</button>
    </header>
    <div className="folder-import-tabs" role="tablist" aria-label="批量导入方式">
      <button type="button" role="tab" aria-selected={tab === "folder"} disabled={boardBusy} onClick={() => setTab("folder")}>文件夹全自动导入（推荐）</button>
      <button type="button" role="tab" aria-selected={tab === "board"} disabled={Boolean(c.busy)} onClick={() => setTab("board")}>可视化手动配对表</button>
    </div>
    {tab === "board" ? (
      <VisualBatchBoard
        projectId={props.projectId}
        onImported={props.onImported}
        onCancel={props.onCancel}
        onOpenBatch={props.onOpenBatch}
        onBusyChange={setBoardBusy}
      />
    ) : <>
    <div className="folder-import-actions">
      <button type="button" className={p ? undefined : "primary"} disabled={Boolean(c.busy)} onClick={() => void c.pick()}>{c.busy === "pick" ? "正在选择…" : p ? "重新选择文件夹" : "选择文件夹"}</button>
      <button type="button" disabled={Boolean(c.busy) || copyingTemplate} onClick={() => void copyAiScriptRule()}>{copyingTemplate ? "正在生成…" : "复制 AI 分镜脚本规则"}</button>
      <button type="button" disabled={Boolean(c.busy) || copyingTemplate} onClick={() => void copyAiTemplate()}>复制 JSON 清单模板</button>
    </div>
    {!p && (
      <div className="folder-import-guide-card">
        <p><strong>把参考图 + <code>prompts.txt</code>（或 <code>分镜.md</code>）放进同一个文件夹，直接选目录即可全自动识别</strong>：</p>
        <ul>
          <li><strong>提示词自带参数（自动识别并从正文剥离）</strong>：支持写 <code>全局: 5s | 16:9 | 高清</code>，每镜写 <code>参数: 6s | 9:16 | 参考图: 01.png, hero.png</code> 或行内标签 <code>[6s 16:9 首帧:01.png 尾帧:02.png]</code>，系统自动匹配对应视频生成器与画幅。</li>
          <li><strong>省略图片名也能自动配对</strong>：按自然数字顺序（<code>1.png → 2.png → 10.png</code>）自动配对，也支持同名 <code>01.png + 01.txt</code>。</li>
          <li><strong>一步入队自动出片</strong>：预览确认后勾选「导入后立即开始生产」即可自动按队列依次生成全部视频。</li>
        </ul>
        <details>
          <summary>查看 prompts.txt 极简写法示例</summary>
          <pre>{`全局: 5s | 16:9 | 高清\n\n## 镜01 雨夜出场\n参数: 6s | 9:16 | 参考图: 01.png, hero.png\nA young woman walks slowly through neon rain...\n\n## 镜02 镜头过渡\n[4s 16:9 首帧:02_a.png 尾帧:02_b.png] Smooth camera pull back...`}</pre>
        </details>
      </div>
    )}
    {c.error && <p className="folder-import-error" role="alert">{c.error}</p>}
    {c.busy && <p role="status">{c.busy === "commit" ? "正在导入，请勿关闭窗口…" : c.busy === "pick" ? "正在等待系统文件夹选择窗口…" : "正在读取并校验文件夹…"}</p>}
    {p && <>
      <dl className="folder-import-summary">
        <div><dt>文件夹</dt><dd>{p.displayRootName}</dd></div>
        <div><dt>清单镜头</dt><dd>{p.shotCount}</dd></div>
        <div><dt>可导入</dt><dd>{p.validCount}</dd></div>
        <div><dt>存在错误</dt><dd>{p.errorCount}</dd></div>
        <div><dt>已选预计时长</dt><dd>{selectedTotalSeconds}s（{String(Math.floor(selectedTotalSeconds / 60)).padStart(2, "0")}:{String(selectedTotalSeconds % 60).padStart(2, "0")}）</dd></div>
        {modeBreakdown && <div><dt>模式分布</dt><dd>{modeBreakdown}</dd></div>}
      </dl>
      {p.errors.map((e, i) => <p key={i} className="folder-import-error" role="alert">{e.message}</p>)}
      {p.warnings.map((w, i) => <p key={i} role="note">{w}</p>)}
      {legacyFieldset && (autoMatchedCleanly ? <details><summary>已自动识别各镜参数与生成器（如需手动统一覆盖，点此展开）</summary>{legacyFieldset}</details> : legacyFieldset)}
      <div className="folder-import-actions">
        <label>镜头名称后缀<input value={c.suffix} maxLength={80} disabled={Boolean(c.busy) || Boolean(r)} onChange={e => c.setSuffix(e.target.value)} placeholder="再次导入需填写后缀" /></label>
        {hasDuplicateNames && !r && (
          <button type="button" disabled={Boolean(c.busy)} onClick={() => {
            const now = new Date();
            const pad = (n: number) => String(n).padStart(2, "0");
            c.setSuffix(`-${pad(now.getHours())}${pad(now.getMinutes())}`);
          }}>自动填不重名后缀</button>
        )}
        <button type="button" disabled={Boolean(c.busy) || Boolean(r)} onClick={() => void c.rescan(undefined, c.autoFolderMode)}>重新校验</button>
        {c.dirty && <span role="status">预览参数已修改，需重新校验才能导入。</span>}
      </div>
      {p.characters.length > 0 && <details><summary>实体计划（{p.characters.length}）</summary><ul>{p.characters.map(e => <li key={`${e.kind}:${e.name}`}>{e.name} · {e.kind === "scene" ? "场景" : e.kind === "prop" ? "道具" : e.kind === "composition" ? "构图" : "角色"} · {e.action === "reuse" ? "使用已有" : `新建，${e.imageCount} 张图`}</li>)}</ul></details>}
      {!r && <div className="folder-import-selection" aria-label="导入选择范围"><div role="group" aria-label="预览行筛选" className="folder-import-row-filters"><button type="button" aria-pressed={rowFilter === "all"} onClick={() => { setRowFilter("all"); setPage(0); }}>全部（{p.rows.length}）</button><button type="button" aria-pressed={rowFilter === "errors"} onClick={() => { setRowFilter("errors"); setPage(0); }}>仅看报错（{p.errorCount}）</button><button type="button" aria-pressed={rowFilter === "selected"} onClick={() => { setRowFilter("selected"); setPage(0); }}>仅看已选（{c.selected.size}）</button></div><button type="button" disabled={Boolean(c.busy)} onClick={() => c.selectRows([])}>清空全部选择</button><button type="button" disabled={Boolean(c.busy) || allValidKeys.length === 0} onClick={() => c.selectRows(allValidKeys)}>全选全部可导入（{allValidKeys.length}）</button><button type="button" disabled={Boolean(c.busy) || validPageRows.length === 0} onClick={() => c.selectRows(validPageRows.map(row => row.rowKey))}>仅选当前页可导入镜头</button><button type="button" disabled={Boolean(c.busy)} onClick={() => c.selectRows(p.rows.filter(row => !row.errors.length).slice(0,p.maxQueueItems).map(row => row.rowKey))}>仅选前 {p.maxQueueItems} 个可导入镜头</button>{c.newlyValid.size > 0 && <p role="status">有 {c.newlyValid.size} 个镜头重新校验后新通过，尚未勾选。<button type="button" disabled={Boolean(c.busy)} onClick={c.selectNewlyValid}>勾选本次新通过镜头</button></p>}</div>}
      <div className="folder-import-table" tabIndex={0} aria-label="导入镜头预览"><table><thead><tr><th>选择</th><th>镜头与提示词</th><th>模式 / 生成器</th><th>参数</th><th>输入</th><th>校验结果</th></tr></thead><tbody>{pageRows.map(row => <tr key={row.rowKey}><td><input type="checkbox" aria-label={`导入 ${row.name}`} checked={c.selected.has(row.rowKey)} disabled={Boolean(c.busy) || Boolean(r) || row.errors.length > 0} onChange={() => c.toggle(row.rowKey)} /></td><td><strong>{row.rowNumber}. {row.name}</strong><p>{row.promptPreview}</p><details><summary>展开完整提示词与顺序参考图</summary><pre>{row.prompt}</pre><dl>{([["style","风格"],["summary","摘要"],["soundscape","环境声"],["music","配乐"]] as const).map(([key,label]) => <div key={key}><dt>{label}</dt><dd>{row.promptSections[key] || "未填写"}</dd></div>)}</dl><ol aria-label={`${row.name} 实际参考图编号`}>{row.referencePreview.map((identity,index) => <li key={`${index}:${identity}`}>Picture {index+1} · {identity.startsWith("managed:") ? "项目已有素材" : identity}</li>)}</ol></details></td><td>{modes[row.mode] ?? row.mode}<small>{row.generatorLabel}</small></td><td>{row.durationSeconds}s<small>{row.width} × {row.height}</small></td><td>{row.firstFrame && <small>首帧：{row.firstFrame}</small>}{row.lastFrame && <small>尾帧：{row.lastFrame}</small>}{row.mode === "reference" && <small>参考图：{row.referenceCount} 张</small>}{row.entityNames.join("、")}{row.openingComposition && <small>开场构图：{row.openingComposition}</small>}{row.endingComposition && <small>结尾构图：{row.endingComposition}</small>}</td><td>{row.errors.length === 0 ? <span>可导入</span> : row.errors.map((e, i) => <p className="folder-import-error" key={i}>{e.message}</p>)}{row.warnings.map((w, i) => <p key={i}>{w.message}</p>)}</td></tr>)}{!visibleRows.length && <tr><td colSpan={6}>当前筛选下没有匹配的预览镜头。</td></tr>}</tbody></table></div>{pageCount>1 && <nav aria-label="导入预览分页"><button type="button" disabled={page===0} onClick={()=>setPage(n=>n-1)}>上一页</button><span>{page+1} / {pageCount}</span><button type="button" disabled={page+1>=pageCount} onClick={()=>setPage(n=>n+1)}>下一页</button></nav>}
      {!r && <footer><p>已选 {c.selected.size} / {p.shotCount}，未选中的 {p.shotCount - c.selected.size} 个镜头不会导入。</p><label><input type="checkbox" checked={c.createQueue} disabled={Boolean(c.busy)} onChange={e => c.setCreateQueue(e.target.checked)} />创建待启动批次</label><label><input type="checkbox" checked={c.autoStart} disabled={Boolean(c.busy) || !c.createQueue} onChange={e => c.setAutoStart(e.target.checked)} />导入后立即开始生产（占用 GPU，需明确勾选）</label>{c.overQueueLimit && <p className="folder-import-error">单个批次最多 {p.maxQueueItems} 个镜头；请减少选择，或取消创建批次。</p>}<button type="button" className="primary" disabled={!c.canCommit} onClick={() => void c.commit()}>{c.busy === "commit" ? "正在导入…" : c.autoStart ? "导入并开始生产" : "确认导入"}</button></footer>}
      {r && <section aria-label="导入最终回执" role="status"><h3>{r.outcome === "completed" ? "导入完成" : r.outcome === "compensated" ? "导入失败，本次新增的数据已撤回" : "导入未完成，有部分数据未能撤回"}</h3><p>本次创建镜头 {r.createdShotIds.length} 个、角色/场景 {r.createdEntityIds.length} 个、素材 {r.importedAssetIds.length} 个；批次 {r.itemCount} 项。{r.autoStarted ? "已开始生产。" : "未自动开始生产。"}</p>{r.failure && <p className="folder-import-error">{r.failure.message}</p>}{r.compensationErrors.map((e, i) => <p className="folder-import-error" key={i}>{targetLabels[e.target] ?? "数据"}：{e.message}</p>)}{r.outcome === "partial" && <p>未撤回：镜头 {r.residual.shotIds.length} 个、角色/场景 {r.residual.entityIds.length} 个、素材 {r.residual.assetIds.length} 个{r.residual.batchId ? "、批次 1 个" : ""}，请手动检查。</p>}{r.warnings.map((w, i) => <p key={i}>{w}</p>)}{r.outcome === "completed" && r.createdShotIds.length > 0 && <section aria-label="逐镜控制"><p>每个镜头均可单独修改提示词、参考素材并重新生成；不会重跑其他镜头。原批次仍保留导入时的冻结输入。</p><div className="folder-import-receipt-shots">{props.onOpenShot && r.createdShotIds.slice(receiptPage*50,(receiptPage+1)*50).map((id, index) => { const ordinal = receiptPage*50 + index; const name = receiptNames.get(id) ?? `导入镜头 ${ordinal + 1}`; return <button type="button" key={id} onClick={() => void props.onOpenShot?.(id)}>编辑「{name}」</button>; })}</div>{r.createdShotIds.length > 50 && <nav aria-label="已导入镜头分页"><button type="button" disabled={receiptPage === 0} onClick={()=>setReceiptPage(n=>n-1)}>上一组镜头</button><span>{receiptPage+1} / {Math.ceil(r.createdShotIds.length/50)}</span><button type="button" disabled={(receiptPage+1)*50 >= r.createdShotIds.length} onClick={()=>setReceiptPage(n=>n+1)}>下一组镜头</button></nav>}<div className="folder-import-actions"><button type="button" onClick={props.onCancel}>返回分镜逐镜编辑</button>{survivingBatch && props.onOpenQueue && <button type="button" className="primary" onClick={() => props.onOpenQueue?.(survivingBatch!)}>查看生产队列</button>}</div></section>}{!(r.outcome === "completed" && r.createdShotIds.length > 0) && survivingBatch && props.onOpenQueue && <button type="button" onClick={() => props.onOpenQueue?.(survivingBatch!)}>查看生产队列</button>}<p>如需再次导入，请重新选择文件夹。</p></section>}
    </>}
    </>}
  </section>;
}
/** Native dialog supplies focus trapping and Escape, retaining the workbench draft. */
export function FolderBatchImportDialog(props: FolderBatchImportProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const el = ref.current; if (el && !el.open) el.showModal(); }, []);
  return <dialog ref={ref} className="folder-import-dialog" aria-label="文件夹批量导入" onCancel={e => { e.preventDefault(); /* Close is explicitly disabled while committing. */ ref.current?.querySelector<HTMLButtonElement>(".folder-import header button:not(:disabled)")?.click(); }}><FolderBatchImportPanel {...props} /></dialog>;
}
