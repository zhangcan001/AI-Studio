# DEV-114 Final Result Visibility

## User problem

After production, users could reach completed results through existing navigation, but the relationship between the selected result, its Shot, and its Asset was not visible from one first-stop surface. File-location actions also lived in the Production Monitor.

## Delivered surface

The Project Command Center renders **最终结果** for the existing completed-shot aggregate facts:

- **Selected Result** — the existing selected video asset, falling back to selected image asset according to the backend aggregate.
- **Shot** — the exact existing completed Shot target.
- **Asset** — the exact existing selected Asset target.
- **File** — the existing production batch monitor path when the bounded completed production item provides a batch target.

The card is read-only. It does not select a result, mutate review state, create an export record, or create a new deliverable object.

## Navigation contract

All links remain project-scoped through the existing Project Command Center navigation callback:

- `查看镜头` → existing Shot workspace with `shotId`.
- `打开资产` / `打开已选择结果` → existing Asset workspace with `assetId` and the related `shotId`.
- `查看文件位置` → existing Production workspace with the exact `batchId` when available; the existing Production Monitor then owns file-location access.

Technical IDs are carried in navigation requests but are not the first visual content of the card. The card uses business labels and the existing human-readable completed-item label.

## Limits and fallback

`AssetView` intentionally does not expose storage paths. If an aggregate completed item has no batch target, the card identifies the result as available in the Asset preview rather than fabricating a file path. This preserves the existing security boundary and avoids bypassing Production Monitor file access.

## Acceptance mapping

The focused frontend test verifies that a selected final result exposes exact Shot, Asset, and Production Monitor navigation targets without starting production or adding a new domain model.

