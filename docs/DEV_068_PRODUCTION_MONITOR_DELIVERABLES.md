# DEV-068 — Production Monitor & Deliverables V1

## 产品范围

Production Monitor 位于现有 ShotWorkspace 的 production mode 下方，承接
Production Package Quick Flow 的 Create → Manual Start → Queue → Monitor
流程。它是现有 `ProductionBatch`、`ProductionBatchItem`、`Task`、`Asset` 和
`ProductionQueue` 的展示投影，不创建第二套结果、任务、队列或执行模型。

版本保持 `0.8.1`；本变更不新增 migration、tag 或 release，migration 上限仍为
025，Migration026 不应出现。

## 数据来源与 Asset truth

Host 按当前 focused batch 读取现有 queue detail 与批量审核生产力投影，再将
批次摘要和 item rows 传给无执行副作用的 `ProductionMonitor` presentational
component。item 顺序按 production ordinal 升序；成品候选来自数据库 Task/Asset
关系，不能重新读取或依赖已经删除的 Production Package 目录。

成功视频的播放复用现有 `ProductionAssetPreview`。列表不创建 video 元素、不
预加载媒体；点击播放后才打开既有预览。文件位置操作只提交 project/batch/item/
asset IDs，由 Rust service 从数据库 Asset 的绝对 `storage_path` 解析并交给
Tauri opener；客户端不接受任意路径或 shell 命令。

## 刷新与安全边界

- 只存在一个 batch-level 3 秒刷新计时器，不为 item 建立 polling。
- 请求有 in-flight guard；前一次请求未结束时不会堆积下一次请求。
- 页面隐藏时暂停刷新，恢复可见时立即刷新；batch 切换和卸载都会清理计时器。
- `COMPLETED`、`FAILED`、`CANCELLED` 或全部 item 进入终态后停止高频刷新。
- Retry 只调用既有 ProductionQueue requeue API，必须由用户点击触发；不会自动
  retry，也不会自动 start 当前或下一个 batch。

## 监控与交付 UX

摘要展示 total、pending、running、succeeded、failed、cancelled/skipped；终态
进度为 `succeeded + failed + cancelled + skipped` / total，并单独显示成功率。
支持全部、生成中、失败、已完成筛选，失败行展示可读错误和错误码，默认每页
50 项并保持 ordinal 顺序。

完成批次提供查看成品、打开成品文件夹和选择下一个生产包等动作；只有成功
Asset 记录可用时才显示相应结果操作。交付清单只应导出数据库 truth 的索引，
标识为 `LOCAL_DELIVERY_MANIFEST`，不复制、移动或重命名媒体。

## 性能与验收

500 items 使用 50/page，不一次渲染 500 个视频，也不执行 500 个独立
`getAsset` 请求。应验证批量读路径、失败筛选、人工 retry、隐藏页暂停、完成后
停止和重启后仍从数据库恢复 batch/task/asset truth。

Targeted tests:

- `pnpm test -- ProductionMonitor`
- `pnpm test -- ProductionQueueDrawer`
- `pnpm test -- ShotWorkspace.production`

最终验收记录（由主线串行 gate 更新）：

```text
RUST = PASS (cargo check --all-targets; 697 lib tests + integration suites)
FRONTEND = PASS (98 files / 400 tests)
TSC = PASS
BUILD = PASS (pnpm build; pnpm tauri build)
LIVE_UAT = BLOCKED (0.8.1 installer launch and initial project view passed; the installed-app
production flow was not triggered because desktop screenshot capture is unsupported and
accessibility click geometry was unavailable in the test environment)
REAL_H3 = NOT_TRIGGERED
```
