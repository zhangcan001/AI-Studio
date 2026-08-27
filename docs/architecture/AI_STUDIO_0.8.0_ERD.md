# AI Studio 0.8.0 Data Foundation ERD

DEV-057 冻结的最小关系只有 Script/Draft 两层，产品运行版本仍是 0.7.0。

```text
┌──────────────┐
│   projects   │
│ id (PK)      │
└──────┬───────┘
       │ project_id  ON DELETE CASCADE
       ▼
┌──────────────────────────────┐
│       script_sources         │
│ id (PK) = scr_<UUID>         │
│ project_id (FK)              │
│ format                       │
│ original_filename?           │
│ source_checksum              │
│ source_bytes                 │
│ source_text                  │
│ schema_version               │
└──────────────┬───────────────┘
               │ (source_id, project_id) FK
               ▼
┌──────────────────────────────────────┐
│        script_import_drafts           │
│ id (PK) = drev_<UUID>                 │
│ draft_id = drf_<UUID>                 │
│ project_id (FK)                       │
│ source_id (FK, same project)          │
│ revision / previous_revision_id       │
│ schema + contract versions            │
│ parser/provider metadata              │
│ summary_json / payload_json           │
│ payload_checksum                      │
└──────────────────────────────────────┘
```

## Boundary decisions

- `source_text` is stored once in `script_sources`; it is not copied into every draft node or formal asset.
- `script_import_drafts` is a document-oriented immutable revision store. There are no `draft_episodes`, `draft_scenes`, `draft_shots`, `draft_entity_matches` or `draft_node_index` tables in Migration 025.
- The previous-revision foreign key and repository transaction enforce same-project, same-draft adjacency. The database trigger rejects in-place revision updates.
- There is deliberately **no FK from Draft to formal `shots`**, and no Draft-to-Profile/ReferenceSet/Batch/Task/Queue/Generation/Comfy relation. Draft and Formal Structure are separate layers until a future explicit Promote DEV.
- Manifest remains version 2; these working tables are included in Backup 15, not in Manifest 2.
