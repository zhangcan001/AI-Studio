import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import { AssetVideoBatchWorkspace } from "./AssetVideoBatchWorkspace";

it("retains the historical queue route, without local import, implicit mapping or auto-start", () => {
 const html=renderToStaticMarkup(<AssetVideoBatchWorkspace projectId="p" comfyConnected productionAdmission={{busy:false}} onAdmissionChanged={vi.fn().mockResolvedValue(undefined)} onOpenTask={vi.fn()} onBackToAssets={vi.fn()} onOpenProductionQueue={vi.fn()} />);
 expect(html).toContain("请改用镜头正式输入槽");
 expect(html).toContain("旧版批量入口已停用");
 expect(html).toContain("返回资产库"); expect(html).toContain("打开生产队列");
 for(const obsolete of ["创建视频批次","加入队列并开始","导入后立即开始生成","分段自动识别","ShotBatchAssetBindPanel","model-version-selector"]) expect(html).not.toContain(obsolete);
});
