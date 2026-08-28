# AI Studio Production Package V1

这是一份给 ChatGPT、Claude、Gemini 或人工工具使用的可复制输入规范。生成一个目录，放入 `production-package.json`，再按 JSON 中的相对路径放入媒体文件。AI Studio 会先预览和校验；只有用户确认后才会创建生产批次。

## 完整示例

```json
{
  "schemaVersion": 1,
  "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
  "packageId": "kṣitigarbha-ep01-v1",
  "name": "EP01",
  "description": "外部智能体准备的第一集视频生产包",
  "createdBy": "agent",
  "createdAt": "2026-08-28T10:00:00Z",
  "source": "storyboard-v1",
  "defaults": {
    "durationSeconds": 5,
    "width": 864,
    "height": 480,
    "mode": "I2V"
  },
  "items": [
    {
      "id": "EP01-SC01-SH001",
      "name": "镜头001",
      "text": "佛陀端坐莲台",
      "imagePrompt": "可选，仅供追溯；不会自动生成图片",
      "videoPrompt": "佛陀缓缓抬眼，衣袂随风轻动，镜头轻微推进，电影感光影。",
      "episode": "EP01",
      "scene": "SC01",
      "durationSeconds": 5,
      "width": 864,
      "height": 480,
      "mode": "I2V",
      "firstFrame": "images/SH001.png",
      "referenceImages": ["references/lotus.png"],
      "referenceAudios": [],
      "referenceVideos": []
    }
  ]
}
```

`packageId`、item `id`、episode 和 scene 是外部显示标签，不要填入数据库中的 Shot/Asset/Task/Batch ID，也不要在包中提供内部 workflow 或 recipe 控制字段。

## 最小例子

```json
{
  "schemaVersion": 1,
  "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
  "name": "EP01",
  "items": [
    {
      "id": "EP01-SH001",
      "name": "镜头001",
      "videoPrompt": "人物抬头，镜头缓慢推进。"
    }
  ]
}
```

没有图片时，使用当前可用的 text-to-video 模式；如要走主路径，请提供 `firstFrame`。

## Image-to-Video

```json
{
  "schemaVersion": 1,
  "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
  "name": "I2V example",
  "defaults": { "durationSeconds": 5, "width": 864, "height": 480, "mode": "I2V" },
  "items": [
    {
      "id": "SH001",
      "name": "首帧到视频",
      "videoPrompt": "角色缓慢转身，布料和头发自然摆动。",
      "firstFrame": "images/SH001.png"
    }
  ]
}
```

## First/Last Frame

```json
{
  "schemaVersion": 1,
  "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
  "name": "transition",
  "items": [
    {
      "id": "SH002",
      "name": "起止帧过渡",
      "videoPrompt": "镜头从庭院平稳移动到人物近景，动作连续自然。",
      "mode": "FIRST_LAST",
      "firstFrame": "images/SH002-first.png",
      "lastFrame": "images/SH002-last.png"
    }
  ]
}
```

## Reference Images

```json
{
  "schemaVersion": 1,
  "packageType": "AI_STUDIO_VIDEO_PRODUCTION",
  "name": "reference example",
  "items": [
    {
      "id": "SH003",
      "name": "角色参考",
      "videoPrompt": "角色向右走过画面，保持服装和面部特征一致。",
      "mode": "REFERENCE_IMAGE",
      "referenceImages": [
        "references/character.png",
        "references/costume.png",
        "references/location.png"
      ]
    }
  ]
}
```

数组顺序会冻结到现有 H3 generation values；不要依赖文件名排序。

## 目录结构与文件名

```text
ProductionPackage/
├─ production-package.json
├─ images/
│  ├─ EP01-SH001.png
│  └─ EP01-SH002-first.png
├─ references/
│  ├─ character.png
│  └─ costume.png
└─ audio/
   └─ ambience.wav
```

路径必须相对于 `ProductionPackage/`，例如 `images/EP01-SH001.png`。不要使用 `C:\...`、`/tmp/...`、`\\server\...`、`../...`、`~/...` 或 `https://...`。建议文件名使用稳定的外部 item ID 和明确的 `first`/`last` 后缀；文件名不是 formal ID。

## 支持的模式、尺寸和时长

外部模式别名由 Inspector 映射到当前 H3 canonical mode，常用别名包括 `I2V`/`IMAGE_TO_VIDEO`、`FIRST_LAST`、`REFERENCE_IMAGE`/`REFERENCE_IMAGES` 和 `T2V`/`TEXT_TO_VIDEO`。最终能力以当前安装的 H3 recipe 为准，不接受包内 workflow/recipe ID。

当前 H3 输出尺寸：`608×352`、`736×416`、`864×480`、`960×544`、`1056×608`、`1152×640`、`1216×672`、`1280×736`、`1344×768`、`1376×768`、`1504×832`、`1664×928`、`1824×1024`、`1920×1088`。

当前时长范围为 `1–15` 秒。若使用 defaults，item 未填写的 duration/width/height/mode 会继承 defaults；item 字段覆盖 defaults。

## 限制

- 一个 package 最多 500 个 item；超过时返回 `PACKAGE_TOO_LARGE`，不会截断。
- `videoPrompt` 必须为非空 UTF-8，单项不超过 64 KiB。
- Inspector 预览 Prompt 最多 300 个字符；完整 Prompt 仅用于后端生成，不写入日志预览。
- 图片、参考图片、音频和视频必须通过现有媒体校验与 SourceAssetImport；格式、大小、尺寸和 SHA-256 不合格会阻止提交。
- 包最多按现有 ProductionQueue/H3 下游上限分块创建批次，保持 JSON item 顺序；不会自动 Start。
- `workflowVersionId`、`recipeId`、`taskId`、`batchId`、`assetId`、`comfyPromptId`、`selectedVideoAssetId` 即使出现在未知字段中也不会执行。

## 错误排查

- `PACKAGE_JSON_INVALID`：检查 JSON 逗号、引号和 UTF-8 编码。
- `PACKAGE_SCHEMA_UNSUPPORTED`：确认 `schemaVersion` 为数字 `1`，`packageType` 为精确字符串 `AI_STUDIO_VIDEO_PRODUCTION`。
- `PACKAGE_PATH_INVALID`：改用 package root 下的相对路径；不要使用绝对路径、URL、`..` 或目录链接。
- `PACKAGE_MEDIA_MISSING` / `PACKAGE_MEDIA_INVALID`：确认文件名大小写、格式、尺寸和文件权限；替换后重新 Inspect。
- `PACKAGE_MEDIA_CHANGED`：Inspect 后文件被替换或修改；重新扫描并再次预览。
- `PACKAGE_PROMPT_EMPTY` / `PACKAGE_PROMPT_TOO_LARGE`：填写非空 Prompt，并控制在 64 KiB 内。
- `PACKAGE_MODE_UNSUPPORTED`：使用 `I2V`、`FIRST_LAST`、`REFERENCE_IMAGE(S)` 或 `T2V` 等当前支持的别名。
- `PACKAGE_RESOLUTION_UNSUPPORTED` / `PACKAGE_DURATION_INVALID`：使用上面的 14 档尺寸和 1–15 秒时长。
- `PACKAGE_DUPLICATE_ITEM_ID`：为重复 item 提供不同的外部 ID；系统不会自动重命名。
