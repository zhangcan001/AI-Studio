//! Stable Draft node reconciliation for source reparses.

use super::{deterministic_diagnostic_id, DraftParseDiffSummary, ANCHOR_MAP_METADATA_KEY};
use crate::domain::script_draft::{
    Diagnostic, DiagnosticSeverity, DraftEpisode, DraftNodeId, DraftNodeOrigin, DraftReviewState,
    DraftScene, DraftShot, DraftStructureV1,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct ReconcileResult {
    pub structure: DraftStructureV1,
    pub diff: DraftParseDiffSummary,
}

pub fn reconcile(
    previous: &DraftStructureV1,
    mut next: DraftStructureV1,
    preserve_human_edits: bool,
) -> ReconcileResult {
    let old_anchors = anchor_map(previous);
    let new_anchors = anchor_map(&next);
    let old_anchor_by_id = reverse_anchor_map(&old_anchors);
    let new_anchor_by_id = reverse_anchor_map(&new_anchors);
    let old_by_anchor = snapshot_map(previous, &old_anchor_by_id);
    let old_ids: BTreeMap<String, DraftNodeId> = old_anchors.clone();
    let mut retained = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut new_map = BTreeMap::new();

    for episode in &mut next.episodes {
        let anchor = anchor_for_id(&new_anchor_by_id, &episode.draft_node_id)
            .unwrap_or_else(|| format!("episode:{}", episode.name.trim().to_ascii_lowercase()));
        reconcile_episode(
            episode,
            &anchor,
            &new_anchor_by_id,
            &old_by_anchor,
            &old_ids,
            preserve_human_edits,
            &mut retained,
            &mut changed,
            &mut new_map,
        );
    }

    let new_keys: BTreeSet<_> = new_map.keys().cloned().collect();
    let old_keys: BTreeSet<_> = old_anchors.keys().cloned().collect();
    let diff = DraftParseDiffSummary {
        retained_nodes: retained.len(),
        added_nodes: new_keys.difference(&old_keys).count(),
        removed_nodes: old_keys.difference(&new_keys).count(),
        changed_nodes: changed.len(),
    };
    if !new_map.is_empty() {
        next.metadata.insert(
            ANCHOR_MAP_METADATA_KEY.to_owned(),
            serde_json::to_string(&new_map).expect("anchor map is serializable"),
        );
    }
    ReconcileResult {
        structure: next,
        diff,
    }
}

pub fn anchor_map(structure: &DraftStructureV1) -> BTreeMap<String, DraftNodeId> {
    structure
        .metadata
        .get(ANCHOR_MAP_METADATA_KEY)
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default()
}

#[derive(Clone)]
enum NodeSnapshot {
    Episode(DraftEpisode),
    Scene(DraftScene),
    Shot(DraftShot),
}

fn snapshot_map(
    structure: &DraftStructureV1,
    anchors: &HashMap<DraftNodeId, String>,
) -> BTreeMap<String, NodeSnapshot> {
    let mut result = BTreeMap::new();
    for episode in &structure.episodes {
        let key = anchor_for_id(anchors, &episode.draft_node_id)
            .unwrap_or_else(|| format!("episode:{}", episode.name.trim().to_ascii_lowercase()));
        result.insert(key.clone(), NodeSnapshot::Episode(episode.clone()));
        for scene in &episode.scenes {
            let key = anchor_for_id(anchors, &scene.draft_node_id).unwrap_or_else(|| {
                format!("{key}/scene:{}", scene.name.trim().to_ascii_lowercase())
            });
            result.insert(key.clone(), NodeSnapshot::Scene(scene.clone()));
            for shot in &scene.shots {
                let key = anchor_for_id(anchors, &shot.draft_node_id).unwrap_or_else(|| {
                    format!("{key}/shot:{}", shot.name.trim().to_ascii_lowercase())
                });
                result.insert(key, NodeSnapshot::Shot(shot.clone()));
            }
        }
    }
    result
}

fn reconcile_episode(
    episode: &mut DraftEpisode,
    anchor: &str,
    new_anchors: &HashMap<DraftNodeId, String>,
    old_by_anchor: &BTreeMap<String, NodeSnapshot>,
    old_ids: &BTreeMap<String, DraftNodeId>,
    preserve_human_edits: bool,
    retained: &mut BTreeSet<String>,
    changed: &mut BTreeSet<String>,
    new_map: &mut BTreeMap<String, DraftNodeId>,
) {
    if let Some(NodeSnapshot::Episode(old)) = old_by_anchor.get(anchor) {
        episode.draft_node_id = old.draft_node_id.clone();
        retained.insert(anchor.to_owned());
        if preserve_human_edits {
            preserve_episode(episode, old, anchor);
        }
        if node_signature(episode) != node_signature(old) {
            changed.insert(anchor.to_owned());
        }
    } else if let Some(old_id) = old_ids.get(anchor) {
        episode.draft_node_id = old_id.clone();
    }
    let episode_id = episode.draft_node_id.clone();
    new_map.insert(anchor.to_owned(), episode_id.clone());

    for scene in &mut episode.scenes {
        let scene_anchor = anchor_for_id(new_anchors, &scene.draft_node_id).unwrap_or_else(|| {
            format!("{anchor}/scene:{}", scene.name.trim().to_ascii_lowercase())
        });
        scene.parent_draft_node_id = Some(episode_id.clone());
        if let Some(NodeSnapshot::Scene(old)) = old_by_anchor.get(&scene_anchor) {
            scene.draft_node_id = old.draft_node_id.clone();
            retained.insert(scene_anchor.clone());
            if preserve_human_edits {
                preserve_scene(scene, old, &scene_anchor);
            }
            if node_signature(scene) != node_signature(old) {
                changed.insert(scene_anchor.clone());
            }
        }
        let scene_id = scene.draft_node_id.clone();
        new_map.insert(scene_anchor.clone(), scene_id.clone());
        for shot in &mut scene.shots {
            let shot_anchor =
                anchor_for_id(new_anchors, &shot.draft_node_id).unwrap_or_else(|| {
                    format!(
                        "{scene_anchor}/shot:{}",
                        shot.name.trim().to_ascii_lowercase()
                    )
                });
            shot.parent_draft_node_id = Some(scene_id.clone());
            shot.parent_scene_draft_id = scene_id.clone();
            if let Some(NodeSnapshot::Shot(old)) = old_by_anchor.get(&shot_anchor) {
                shot.draft_node_id = old.draft_node_id.clone();
                retained.insert(shot_anchor.clone());
                if preserve_human_edits {
                    preserve_shot(shot, old, &shot_anchor);
                }
                if node_signature(shot) != node_signature(old) {
                    changed.insert(shot_anchor.clone());
                }
            }
            new_map.insert(shot_anchor, shot.draft_node_id.clone());
        }
    }
}

fn reverse_anchor_map(anchors: &BTreeMap<String, DraftNodeId>) -> HashMap<DraftNodeId, String> {
    anchors
        .iter()
        .map(|(anchor, node_id)| (node_id.clone(), anchor.clone()))
        .collect()
}

fn anchor_for_id(anchors: &HashMap<DraftNodeId, String>, id: &DraftNodeId) -> Option<String> {
    anchors.get(id).cloned()
}

fn preserve_episode(node: &mut DraftEpisode, old: &DraftEpisode, anchor: &str) {
    if is_human(old.origin, old.review_state) {
        node.original_suggestion = Some(
            node.current_value
                .clone()
                .unwrap_or_else(|| node.name.clone()),
        );
        node.current_value = old.current_value.clone();
        node.review_state = old.review_state;
        node.origin = old.origin;
        add_preserved_diagnostic(&mut node.diagnostics, &node.draft_node_id, anchor);
    }
}

fn preserve_scene(node: &mut DraftScene, old: &DraftScene, anchor: &str) {
    if is_human(old.origin, old.review_state) {
        node.original_suggestion = Some(
            node.current_value
                .clone()
                .unwrap_or_else(|| node.name.clone()),
        );
        node.current_value = old.current_value.clone();
        node.review_state = old.review_state;
        node.origin = old.origin;
        add_preserved_diagnostic(&mut node.diagnostics, &node.draft_node_id, anchor);
    }
}

fn preserve_shot(node: &mut DraftShot, old: &DraftShot, anchor: &str) {
    if is_human(old.origin, old.review_state) {
        node.original_suggestion = Some(
            node.current_value
                .clone()
                .unwrap_or_else(|| node.name.clone()),
        );
        node.current_value = old.current_value.clone();
        node.review_state = old.review_state;
        node.origin = old.origin;
        add_preserved_diagnostic(&mut node.diagnostics, &node.draft_node_id, anchor);
    }
}

fn is_human(origin: DraftNodeOrigin, review_state: DraftReviewState) -> bool {
    origin == DraftNodeOrigin::Human
        || matches!(
            review_state,
            DraftReviewState::Edited | DraftReviewState::Accepted | DraftReviewState::Rejected
        )
}

fn add_preserved_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    node_id: &DraftNodeId,
    anchor: &str,
) {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "REPARSE_HUMAN_EDIT_PRESERVED")
    {
        return;
    }
    let mut diagnostic = Diagnostic::new(
        DiagnosticSeverity::Info,
        "REPARSE_HUMAN_EDIT_PRESERVED",
        "Human edits were preserved while applying the new parser suggestion",
    )
    .for_node(node_id.clone());
    diagnostic.diagnostic_id = deterministic_diagnostic_id(&format!("{anchor}/human-edit"));
    diagnostics.push(diagnostic);
}

fn node_signature<T: serde::Serialize>(node: &T) -> serde_json::Value {
    let mut value = serde_json::to_value(node).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        object.remove("draftNodeId");
        object.remove("parentDraftNodeId");
        object.remove("parentSceneDraftId");
        object.remove("sourceSpans");
        object.remove("diagnostics");
    }
    value
}
