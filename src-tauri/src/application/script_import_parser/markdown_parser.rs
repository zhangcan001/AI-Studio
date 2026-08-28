//! Deterministic Markdown parser for Draft previews.

use super::{
    check_cancel, diagnostic, make_episode, make_scene, make_shot, new_structure, source_block,
    ParserError, ParserInput, ParserOutput,
};
use crate::domain::script_draft::{
    Diagnostic, DiagnosticSeverity, DraftStructureV1, SourceBlockKind,
};
use std::collections::BTreeMap;

pub fn parse(input: &ParserInput<'_>) -> Result<ParserOutput, ParserError> {
    check_cancel(input)?;
    let lines: Vec<Line<'_>> = input
        .map
        .lines()
        .iter()
        .map(|line| Line {
            start: line.start_byte,
            end: line.end_byte,
            content_end: line.content_end_byte,
            text: line.text(),
        })
        .collect();
    let mut diagnostics = Vec::new();
    if input.map.has_bom() {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Info,
            "SOURCE_ENCODING_OR_BOM",
            "UTF-8 BOM detected and ignored for semantic parsing",
            Some(input.map.span(0, input.map.bom_len())),
        ));
    }
    let Some(first) = lines.iter().find(|line| !line.text.trim().is_empty()) else {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Blocker,
            "SCRIPT_EMPTY",
            "the Markdown document contains no non-whitespace text",
            Some(input.map.span(0, input.raw.len())),
        ));
        return Ok(ParserOutput {
            source_blocks: Vec::new(),
            structure: None,
            diagnostics,
            anchors: BTreeMap::new(),
        });
    };

    let mut blocks = Vec::new();
    let mut anchors = BTreeMap::new();
    let mut anchor_counts = BTreeMap::new();
    let mut structure = new_structure(input.source_id, None);
    let mut episode_index = None;
    let mut scene_index = None;
    let mut previous_heading_level = 0usize;
    let mut has_non_code = false;
    let mut saw_episode_heading = false;
    let mut saw_scene_heading = false;
    let mut i = 0usize;

    while i < lines.len() {
        check_cancel(input)?;
        if lines[i].text.trim().is_empty() {
            i += 1;
            continue;
        }
        if is_fence(lines[i].text) {
            let start = lines[i].start;
            let mut end_index = i + 1;
            while end_index < lines.len() && !is_fence(lines[end_index].text) {
                end_index += 1;
            }
            if end_index < lines.len() {
                end_index += 1;
            }
            let end = lines[end_index.saturating_sub(1)]
                .end
                .max(lines[end_index.saturating_sub(1)].content_end);
            blocks.push(source_block(input, start, end, SourceBlockKind::Code, None));
            diagnostics.push(diagnostic(
                DiagnosticSeverity::Info,
                "MARKDOWN_CODE_IGNORED",
                "fenced code was preserved as source text and ignored as structure",
                Some(input.map.span(start, end)),
            ));
            i = end_index;
            continue;
        }

        if let Some((level, heading)) = heading(lines[i].text) {
            has_non_code = true;
            let end = lines[i].content_end;
            blocks.push(source_block(
                input,
                lines[i].start,
                end,
                SourceBlockKind::Heading,
                None,
            ));
            if previous_heading_level > 0 && level > previous_heading_level + 1 {
                diagnostics.push(diagnostic(
                    DiagnosticSeverity::Warning,
                    "MARKDOWN_HEADING_GAP",
                    "Markdown heading levels skip an intermediate structure level",
                    Some(input.map.span(lines[i].start, end)),
                ));
            }
            previous_heading_level = level;
            let heading_name = heading.to_owned();
            if level == 1 {
                saw_episode_heading = true;
                let anchor = unique_anchor(
                    format!("episode:{}", normalize(heading)),
                    &mut anchor_counts,
                    &mut diagnostics,
                    input.map.span(lines[i].start, end),
                );
                let episode = make_episode(
                    &anchor,
                    heading_name,
                    input.map.span(lines[i].start, end),
                    Vec::new(),
                    Vec::new(),
                );
                if structure.title.is_none() {
                    structure.title = Some(episode.name.clone());
                }
                let id = episode.draft_node_id.clone();
                structure.episodes.push(episode);
                anchors.insert(anchor, id);
                episode_index = Some(structure.episodes.len() - 1);
                scene_index = None;
            } else if level == 2 {
                saw_scene_heading = true;
                ensure_episode(
                    input,
                    &mut structure,
                    &mut anchors,
                    &mut anchor_counts,
                    &mut episode_index,
                    &mut diagnostics,
                    &lines[i],
                );
                let episode = episode_index.expect("episode ensured");
                let parent_id = structure.episodes[episode].draft_node_id.clone();
                let parent_anchor = anchor_for_id(&anchors, &parent_id)
                    .unwrap_or_else(|| "episode:implicit".to_owned());
                let anchor = unique_anchor(
                    format!("{parent_anchor}/scene:{}", normalize(heading)),
                    &mut anchor_counts,
                    &mut diagnostics,
                    input.map.span(lines[i].start, end),
                );
                let scene = make_scene(
                    &anchor,
                    parent_id,
                    heading_name,
                    input.map.span(lines[i].start, end),
                    Vec::new(),
                    None,
                    None,
                    Vec::new(),
                );
                let id = scene.draft_node_id.clone();
                structure.episodes[episode].scenes.push(scene);
                anchors.insert(anchor, id);
                scene_index = Some(structure.episodes[episode].scenes.len() - 1);
            } else {
                ensure_episode(
                    input,
                    &mut structure,
                    &mut anchors,
                    &mut anchor_counts,
                    &mut episode_index,
                    &mut diagnostics,
                    &lines[i],
                );
                ensure_scene(
                    input,
                    &mut structure,
                    &mut anchors,
                    &mut anchor_counts,
                    &mut episode_index,
                    &mut scene_index,
                    &mut diagnostics,
                    &lines[i],
                );
                let episode = episode_index.expect("episode ensured");
                let scene = scene_index.expect("scene ensured");
                let scene_id = structure.episodes[episode].scenes[scene]
                    .draft_node_id
                    .clone();
                let scene_anchor = anchor_for_id(&anchors, &scene_id)
                    .unwrap_or_else(|| "episode:implicit/scene:implicit".to_owned());
                let anchor = unique_anchor(
                    format!("{scene_anchor}/shot:{}", normalize(heading)),
                    &mut anchor_counts,
                    &mut diagnostics,
                    input.map.span(lines[i].start, end),
                );
                let shot = make_shot(
                    &anchor,
                    scene_id,
                    heading_name,
                    input.map.span(lines[i].start, end),
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                );
                let id = shot.draft_node_id.clone();
                structure.episodes[episode].scenes[scene].shots.push(shot);
                anchors.insert(anchor, id);
            }
            i += 1;
            continue;
        }

        let kind = block_kind(lines[i].text);
        let end_index = contiguous_block_end(&lines, i, kind);
        let start = lines[i].start;
        let end = lines[end_index - 1]
            .end
            .max(lines[end_index - 1].content_end);
        blocks.push(source_block(input, start, end, kind, None));
        if kind != SourceBlockKind::Code {
            has_non_code = true;
            ensure_episode(
                input,
                &mut structure,
                &mut anchors,
                &mut anchor_counts,
                &mut episode_index,
                &mut diagnostics,
                &lines[i],
            );
            ensure_scene(
                input,
                &mut structure,
                &mut anchors,
                &mut anchor_counts,
                &mut episode_index,
                &mut scene_index,
                &mut diagnostics,
                &lines[i],
            );
            let episode = episode_index.expect("episode ensured");
            let scene = scene_index.expect("scene ensured");
            let scene_id = structure.episodes[episode].scenes[scene]
                .draft_node_id
                .clone();
            let scene_anchor = anchor_for_id(&anchors, &scene_id)
                .unwrap_or_else(|| "episode:implicit/scene:implicit".to_owned());
            let text = input.raw[start..end]
                .iter()
                .copied()
                .filter(|byte| *byte != b'\r')
                .collect::<Vec<_>>();
            let text = String::from_utf8(text).map_err(|_| ParserError::InvalidUtf8)?;
            let display = text.trim().to_owned();
            let anchor = unique_anchor(
                format!("{scene_anchor}/shot:{}", normalize(&display)),
                &mut anchor_counts,
                &mut diagnostics,
                input.map.span(start, end),
            );
            let mut shot = make_shot(
                &anchor,
                scene_id,
                display.clone(),
                input.map.span(start, end),
                Some(display),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            let id = shot.draft_node_id.clone();
            for mention in &mut shot.characters {
                mention.draft_node_id = Some(id.clone());
            }
            structure.episodes[episode].scenes[scene].shots.push(shot);
            anchors.insert(anchor, id);
        }
        i = end_index;
    }

    if !has_non_code {
        structure.diagnostics = diagnostics.clone();
        return Ok(ParserOutput {
            source_blocks: blocks,
            structure: Some(structure),
            diagnostics,
            anchors,
        });
    }
    for (episode_ordinal, episode) in structure.episodes.iter_mut().enumerate() {
        episode.ordinal = episode_ordinal as u32;
        for (scene_ordinal, scene) in episode.scenes.iter_mut().enumerate() {
            scene.ordinal = scene_ordinal as u32;
            for (shot_ordinal, shot) in scene.shots.iter_mut().enumerate() {
                shot.ordinal = shot_ordinal as u32;
            }
        }
    }
    if !saw_scene_heading {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "UNCERTAIN_SCENE_BOUNDARY",
            "no Markdown scene heading was found; an implicit Draft scene was created",
            Some(input.map.span(first.start, first.content_end)),
        ));
    }
    if !saw_episode_heading {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "IMPLICIT_EPISODE_STRUCTURE",
            "no Markdown episode heading was found; an implicit Draft episode was created",
            Some(input.map.span(first.start, first.content_end)),
        ));
    }
    structure.diagnostics = diagnostics.clone();
    Ok(ParserOutput {
        source_blocks: blocks,
        structure: Some(structure),
        diagnostics,
        anchors,
    })
}

struct Line<'a> {
    start: usize,
    end: usize,
    content_end: usize,
    text: &'a str,
}

fn heading(text: &str) -> Option<(usize, &str)> {
    let trimmed = text.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0
        || level > 6
        || (trimmed
            .as_bytes()
            .get(level)
            .is_some_and(|byte| !byte.is_ascii_whitespace()))
    {
        return None;
    }
    Some((level, trimmed[level..].trim()))
}

fn is_fence(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn block_kind(text: &str) -> SourceBlockKind {
    let trimmed = text.trim_start();
    if trimmed.starts_with('>') {
        SourceBlockKind::Quote
    } else if is_list_item(trimmed) {
        SourceBlockKind::List
    } else if trimmed.contains('|') {
        SourceBlockKind::Table
    } else {
        SourceBlockKind::Paragraph
    }
}

fn is_list_item(text: &str) -> bool {
    text.starts_with("- ")
        || text.starts_with("* ")
        || text.starts_with("+ ")
        || text.find('.').is_some_and(|position| {
            position > 0
                && text[..position].chars().all(|c| c.is_ascii_digit())
                && text[position + 1..].starts_with(' ')
        })
}

fn contiguous_block_end(lines: &[Line<'_>], start: usize, kind: SourceBlockKind) -> usize {
    let mut end = start + 1;
    while end < lines.len() && !lines[end].text.trim().is_empty() {
        if heading(lines[end].text).is_some()
            || is_fence(lines[end].text)
            || block_kind(lines[end].text) != kind
        {
            break;
        }
        end += 1;
    }
    end
}

fn ensure_episode(
    input: &ParserInput<'_>,
    structure: &mut DraftStructureV1,
    anchors: &mut BTreeMap<String, crate::domain::script_draft::DraftNodeId>,
    counts: &mut BTreeMap<String, usize>,
    index: &mut Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
    line: &Line<'_>,
) {
    if index.is_some() {
        return;
    }
    let span = input.map.span(line.start, line.content_end);
    let anchor = unique_anchor(
        "episode:implicit".to_owned(),
        counts,
        diagnostics,
        span.clone(),
    );
    let episode = make_episode(&anchor, "未分集".to_owned(), span, Vec::new(), Vec::new());
    let id = episode.draft_node_id.clone();
    structure.episodes.push(episode);
    anchors.insert(anchor, id);
    *index = Some(structure.episodes.len() - 1);
}

fn ensure_scene(
    input: &ParserInput<'_>,
    structure: &mut DraftStructureV1,
    anchors: &mut BTreeMap<String, crate::domain::script_draft::DraftNodeId>,
    counts: &mut BTreeMap<String, usize>,
    episode_index: &mut Option<usize>,
    scene_index: &mut Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
    line: &Line<'_>,
) {
    if scene_index.is_some() {
        return;
    }
    let episode = episode_index.expect("episode is ensured first");
    let span = input.map.span(line.start, line.content_end);
    let parent = structure.episodes[episode].draft_node_id.clone();
    let parent_anchor =
        anchor_for_id(anchors, &parent).unwrap_or_else(|| "episode:implicit".to_owned());
    let anchor = unique_anchor(
        format!("{parent_anchor}/scene:implicit"),
        counts,
        diagnostics,
        span.clone(),
    );
    let mut scene = make_scene(
        &anchor,
        parent,
        "未分场景".to_owned(),
        span,
        vec![diagnostic(
            DiagnosticSeverity::Warning,
            "UNCERTAIN_SCENE_BOUNDARY",
            "scene boundary is implicit and should be reviewed",
            None,
        )],
        None,
        None,
        Vec::new(),
    );
    let id = scene.draft_node_id.clone();
    for diagnostic in &mut scene.diagnostics {
        if diagnostic.draft_node_id.is_none() {
            diagnostic.draft_node_id = Some(id.clone());
        }
    }
    structure.episodes[episode].scenes.push(scene);
    anchors.insert(anchor, id);
    *scene_index = Some(structure.episodes[episode].scenes.len() - 1);
}

fn unique_anchor(
    base: String,
    counts: &mut BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    span: crate::domain::script_draft::SourceSpan,
) -> String {
    let occurrence = counts.entry(base.clone()).or_insert(0);
    let value = if *occurrence == 0 {
        base.clone()
    } else {
        format!("{base}#{}", *occurrence)
    };
    if *occurrence > 0 {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "AMBIGUOUS_PARSE_ANCHOR",
            "duplicate Markdown anchor was disambiguated by occurrence order",
            Some(span),
        ));
    }
    *occurrence += 1;
    value
}

fn anchor_for_id(
    anchors: &BTreeMap<String, crate::domain::script_draft::DraftNodeId>,
    id: &crate::domain::script_draft::DraftNodeId,
) -> Option<String> {
    anchors
        .iter()
        .find_map(|(anchor, candidate)| (candidate == id).then(|| anchor.clone()))
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::super::{parse_source, ScriptParseMode, ScriptParseOptions};
    use crate::domain::script_draft::{ScriptFormat, SourceBlockKind, SourceId};

    fn parse(text: &str) -> super::super::ParserOutput {
        let source_id = SourceId::new();
        parse_source(
            &source_id,
            "project-1",
            ScriptFormat::Markdown,
            Some("story.md"),
            text.as_bytes(),
            &ScriptParseOptions {
                mode: ScriptParseMode::Auto,
                preserve_human_edits: true,
            },
            None,
        )
        .unwrap()
    }

    #[test]
    fn parses_headings_lists_quotes_tables_and_code_without_executing_code() {
        let output = parse("# Episode\n## Scene\n### Shot\n- one\n  - nested\n> quote\n| a | b |\n|---|---|\n```json\n{\"workflowVersionId\":\"no\"}\n```");
        let structure = output.structure.unwrap();
        assert_eq!(structure.episodes.len(), 1);
        assert_eq!(structure.episodes[0].scenes.len(), 1);
        assert!(structure.episodes[0].scenes[0].shots.len() >= 4);
        assert!(output
            .source_blocks
            .iter()
            .any(|block| block.kind == SourceBlockKind::Code));
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MARKDOWN_CODE_IGNORED"));
        assert!(output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("workflowVersionId")));
    }

    #[test]
    fn heading_gap_is_visible_and_links_are_plain_text() {
        let output = parse("# E\n### S\n[read](https://example.invalid/a)");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MARKDOWN_HEADING_GAP"));
        assert!(output.source_blocks.iter().any(|block| block
            .preview
            .as_deref()
            .is_some_and(|text| text.contains("read"))));
    }

    #[test]
    fn empty_markdown_is_blocked() {
        let output = parse("\r\n  \n");
        assert!(output.structure.is_none());
        assert_eq!(output.diagnostics[0].code, "SCRIPT_EMPTY");
    }

    #[test]
    fn paragraph_without_h2_has_visible_implicit_scene_warning() {
        let output = parse("# Episode\n\nA paragraph");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UNCERTAIN_SCENE_BOUNDARY"));
    }
}
