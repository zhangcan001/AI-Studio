import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AssetDeleteDialog } from "./AssetDeleteDialog";

describe("素材删除确认", () => {
  it("explains the irreversible project-file deletion before inspection completes", () => {
    const html = renderToStaticMarkup(
      <AssetDeleteDialog
        projectId="project-1"
        assets={[{
          id: "ast_one",
          assetType: "image",
          category: "source_image",
          name: "Reference",
          originalName: "reference.png",
          mimeType: "image/png",
          fileSize: 12,
          createdAt: "2026-01-01T00:00:00Z",
          isFavorite: false,
          tags: [],
        }]}
        onClose={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    expect(html).toContain("删除素材");
    expect(html).toContain("此操作会删除 AI Studio 项目中的素材文件，无法撤销");
    expect(html).toContain("正在检查素材引用");
  });
});
