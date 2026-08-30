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

## 最终验收记录

本次真人 UAT 使用 `0.8.1` 最新 release build，ComfyUI `/system_stats` 与
`/object_info` 均返回 HTTP 200。已验证 Batch B 的生产、监控、播放、文件位置、
筛选、成品查看、清单导出、外部生产包暂时改名后的数据库真相，以及重启后的恢复。

Batch B：`pbt_53a0c3e2ea2f48f998995dcc5af6490f`，3/3 item 成功，3/3 Task 成功。
对应视频 Asset 与文件大小为：

- `ast_0145b8d8-7e39-48c6-8a71-96f023056a12` — 958896 bytes
- `ast_525382a2-f3f6-44d5-b0bc-9808ebf75220` — 1941922 bytes
- `ast_71ef68a5-9694-46c8-83b7-34f149d9840a` — 1083272 bytes

Selected-batch manifest UAT：Batch B 导出为
`LOCAL_DELIVERY_MANIFEST_pbt_53a0c3e2ea2f48f998995dcc5af6490f (1).json`；历史 Batch A
`pbt_3e7baa7941d744869970d4dbddc24c0a` 导出为独立的
`LOCAL_DELIVERY_MANIFEST_pbt_3e7baa7941d744869970d4dbddc24c0a (1).json`。A → B
快速切换后再次导出仍为 Batch B，3 个 item 均与对应数据库 Batch 和 Asset truth
一致，未混入 A/B item。

```text
IMPLEMENTATION_SHA = 053b1942487414a6dc1ddd913a0e900a555dcae7
MANIFEST_FIX_SHA = 50d502f15289020bcf1edc3f5c5dbb5ead5b830b
SOURCE_CI = 33319070741 success

RUST = PASS (cargo fmt/check/test --all-targets; 697 passed, 0 failed, 1 ignored)
FRONTEND = PASS (98 files / 402 tests)
TSC = PASS
BUILD = PASS (pnpm build; pnpm tauri build)
LIVE_UAT = PASS
COMFY_PREFLIGHT = PASS
MONITOR_VISIBLE = PASS
MONITOR_SELECTED_BATCH = B
LIVE_PROGRESS = PASS
RUNNING_STATE = PASS
REAL_H3 = PASS
VIDEO_ASSET = PASS
VIDEO_OPEN = PASS
OPEN_FILE_LOCATION = PASS
COMPLETED_STATE = PASS
FILTER_SUCCEEDED = PASS
MANIFEST_BATCH_B = PASS
SELECTED_BATCH_MANIFEST = PASS
STALE_MANIFEST_GUARD = PASS
MANIFEST_ASSET_TRUTH = PASS
MANIFEST_ID_GUARD_TEST = PASS
MANIFEST_REFRESH_TEST = PASS
PACKAGE_SOURCE_REQUIRED_AFTER_IMPORT = NO
RESTART_MONITOR_TRUTH = PASS
AUTO_START = NO
AUTO_RETRY = NO
MANUAL_RETRY = NOT_TRIGGERED
FILTER_FAILED = NOT_TRIGGERED
COMPLETED_STOP = AUTOMATED_PASS
MIGRATION = 025
MIGRATION026 = ABSENT
DEV_068 = PASS
```
