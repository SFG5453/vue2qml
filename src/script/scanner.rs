pub(crate) fn property_object<'source>(source: &'source str, key: &str) -> Option<&'source str> {
    let key_start = find_identifier(source, key, 0)?;
    let mut cursor = key_start + key.len();
    skip_space_and_comments(source, &mut cursor);
    if source.as_bytes().get(cursor) != Some(&b':') {
        return property_object(&source[cursor..], key).map(|found| {
            let offset = found.as_ptr() as usize - source[cursor..].as_ptr() as usize;
            &source[cursor + offset..cursor + offset + found.len()]
        });
    }
    cursor += 1;
    skip_space_and_comments(source, &mut cursor);
    object_at(source, cursor)
}

pub(crate) fn call_object<'source>(source: &'source str, name: &str) -> Option<&'source str> {
    let start = find_identifier(source, name, 0)?;
    let mut cursor = start + name.len();
    skip_space_and_comments(source, &mut cursor);
    if source.as_bytes().get(cursor) != Some(&b'(') {
        return call_object(&source[cursor..], name).map(|found| {
            let offset = found.as_ptr() as usize - source[cursor..].as_ptr() as usize;
            &source[cursor + offset..cursor + offset + found.len()]
        });
    }
    cursor += 1;
    skip_space_and_comments(source, &mut cursor);
    object_at(source, cursor)
}

pub(crate) fn string_property(source: &str, key: &str) -> Option<String> {
    let start = find_identifier(source, key, 0)?;
    let mut cursor = start + key.len();
    skip_space_and_comments(source, &mut cursor);
    if source.as_bytes().get(cursor) != Some(&b':') {
        return string_property(&source[cursor..], key);
    }
    cursor += 1;
    skip_space_and_comments(source, &mut cursor);
    read_string(source, &mut cursor)
}

pub(crate) fn split_top_level(source: &str, separator: u8) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    let mut stack = Vec::new();
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if starts_comment(source, cursor) {
            skip_comment(source, &mut cursor);
            continue;
        }
        match byte {
            b'{' | b'[' | b'(' => stack.push(byte),
            b'}' | b']' | b')' => {
                stack.pop();
            }
            _ if byte == separator && stack.is_empty() => {
                parts.push(&source[start..cursor]);
                start = cursor + 1;
            }
            _ => {}
        }
        cursor += 1;
    }
    parts.push(&source[start..]);
    parts
}

pub(crate) fn top_level_colon(source: &str) -> Option<usize> {
    let mut cursor = 0;
    let mut stack = Vec::new();
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if starts_comment(source, cursor) {
            skip_comment(source, &mut cursor);
            continue;
        }
        match byte {
            b'{' | b'[' | b'(' => stack.push(byte),
            b'}' | b']' | b')' => {
                stack.pop();
            }
            b':' if stack.is_empty() => return Some(cursor),
            _ => {}
        }
        cursor += 1;
    }
    None
}

pub(crate) fn object_at(source: &str, open: usize) -> Option<&str> {
    if source.as_bytes().get(open) != Some(&b'{') {
        return None;
    }
    matching(source, open).map(|close| &source[open + 1..close])
}

pub(crate) fn import_statements(source: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut cursor = 0;
    while let Some(start) = find_identifier(source, "import", cursor) {
        let mut end = start + "import".len();
        let mut nesting = 0_u32;
        while end < source.len() {
            let byte = source.as_bytes()[end];
            if matches!(byte, b'\'' | b'"' | b'`') {
                skip_string(source, &mut end, byte);
                continue;
            }
            match byte {
                b'{' | b'[' | b'(' => nesting += 1,
                b'}' | b']' | b')' => nesting = nesting.saturating_sub(1),
                b';' if nesting == 0 => {
                    end += 1;
                    break;
                }
                b'\n' if nesting == 0 && source[start..end].contains(" from ") => break,
                _ => {}
            }
            end += 1;
        }
        statements.push(&source[start..end]);
        cursor = end;
    }
    statements
}

pub(crate) fn parse_import(statement: &str) -> Option<(&str, &str)> {
    let body = statement.strip_prefix("import")?.trim();
    let from = body.rfind(" from ")?;
    let binding = body[..from].trim();
    let mut path = body[from + 6..].trim().trim_end_matches(';').trim();
    if path.len() < 2 {
        return None;
    }
    let quote = path.as_bytes()[0];
    if !matches!(quote, b'\'' | b'"') || path.as_bytes().last() != Some(&quote) {
        return None;
    }
    path = &path[1..path.len() - 1];
    Some((binding, path))
}

pub(crate) fn declarations(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some((token, end)) = next_identifier(source, cursor) else {
            break;
        };
        cursor = end;
        if !matches!(token, "const" | "let" | "var" | "function" | "class") {
            continue;
        }
        let Some((name, name_end)) = next_identifier(source, cursor) else {
            break;
        };
        if !is_reserved(name) {
            names.push(name.to_owned());
        }
        cursor = name_end;
    }
    names
}

pub(crate) fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}

fn matching(source: &str, open: usize) -> Option<usize> {
    let opening = *source.as_bytes().get(open)?;
    let closing = match opening {
        b'{' => b'}',
        b'[' => b']',
        b'(' => b')',
        _ => return None,
    };
    let mut depth = 1_u32;
    let mut cursor = open + 1;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
            continue;
        }
        if starts_comment(source, cursor) {
            skip_comment(source, &mut cursor);
            continue;
        }
        if byte == opening {
            depth += 1;
        } else if byte == closing {
            depth -= 1;
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += 1;
    }
    None
}

fn find_identifier(source: &str, name: &str, mut cursor: usize) -> Option<usize> {
    while let Some((token, end)) = next_identifier(source, cursor) {
        let start = end - token.len();
        if token == name {
            return Some(start);
        }
        cursor = end;
    }
    None
}

fn next_identifier(source: &str, mut cursor: usize) -> Option<(&str, usize)> {
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if matches!(byte, b'\'' | b'"' | b'`') {
            skip_string(source, &mut cursor, byte);
        } else if starts_comment(source, cursor) {
            skip_comment(source, &mut cursor);
        } else if byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') {
            let start = cursor;
            cursor += 1;
            while source
                .as_bytes()
                .get(cursor)
                .is_some_and(|next| next.is_ascii_alphanumeric() || matches!(next, b'_' | b'$'))
            {
                cursor += 1;
            }
            return Some((&source[start..cursor], cursor));
        } else {
            cursor += 1;
        }
    }
    None
}

fn read_string(source: &str, cursor: &mut usize) -> Option<String> {
    let quote @ (b'\'' | b'"') = *source.as_bytes().get(*cursor)? else {
        return None;
    };
    *cursor += 1;
    let start = *cursor;
    while let Some(byte) = source.as_bytes().get(*cursor).copied() {
        if byte == b'\\' {
            *cursor += 2;
        } else if byte == quote {
            let value = source[start..*cursor].to_owned();
            *cursor += 1;
            return Some(value);
        } else {
            *cursor += 1;
        }
    }
    None
}

fn skip_space_and_comments(source: &str, cursor: &mut usize) {
    loop {
        while source
            .as_bytes()
            .get(*cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            *cursor += 1;
        }
        if starts_comment(source, *cursor) {
            skip_comment(source, cursor);
        } else {
            break;
        }
    }
}

fn starts_comment(source: &str, cursor: usize) -> bool {
    source[cursor..].starts_with("//") || source[cursor..].starts_with("/*")
}

fn skip_comment(source: &str, cursor: &mut usize) {
    if source[*cursor..].starts_with("//") {
        *cursor = source[*cursor..]
            .find('\n')
            .map_or(source.len(), |end| *cursor + end + 1);
    } else if source[*cursor..].starts_with("/*") {
        *cursor = source[*cursor + 2..]
            .find("*/")
            .map_or(source.len(), |end| *cursor + 2 + end + 2);
    }
}

fn skip_string(source: &str, cursor: &mut usize, quote: u8) {
    *cursor += 1;
    while let Some(byte) = source.as_bytes().get(*cursor).copied() {
        if byte == b'\\' {
            *cursor += 2;
        } else {
            *cursor += 1;
            if byte == quote {
                break;
            }
        }
    }
}

fn is_reserved(value: &str) -> bool {
    matches!(
        value,
        "if" | "else" | "for" | "while" | "return" | "switch" | "catch" | "finally"
    )
}
