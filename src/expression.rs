use std::collections::BTreeSet;

mod compat;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct IdentifierUsage {
    pub values: BTreeSet<String>,
    pub calls: BTreeSet<String>,
}

pub(crate) fn normalize(source: &str) -> String {
    let rewritten = rewrite(source);
    if compat::contains_unsupported_syntax(&rewritten) {
        format!("Vue2Qml.Runtime.sourceExpression({})", qml_string(source))
    } else {
        rewritten
    }
}

fn rewrite(source: &str) -> String {
    let decoded = decode_entities(source);
    rewrite_identifiers(&decoded).replace("?.", ".")
}

pub(crate) fn text_expression(source: &str) -> String {
    let mut parts = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("{{") {
        let open = cursor + relative;
        if open > cursor {
            let text = collapse_template_whitespace(&source[cursor..open]);
            if !text.is_empty() {
                parts.push(qml_string(&decode_entities(&text)));
            }
        }
        let expression_start = open + 2;
        let Some(relative_end) = source[expression_start..].find("}}") else {
            let tail = collapse_template_whitespace(&source[open..]);
            parts.push(qml_string(&decode_entities(&tail)));
            cursor = source.len();
            break;
        };
        let end = expression_start + relative_end;
        let expression = normalize(source[expression_start..end].trim());
        parts.push(format!("Vue2Qml.Runtime.display({expression})"));
        cursor = end + 2;
    }
    if cursor < source.len() {
        let text = collapse_template_whitespace(&source[cursor..]);
        if !text.is_empty() {
            parts.push(qml_string(&decode_entities(&text)));
        }
    }
    if parts.is_empty() {
        qml_string("")
    } else {
        parts.join(" + ")
    }
}

pub(crate) fn identifiers(source: &str) -> IdentifierUsage {
    let source = rewrite(source);
    let mut usage = IdentifierUsage::default();
    scan_identifiers(&source, &mut usage);
    usage
}

pub(crate) fn qml_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\u{2028}' => output.push_str("\\u2028"),
            '\u{2029}' => output.push_str("\\u2029"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

pub(crate) fn qml_property_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut uppercase_next = false;
    for character in name.chars() {
        if character == '-' || character == ':' {
            uppercase_next = true;
        } else if character == '$' {
            output.push_str("vue_");
        } else if character.is_ascii_alphanumeric() || character == '_' {
            if uppercase_next {
                output.extend(character.to_uppercase());
                uppercase_next = false;
            } else {
                output.push(character);
            }
        } else {
            output.push('_');
        }
    }
    if output.is_empty() || !output.as_bytes()[0].is_ascii_lowercase() {
        output.insert_str(0, "vue_");
    }
    if is_qml_keyword(&output) {
        output.insert_str(0, "vue_");
    }
    output
}

fn rewrite_identifiers(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            copy_string(source, &mut cursor, &mut output, byte);
        } else if is_identifier_start(byte) {
            let start = cursor;
            cursor += 1;
            while source
                .as_bytes()
                .get(cursor)
                .copied()
                .is_some_and(is_identifier_part)
            {
                cursor += 1;
            }
            let token = &source[start..cursor];
            match token {
                "$event" => output.push_str("event"),
                "undefined" => output.push_str("null"),
                token if token.starts_with('$') => output.push_str(&qml_property_name(token)),
                _ => output.push_str(token),
            }
        } else {
            let length = source[cursor..].chars().next().map_or(1, char::len_utf8);
            output.push_str(&source[cursor..cursor + length]);
            cursor += length;
        }
    }
    output
}

fn copy_string(source: &str, cursor: &mut usize, output: &mut String, quote: u8) {
    let start = *cursor;
    skip_string(source, cursor, quote);
    output.push_str(&source[start..*cursor]);
}

fn scan_identifiers(source: &str, usage: &mut IdentifierUsage) {
    let mut cursor = 0;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if byte == b'`' {
            scan_template_literal(source, &mut cursor, usage);
            continue;
        }
        if source[cursor..].starts_with("//") || source[cursor..].starts_with("/*") {
            skip_comment(source, &mut cursor);
            continue;
        }
        if !is_identifier_start(byte) {
            cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let start = cursor;
        cursor += 1;
        while source
            .as_bytes()
            .get(cursor)
            .copied()
            .is_some_and(is_identifier_part)
        {
            cursor += 1;
        }
        let name = &source[start..cursor];
        let previous = previous_non_space(source, start);
        let next = next_non_space(source, cursor);
        if previous == Some(b'.') || is_ignored_identifier(name) || is_object_key(previous, next) {
            continue;
        }
        usage.values.insert(name.to_owned());
        if next == Some(b'(') {
            usage.calls.insert(name.to_owned());
        }
    }
}

fn scan_template_literal(source: &str, cursor: &mut usize, usage: &mut IdentifierUsage) {
    *cursor += 1;
    while *cursor < source.len() {
        if source.as_bytes()[*cursor] == b'\\' {
            *cursor = (*cursor + 2).min(source.len());
        } else if source.as_bytes().get(*cursor..*cursor + 2) == Some(b"${") {
            let start = *cursor + 2;
            if let Some(end) = interpolation_end(source, start) {
                scan_identifiers(&source[start..end], usage);
                *cursor = end + 1;
            } else {
                *cursor = source.len();
            }
        } else {
            let byte = source.as_bytes()[*cursor];
            *cursor += source[*cursor..].chars().next().map_or(1, char::len_utf8);
            if byte == b'`' {
                break;
            }
        }
    }
}

fn interpolation_end(source: &str, mut cursor: usize) -> Option<usize> {
    let mut depth = 1_u32;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

fn is_object_key(previous: Option<u8>, next: Option<u8>) -> bool {
    next == Some(b':') && matches!(previous, None | Some(b'{') | Some(b','))
}

fn previous_non_space(source: &str, cursor: usize) -> Option<u8> {
    source.as_bytes()[..cursor]
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn next_non_space(source: &str, cursor: usize) -> Option<u8> {
    source.as_bytes()[cursor..]
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn skip_string(source: &str, cursor: &mut usize, quote: u8) {
    *cursor += 1;
    while let Some(byte) = source.as_bytes().get(*cursor).copied() {
        *cursor += 1;
        if byte == b'\\' {
            *cursor = (*cursor + 1).min(source.len());
        } else if byte == quote {
            break;
        }
    }
}

fn skip_comment(source: &str, cursor: &mut usize) {
    if source[*cursor..].starts_with("//") {
        *cursor = source[*cursor..]
            .find('\n')
            .map_or(source.len(), |end| *cursor + end + 1);
    } else {
        *cursor = source[*cursor + 2..]
            .find("*/")
            .map_or(source.len(), |end| *cursor + end + 4);
    }
}

fn collapse_template_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_entities(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('&') {
        let ampersand = cursor + relative;
        output.push_str(&source[cursor..ampersand]);
        let Some(relative_end) = source[ampersand..].find(';') else {
            output.push_str(&source[ampersand..]);
            return output;
        };
        let end = ampersand + relative_end;
        let entity = &source[ampersand + 1..end];
        if let Some(decoded) = decode_entity(entity) {
            output.push(decoded);
            cursor = end + 1;
        } else {
            output.push('&');
            cursor = ampersand + 1;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        value if value.starts_with("#x") || value.starts_with("#X") => {
            u32::from_str_radix(&value[2..], 16)
                .ok()
                .and_then(char::from_u32)
        }
        value if value.starts_with('#') => value[1..].parse().ok().and_then(char::from_u32),
        _ => None,
    }
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

const fn is_identifier_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn is_ignored_identifier(name: &str) -> bool {
    matches!(
        name,
        "true" | "false" | "null" | "this" | "event" | "index" | "modelData"
    ) || is_qml_keyword(name)
        || matches!(
            name,
            "Math"
                | "Date"
                | "JSON"
                | "Object"
                | "Array"
                | "String"
                | "Number"
                | "Boolean"
                | "RegExp"
                | "Promise"
                | "console"
                | "Qt"
                | "qsTr"
                | "parseInt"
                | "parseFloat"
                | "isNaN"
                | "Infinity"
                | "NaN"
                | "Vue2Qml"
        )
}

fn is_qml_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "let"
            | "new"
            | "of"
            | "return"
            | "signal"
            | "static"
            | "super"
            | "switch"
            | "throw"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "property"
            | "readonly"
            | "required"
    )
}

#[cfg(test)]
mod tests {
    use super::{identifiers, normalize, qml_property_name, text_expression};

    #[test]
    fn normalizes_vue_only_identifiers() {
        assert_eq!(
            normalize("$q.notify(undefined, $event)"),
            "vue_q.notify(null, event)"
        );
        assert_eq!(qml_property_name("model-value"), "modelValue");
    }

    #[test]
    fn builds_mixed_text_expression() {
        assert_eq!(
            text_expression("Hello {{ user.name }}!"),
            "\"Hello\" + Vue2Qml.Runtime.display(user.name) + \"!\""
        );
    }

    #[test]
    fn collects_roots_but_not_member_names() {
        let usage = identifiers("activeTrack?.title || format(item.value)");
        assert!(usage.values.contains("activeTrack"));
        assert!(usage.values.contains("format"));
        assert!(usage.values.contains("item"));
        assert!(!usage.values.contains("title"));
        assert!(usage.calls.contains("format"));
    }
}
