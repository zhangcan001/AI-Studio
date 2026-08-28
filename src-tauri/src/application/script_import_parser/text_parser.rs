//! Deterministic TXT, screenplay, and conservative novel parsing.

use super::{
    check_cancel, diagnostic, make_episode, make_mention, make_scene, make_shot, new_structure,
    source_block, ParserError, ParserInput, ParserOutput, ScriptParseMode,
};
use crate::domain::script_draft::{
    Diagnostic, DiagnosticSeverity, DraftDialogue, DraftStructureV1, SourceBlockKind, SourceSpan,
};
use std::collections::BTreeMap;

pub fn parse(input: &ParserInput<'_>) -> Result<ParserOutput, ParserError> {
    check_cancel(input)?;
    let records: Vec<LineRecord<'_>> = input
        .map
        .lines()
        .iter()
        .map(|line| LineRecord {
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
    let first_content = records.iter().find(|line| !line.is_blank());
    if let Some(line) = first_content {
        if input.map.text().trim().is_empty() {
            diagnostics.push(diagnostic(
                DiagnosticSeverity::Blocker,
                "SCRIPT_EMPTY",
                "the script contains no non-whitespace text",
                Some(input.map.span(line.start, line.content_end)),
            ));
            return Ok(ParserOutput {
                source_blocks: Vec::new(),
                structure: None,
                diagnostics,
                anchors: BTreeMap::new(),
            });
        }
    } else {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Blocker,
            "SCRIPT_EMPTY",
            "the script contains no non-whitespace text",
            Some(input.map.span(0, input.raw.len())),
        ));
        return Ok(ParserOutput {
            source_blocks: Vec::new(),
            structure: None,
            diagnostics,
            anchors: BTreeMap::new(),
        });
    }

    let novel = matches!(input.options.mode, ScriptParseMode::Novel)
        || (matches!(input.options.mode, ScriptParseMode::Auto)
            && records.iter().any(|line| is_chapter_heading(line.text)));
    let mut structure = new_structure(input.source_id, None);
    let mut blocks = Vec::new();
    let mut anchors = BTreeMap::new();
    let mut anchor_counts = BTreeMap::<String, usize>::new();
    let mut episode_index: Option<usize> = None;
    let mut scene_index: Option<usize> = None;
    let mut saw_explicit_episode = false;
    let mut saw_explicit_scene = false;
    let mut novel_warning_added = false;
    let mut i = 0;

    while i < records.len() {
        check_cancel(input)?;
        let line = &records[i];
        if line.is_blank() {
            i += 1;
            continue;
        }

        if is_episode_heading(line.text, novel) {
            let name = clean_heading(line.text);
            let anchor = unique_anchor(
                format!("episode:{}", normalize_anchor(&name)),
                &mut anchor_counts,
                &mut diagnostics,
                input.map.span(line.start, line.content_end),
            );
            let episode = make_episode(
                &anchor,
                name,
                input.map.span(line.start, line.content_end),
                Vec::new(),
                Vec::new(),
            );
            episode_index = Some(structure.episodes.len());
            scene_index = None;
            structure.episodes.push(episode);
            anchors.insert(
                anchor,
                structure.episodes.last().unwrap().draft_node_id.clone(),
            );
            saw_explicit_episode = true;
            blocks.push(source_block(
                input,
                line.start,
                line.content_end,
                SourceBlockKind::Heading,
                None,
            ));
            i += 1;
            continue;
        }

        if is_scene_heading(line.text, input.options.mode, novel) {
            ensure_episode(
                input,
                &mut structure,
                &mut anchors,
                &mut anchor_counts,
                &mut episode_index,
                &mut diagnostics,
                line,
            );
            let name = clean_heading(line.text);
            let parent_id = structure.episodes[episode_index.unwrap()]
                .draft_node_id
                .clone();
            let episode_anchor = anchor_for_id(&anchors, &parent_id)
                .unwrap_or_else(|| "episode:implicit".to_owned());
            let anchor = unique_anchor(
                format!("{episode_anchor}/scene:{}", normalize_anchor(&name)),
                &mut anchor_counts,
                &mut diagnostics,
                input.map.span(line.start, line.content_end),
            );
            let boundary_diagnostic = novel
                .then(|| is_novel_scene_boundary(line.text))
                .filter(|is_boundary| *is_boundary)
                .map(|_| {
                    diagnostic(
                        DiagnosticSeverity::Warning,
                        "UNCERTAIN_SCENE_BOUNDARY",
                        "novel time or location change is only a Draft scene suggestion",
                        Some(input.map.span(line.start, line.content_end)),
                    )
                });
            let mut scene = make_scene(
                &anchor,
                parent_id,
                name.clone(),
                input.map.span(line.start, line.content_end),
                boundary_diagnostic.iter().cloned().collect(),
                location_suggestion(&name),
                time_suggestion(&name),
                Vec::new(),
            );
            let scene_id = scene.draft_node_id.clone();
            for diagnostic in &mut scene.diagnostics {
                if diagnostic.draft_node_id.is_none() {
                    diagnostic.draft_node_id = Some(scene_id.clone());
                }
            }
            if boundary_diagnostic.is_some() {
                diagnostics.extend(scene.diagnostics.iter().cloned());
            }
            structure.episodes[episode_index.unwrap()]
                .scenes
                .push(scene);
            scene_index = Some(structure.episodes[episode_index.unwrap()].scenes.len() - 1);
            anchors.insert(anchor, scene_id);
            saw_explicit_scene = true;
            blocks.push(source_block(
                input,
                line.start,
                line.content_end,
                SourceBlockKind::Heading,
                None,
            ));
            i += 1;
            continue;
        }

        ensure_episode(
            input,
            &mut structure,
            &mut anchors,
            &mut anchor_counts,
            &mut episode_index,
            &mut diagnostics,
            line,
        );
        ensure_scene(
            input,
            &mut structure,
            &mut anchors,
            &mut anchor_counts,
            &mut episode_index,
            &mut scene_index,
            &mut diagnostics,
            line,
        );

        let (end_index, block_kind, text, dialogue, mut shot_diags) =
            classify_block(input, &records, i, novel)?;
        let end = records[end_index - 1]
            .end
            .max(records[end_index - 1].content_end);
        let start = line.start;
        let scene_id = structure.episodes[episode_index.unwrap()].scenes[scene_index.unwrap()]
            .draft_node_id
            .clone();
        let scene_anchor =
            anchor_for_id(&anchors, &scene_id).unwrap_or_else(|| "scene:implicit".to_owned());
        let display = text.trim().to_owned();
        let shot_anchor = unique_anchor(
            format!("{scene_anchor}/shot:{}", normalize_anchor(&display)),
            &mut anchor_counts,
            &mut diagnostics,
            input.map.span(start, end),
        );
        let mut characters = Vec::new();
        let (description, action) = if let Some(ref dialogue) = dialogue {
            if let Some(speaker) = dialogue.first().and_then(|value| value.speaker.clone()) {
                characters.push(make_mention(
                    &format!("{shot_anchor}/character:{speaker}"),
                    speaker,
                    dialogue
                        .first()
                        .and_then(|value| value.source_spans.first())
                        .cloned()
                        .unwrap_or_else(|| input.map.span(start, end)),
                ));
            }
            (None, None)
        } else if novel {
            if contains_thought_cue(&display) {
                shot_diags.push(diagnostic(
                    DiagnosticSeverity::Info,
                    "NOVEL_THOUGHT_CUE",
                    "novel thought cue kept as narration, not an action",
                    Some(input.map.span(start, end)),
                ));
            }
            if !novel_warning_added {
                diagnostics.push(diagnostic(
                    DiagnosticSeverity::Warning,
                    "NOVEL_HEURISTIC_DRAFT",
                    "novel text was converted conservatively into Draft candidates",
                    Some(input.map.span(start, end)),
                ));
                novel_warning_added = true;
            }
            (Some(display.clone()), None)
        } else if matches!(block_kind, SourceBlockKind::Dialogue) {
            (None, None)
        } else {
            (
                Some(display.clone()),
                matches!(input.options.mode, ScriptParseMode::Screenplay)
                    .then_some(display.clone()),
            )
        };
        let mut shot = make_shot(
            &shot_anchor,
            scene_id.clone(),
            if matches!(block_kind, SourceBlockKind::Dialogue) {
                "对白".to_owned()
            } else {
                display.clone()
            },
            input.map.span(start, end),
            description,
            action,
            dialogue.unwrap_or_default(),
            characters,
            std::mem::take(&mut shot_diags),
        );
        let shot_id = shot.draft_node_id.clone();
        for diagnostic in &mut shot.diagnostics {
            if diagnostic.draft_node_id.is_none() {
                diagnostic.draft_node_id = Some(shot_id.clone());
            }
        }
        for mention in &mut shot.characters {
            mention.draft_node_id = Some(shot_id.clone());
        }
        diagnostics.extend(shot.diagnostics.iter().cloned());
        let scene = &mut structure.episodes[episode_index.unwrap()].scenes[scene_index.unwrap()];
        scene.shots.push(shot);
        anchors.insert(shot_anchor, shot_id);
        blocks.push(source_block(
            input,
            start,
            end,
            block_kind,
            Some(scene_anchor),
        ));
        i = end_index;
    }

    if !saw_explicit_scene
        && structure
            .episodes
            .iter()
            .all(|episode| episode.scenes.is_empty())
    {
        if let Some(line) = first_content {
            ensure_scene(
                input,
                &mut structure,
                &mut anchors,
                &mut anchor_counts,
                &mut episode_index,
                &mut scene_index,
                &mut diagnostics,
                line,
            );
        }
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
    if !saw_explicit_episode {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "IMPLICIT_EPISODE_STRUCTURE",
            "no episode heading was found; an implicit Draft episode was created",
            first_content.map(|line| input.map.span(line.start, line.content_end)),
        ));
    }
    if !saw_explicit_scene {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "UNCERTAIN_SCENE_BOUNDARY",
            "no strong scene heading was found; an implicit Draft scene was created",
            first_content.map(|line| input.map.span(line.start, line.content_end)),
        ));
    }
    Ok(ParserOutput {
        source_blocks: blocks,
        structure: Some(structure),
        diagnostics,
        anchors,
    })
}

#[derive(Clone, Copy)]
struct LineRecord<'a> {
    start: usize,
    end: usize,
    content_end: usize,
    text: &'a str,
}

impl LineRecord<'_> {
    fn is_blank(self) -> bool {
        self.text.trim().is_empty()
    }
}

fn classify_block(
    input: &ParserInput<'_>,
    records: &[LineRecord<'_>],
    start: usize,
    novel: bool,
) -> Result<
    (
        usize,
        SourceBlockKind,
        String,
        Option<Vec<DraftDialogue>>,
        Vec<Diagnostic>,
    ),
    ParserError,
> {
    let line = records[start];
    if let Some((speaker, text_start)) = colon_dialogue(line.text) {
        let speaker = if novel {
            novel_speaker_name(&speaker)
        } else {
            speaker
        };
        let text = line.text[text_start..].trim().to_owned();
        let speaker_start = line.start + line.text.find(&speaker).unwrap_or(0);
        let span = input.map.span(speaker_start, line.content_end);
        return Ok((
            start + 1,
            SourceBlockKind::Dialogue,
            line.text.to_owned(),
            Some(vec![DraftDialogue {
                speaker: Some(speaker),
                text,
                source_spans: vec![span],
            }]),
            Vec::new(),
        ));
    }
    if novel && has_quoted_dialogue(line.text) {
        let (text, speaker) = quoted_dialogue(line.text);
        let span = input.map.span(line.start, line.content_end);
        let mut diags = Vec::new();
        if speaker.is_none() {
            diags.push(diagnostic(
                DiagnosticSeverity::Warning,
                "UNRESOLVED_DIALOGUE_SPEAKER",
                "quoted dialogue speaker could not be determined confidently",
                Some(span.clone()),
            ));
        }
        return Ok((
            start + 1,
            SourceBlockKind::Dialogue,
            line.text.to_owned(),
            Some(vec![DraftDialogue {
                speaker,
                text,
                source_spans: vec![span],
            }]),
            diags,
        ));
    }
    if !novel
        && start + 1 < records.len()
        && !records[start + 1].is_blank()
        && is_screenplay_speaker(line.text)
        && !is_heading(line.text, input.options.mode, novel)
    {
        let next = records[start + 1];
        if colon_dialogue(next.text).is_none() && !is_heading(next.text, input.options.mode, novel)
        {
            let span = input.map.span(line.start, next.content_end);
            return Ok((
                start + 2,
                SourceBlockKind::Dialogue,
                format!("{}\n{}", line.text, next.text),
                Some(vec![DraftDialogue {
                    speaker: Some(line.text.trim().to_owned()),
                    text: next.text.trim().to_owned(),
                    source_spans: vec![span],
                }]),
                Vec::new(),
            ));
        }
    }
    let mut end_index = start + 1;
    while end_index < records.len()
        && !records[end_index].is_blank()
        && !is_heading(records[end_index].text, input.options.mode, novel)
        && colon_dialogue(records[end_index].text).is_none()
    {
        end_index += 1;
    }
    let end = records[end_index - 1].content_end;
    let raw = &input.raw[line.start..end];
    let text = std::str::from_utf8(raw)
        .map_err(|_| ParserError::InvalidUtf8)?
        .to_owned();
    let kind = if novel {
        SourceBlockKind::Narration
    } else {
        SourceBlockKind::Paragraph
    };
    Ok((end_index, kind, text, None, Vec::new()))
}

fn ensure_episode(
    input: &ParserInput<'_>,
    structure: &mut DraftStructureV1,
    anchors: &mut BTreeMap<String, crate::domain::script_draft::DraftNodeId>,
    counts: &mut BTreeMap<String, usize>,
    index: &mut Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
    line: &LineRecord<'_>,
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
    line: &LineRecord<'_>,
) {
    if scene_index.is_some() {
        return;
    }
    let episode = episode_index.expect("episode ensured");
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
    span: SourceSpan,
) -> String {
    let occurrence = counts.entry(base.clone()).or_insert(0);
    let result = if *occurrence == 0 {
        base.clone()
    } else {
        format!("{base}#{}", *occurrence)
    };
    if *occurrence > 0 {
        diagnostics.push(diagnostic(
            DiagnosticSeverity::Warning,
            "AMBIGUOUS_PARSE_ANCHOR",
            "duplicate parse anchor was disambiguated by occurrence order",
            Some(span),
        ));
    }
    *occurrence += 1;
    result
}

fn anchor_for_id(
    anchors: &BTreeMap<String, crate::domain::script_draft::DraftNodeId>,
    id: &crate::domain::script_draft::DraftNodeId,
) -> Option<String> {
    anchors
        .iter()
        .find_map(|(anchor, candidate)| (candidate == id).then(|| anchor.clone()))
}

fn is_episode_heading(text: &str, novel: bool) -> bool {
    let value = text.trim();
    let upper = value.to_ascii_uppercase();
    ["EP", "EPISODE ", "CHAPTER ", "PART "]
        .iter()
        .any(|prefix| numbered_heading(&upper, prefix))
        || ["第", "第一章", "第一集", "第一回"].iter().any(|prefix| {
            value.starts_with(prefix)
                && (value.contains('章') || value.contains('集') || value.contains('回'))
        })
        || (novel && is_chapter_heading(value))
}

fn numbered_heading(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .and_then(|suffix| suffix.trim().chars().next())
        .is_some_and(|character| character.is_ascii_digit())
}

fn is_chapter_heading(text: &str) -> bool {
    let value = text.trim();
    value.starts_with('第') && (value.contains('章') || value.contains('回')) || {
        let upper = value.to_ascii_uppercase();
        (upper.starts_with("CHAPTER ") || upper.starts_with("PART "))
            && upper.chars().any(|c| c.is_ascii_digit())
    }
}

fn is_scene_heading(text: &str, mode: ScriptParseMode, novel: bool) -> bool {
    let value = text.trim();
    if novel && is_novel_scene_boundary(value) {
        return true;
    }
    is_scene(value, mode, novel)
}

fn is_novel_scene_boundary(text: &str) -> bool {
    text.starts_with("第二天")
        || text.starts_with("次日")
        || text.starts_with("与此同时")
        || text.starts_with("地点：")
        || text.starts_with("地点:")
}

fn is_scene(text: &str, mode: ScriptParseMode, _novel: bool) -> bool {
    let upper = text.to_ascii_uppercase();
    text.starts_with("场景")
        || text.starts_with("内景")
        || text.starts_with("外景")
        || upper.starts_with("SCENE ")
        || upper.starts_with("INT.")
        || upper.starts_with("EXT.")
        || upper.starts_with("INT/")
        || upper.starts_with("I/E.")
        || (matches!(mode, ScriptParseMode::Screenplay) && (upper == "INT" || upper == "EXT"))
        || (text.contains(" - ") && text.len() <= 100)
}

fn is_heading(text: &str, mode: ScriptParseMode, novel: bool) -> bool {
    is_episode_heading(text, novel) || is_scene_heading(text, mode, novel)
}

fn clean_heading(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| c == '#' || c == ':' || c == '：')
        .trim()
        .to_owned()
}

fn normalize_anchor(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn colon_dialogue(text: &str) -> Option<(String, usize)> {
    let position = text.find(['：', ':'])?;
    let speaker = text[..position].trim();
    let dialogue = text[position + text[position..].chars().next().unwrap().len_utf8()..].trim();
    if speaker.is_empty() || speaker.len() > 120 || dialogue.is_empty() {
        return None;
    }
    Some((
        speaker.to_owned(),
        position + text[position..].chars().next().unwrap().len_utf8(),
    ))
}

fn is_screenplay_speaker(text: &str) -> bool {
    let value = text.trim();
    !value.is_empty()
        && value.len() <= 60
        && !value.chars().any(|c| ".!?。！？,，；;:：".contains(c))
        && value.chars().all(|c| {
            c.is_alphanumeric()
                || c == '_'
                || ('\u{4e00}'..='\u{9fff}').contains(&c)
                || c.is_whitespace()
        })
}

fn has_quoted_dialogue(text: &str) -> bool {
    (text.contains('“') && text.contains('”')) || (text.matches('"').count() >= 2)
}

fn quoted_dialogue(text: &str) -> (String, Option<String>) {
    let (left, right) = if let (Some(start), Some(end)) = (text.find('“'), text.find('”')) {
        (start, end)
    } else if let (Some(start), Some(end)) = (text.find('"'), text.rfind('"')) {
        (start, end)
    } else {
        return (text.to_owned(), None);
    };
    let spoken = text[left + 1..right].to_owned();
    let prefix = text[..left]
        .trim()
        .trim_end_matches(|c: char| c == '：' || c == ':');
    let speaker = prefix
        .trim_end_matches(|c: char| c == '，' || c == ',' || c == ' ')
        .strip_suffix('说')
        .or_else(|| prefix.strip_suffix('问'))
        .or_else(|| prefix.strip_suffix('道'))
        .or_else(|| prefix.strip_suffix('喊'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    (spoken, speaker)
}

fn novel_speaker_name(value: &str) -> String {
    value
        .strip_suffix('说')
        .or_else(|| value.strip_suffix('问'))
        .or_else(|| value.strip_suffix('道'))
        .or_else(|| value.strip_suffix('喊'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(value)
        .to_owned()
}

fn contains_thought_cue(text: &str) -> bool {
    ["心想", "想着", "觉得", "意识到", "内心", "暗想"]
        .iter()
        .any(|cue| text.contains(cue))
}

fn location_suggestion(name: &str) -> Option<String> {
    name.split('-')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn time_suggestion(name: &str) -> Option<String> {
    name.split('-')
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::script_import_parser::{parse_source, ScriptParseOptions};
    use crate::domain::script_draft::{ScriptFormat, SourceId};

    fn parse(source: &str, mode: ScriptParseMode) -> ParserOutput {
        let source_id = SourceId::new();
        parse_source(
            &source_id,
            "project",
            ScriptFormat::Txt,
            Some("story.txt"),
            source.as_bytes(),
            &ScriptParseOptions {
                mode,
                ..ScriptParseOptions::default()
            },
            None,
        )
        .expect("TXT parser should return a result")
    }

    #[test]
    fn parses_chinese_screenplay_lines_and_explicit_character_mentions() {
        let output = parse(
            "第一集\r\n\r\n场景 1 - 夜\r\n\r\n张三：你好",
            ScriptParseMode::Auto,
        );
        let structure = output.structure.expect("structure");
        let scene = &structure.episodes[0].scenes[0];
        let shot = &scene.shots[0];
        assert_eq!(structure.episodes[0].ordinal, 0);
        assert_eq!(scene.ordinal, 0);
        assert_eq!(shot.ordinal, 0);
        assert_eq!(shot.dialogue[0].speaker.as_deref(), Some("张三"));
        assert_eq!(shot.characters[0].raw_text, "张三");
        assert_eq!(scene.location_suggestion.as_deref(), Some("场景 1"));
        assert_eq!(scene.time_suggestion.as_deref(), Some("夜"));
        assert!(shot.source_spans[0].end_byte > shot.source_spans[0].start_byte);
    }

    #[test]
    fn missing_headings_create_visible_implicit_draft_structure() {
        let output = parse("一段普通叙述。\n\n下一段。", ScriptParseMode::Auto);
        let structure = output.structure.expect("structure");
        assert_eq!(structure.episodes[0].name, "未分集");
        assert_eq!(structure.episodes[0].scenes[0].name, "未分场景");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "IMPLICIT_EPISODE_STRUCTURE"));
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UNCERTAIN_SCENE_BOUNDARY"));
    }

    #[test]
    fn novel_keeps_thought_as_narration_and_marks_uncertain_time_boundary() {
        let output = parse(
            "第一章\n\n他心想，这里不安全。\n\n第二天\n\n\"我们走。\"\n\n张三说：“我们走。”",
            ScriptParseMode::Novel,
        );
        let structure = output.structure.expect("structure");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NOVEL_HEURISTIC_DRAFT"));
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UNCERTAIN_SCENE_BOUNDARY"));
        let thought = &structure.episodes[0].scenes[0].shots[0];
        assert!(thought.description.is_some());
        assert!(thought.action.is_none());
        assert!(thought
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NOVEL_THOUGHT_CUE"));
        assert!(structure.episodes[0]
            .scenes
            .iter()
            .flat_map(|scene| scene.shots.iter())
            .any(|shot| shot
                .dialogue
                .iter()
                .any(|dialogue| dialogue.speaker.is_none())));
        assert!(structure.episodes[0]
            .scenes
            .iter()
            .flat_map(|scene| scene.shots.iter())
            .any(|shot| shot
                .dialogue
                .iter()
                .any(|dialogue| dialogue.speaker.as_deref() == Some("张三"))));
    }

    #[test]
    fn bom_and_long_line_keep_raw_spans_and_bounded_previews() {
        let long_line = "长".repeat(1_048_576);
        let source = format!("\u{feff}{long_line}");
        let output = parse(&source, ScriptParseMode::Auto);
        let block = &output.source_blocks[0];
        assert_eq!(block.span.start_byte, 3);
        assert!(block.preview.as_ref().unwrap().chars().count() <= 160);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SOURCE_ENCODING_OR_BOM"));
    }

    #[test]
    fn screenplay_mode_supports_name_then_dialogue_without_colon() {
        let output = parse(
            "EP01\n\nINT. ROOM - DAY\n\nJOHN\nHello.",
            ScriptParseMode::Screenplay,
        );
        let shot = &output.structure.unwrap().episodes[0].scenes[0].shots[0];
        assert_eq!(shot.dialogue[0].speaker.as_deref(), Some("JOHN"));
        assert_eq!(shot.dialogue[0].text, "Hello.");
    }
}
