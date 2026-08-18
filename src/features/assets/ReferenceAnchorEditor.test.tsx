import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { ReferenceAnchorEditor } from "./ReferenceAnchorEditor";

describe("参考锚点编辑器", () => {
  it("shows ordered references and the primary action without a second asset picker", () => {
    const html = renderToStaticMarkup(
      <ReferenceAnchorEditor
        selectedAssets={[]}
        onSave={vi.fn(async () => undefined)}
        onCancel={vi.fn()}
      />,
    );
    expect(html).toContain("创建参考锚点");
    expect(html).toContain("添加已选择图片");
    expect(html).toContain("暂无参考图");
    expect(html).not.toContain("导入本地素材");
  });
});
