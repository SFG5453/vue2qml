pub(super) fn contains_unsupported_syntax(source: &str) -> bool {
    let mut cursor = 0;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if source.as_bytes().get(cursor..cursor + 3) == Some(b"...") {
            return true;
        }
        if byte == b'[' && is_computed_property(source, cursor) {
            return true;
        }
        cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn is_computed_property(source: &str, open: usize) -> bool {
    let previous = source.as_bytes()[..open]
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    if !matches!(previous, Some(b'{') | Some(b',')) {
        return false;
    }
    let Some(close) = matching_bracket(source, open) else {
        return false;
    };
    source.as_bytes()[close + 1..]
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b':')
}

fn matching_bracket(source: &str, open: usize) -> Option<usize> {
    let mut depth = 1_u32;
    let mut cursor = open + 1;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if byte == b'[' {
            depth += 1;
        } else if byte == b']' {
            depth -= 1;
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
    }
    None
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

#[cfg(test)]
mod tests {
    use super::contains_unsupported_syntax;

    #[test]
    fn detects_object_extensions_but_not_arrays() {
        assert!(contains_unsupported_syntax("({ ...style })"));
        assert!(contains_unsupported_syntax("({ [`x-${id}`]: true })"));
        assert!(!contains_unsupported_syntax("items[index]"));
        assert!(!contains_unsupported_syntax("['a', 'b']"));
    }
}
