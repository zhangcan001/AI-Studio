use std::{fs, path::PathBuf};

fn app_state_source() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join("src/app_state.rs")).expect("AppState source should be readable")
}

fn block_after<'a>(source: &'a str, marker: &str, open: char, close: char) -> &'a str {
    let marker_start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing source marker: {marker}"));
    let open_index = marker_start + marker.len() - 1;
    let mut depth = 0;
    for (offset, character) in source[open_index..].char_indices() {
        match character {
            value if value == open => depth += 1,
            value if value == close => {
                depth -= 1;
                if depth == 0 {
                    return &source[open_index + 1..open_index + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unclosed source block: {marker}");
}

fn split_top_level(source: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut stack = Vec::new();
    let mut start = 0;
    for (index, character) in source.char_indices() {
        match character {
            '<' | '(' | '[' | '{' => stack.push(character),
            '>' | ')' | ']' | '}' => {
                let expected = match character {
                    '>' => '<',
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                if stack.last() == Some(&expected) {
                    stack.pop();
                }
            }
            ',' if stack.is_empty() => {
                let value = source[start..index].trim();
                if !value.is_empty() {
                    values.push(value);
                }
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        values.push(tail);
    }
    values
}

fn struct_fields<'a>(source: &'a str, name: &str) -> Vec<&'a str> {
    let marker = format!("pub struct {name} {{");
    split_top_level(block_after(source, &marker, '{', '}'))
}

fn app_state_fields(source: &str) -> Vec<(&str, &str)> {
    struct_fields(source, "AppState")
        .into_iter()
        .map(|field| {
            let (name, value_type) = field
                .split_once(':')
                .unwrap_or_else(|| panic!("AppState member is not a field: {field}"));
            (name.trim(), value_type.trim())
        })
        .collect()
}

fn constructor_parameter_count(source: &str) -> usize {
    let marker = "pub fn new(";
    split_top_level(block_after(source, marker, '(', ')')).len()
}

fn bundle_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let declaration = line.trim().strip_prefix("pub struct ")?;
            let name = declaration.split_whitespace().next()?.trim_end_matches('{');
            (name.ends_with("Services") && declaration.ends_with('{')).then(|| name.to_owned())
        })
        .collect()
}

#[test]
fn app_state_keeps_only_a_small_direct_service_surface() {
    let source = app_state_source();
    let direct_services = app_state_fields(&source)
        .iter()
        .filter(|(_, value_type)| {
            value_type.starts_with("Arc<") && value_type.ends_with("Service>")
        })
        .count();

    assert!(
        direct_services <= 12,
        "AppState must expose domain bundles, not a flat list of {direct_services} services"
    );
}

#[test]
fn app_state_constructor_has_a_bounded_explicit_parameter_list() {
    let parameters = constructor_parameter_count(&app_state_source());
    assert!(
        parameters <= 12,
        "AppState::new must take explicit domain bundles instead of {parameters} positional dependencies"
    );
}

#[test]
fn app_state_contains_only_dependency_bundles_and_the_bundles_have_no_behavior() {
    let source = app_state_source();
    let bundles = bundle_names(&source);
    assert!(
        (6..=10).contains(&bundles.len()),
        "AppState should contain a small number of domain bundles; found {}",
        bundles.len()
    );

    let bundle_types = bundles.iter().map(String::as_str).collect::<Vec<_>>();
    for (_, value_type) in app_state_fields(&source) {
        assert!(
            bundle_types.contains(&value_type),
            "AppState top-level dependency must be a domain bundle, got {value_type}"
        );
    }

    for name in bundles {
        for field in struct_fields(&source, &name) {
            let (_, value_type) = field
                .split_once(':')
                .unwrap_or_else(|| panic!("bundle member is not a field: {field}"));
            assert!(
                value_type.trim().starts_with("Arc<") || value_type.trim() == "AppDataDirs",
                "{name} may group dependencies only, got {field}"
            );
        }
        assert!(
            !source.contains(&format!("impl {name}")),
            "{name} must not own domain behavior"
        );
    }

    assert!(
        !source.contains("GenerationService"),
        "GenerationService must remain internal to the Production Queue worker, not AppState"
    );
}
