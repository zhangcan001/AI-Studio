import type { RecipeField, RecipeViewModel } from "../../types/generation";
import { runtimeKindFor, runtimeKindLabel } from "./pack05";

interface Props {
  recipe: RecipeViewModel;
}

export function CreationModeHint({ recipe }: Props) {
  const kind = runtimeKindFor(recipe);
  const normalizedMode = `${recipe.category} ${recipe.mode}`.toLocaleLowerCase();
  const mediaFields = recipe.fields.filter((field): field is Exclude<RecipeField, Extract<RecipeField, { type: "textarea" | "integer" | "number" | "seed" }>> => ["image", "images", "video", "videos", "audio", "audios"].includes(field.type));
  const requiredMediaFields = mediaFields.filter((field) => field.required).length;
  const isReferenceMode = /(reference|image.?to.?image|img2img|参考|图生图)/i.test(normalizedMode);
  const message = isReferenceMode
    ? "参考创作：先准备引用素材，再补齐文字和数字参数；素材只会填入输入项，不会自动提交。"
    : kind === "video"
      ? "视频创作：先确认参考素材、时长和输出参数；批量任务会按队列顺序执行。"
      : kind === "audio"
        ? "音频创作：先检查音频输入与文本参数，再提交单次或批量任务。"
        : "图片创作：可以从文字开始，也可以把素材库中的图片加入对应输入项。";

  return (
    <section className="creation-mode-hint" aria-label="创作模式提示">
      <div>
        <span className="section-label">{runtimeKindLabel(kind)}创作</span>
        <p>{message}</p>
      </div>
      <small>{mediaFields.length ? `${requiredMediaFields} 个必填素材输入 · ${mediaFields.length} 个素材输入` : "当前工作流没有素材输入"}</small>
    </section>
  );
}
