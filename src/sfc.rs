use std::collections::BTreeMap;

use crate::error::{Error, ErrorKind, Result};
use crate::model::{Block, Sfc};

pub fn parse_sfc(source: &str) -> Result<Sfc> {
    let mut sfc = Sfc::default();
    let mut cursor = 0;
    while let Some(open) = find_next_opening(source, cursor) {
        let header = parse_opening_tag(source, open)?;
        let close = find_block_close(source, header.content_start, &header.name)?;
        let block = Block {
            attributes: parse_attributes(&source[header.attributes_start..header.attributes_end])?,
            content: source[header.content_start..close.start].to_owned(),
            start: open,
            end: close.end,
        };
        match header.name.as_str() {
            "template" => {
                if sfc.template.replace(block).is_some() {
                    return Err(Error::new(
                        ErrorKind::InvalidSfc,
                        "an SFC may only contain one top-level <template> block",
                    ));
                }
            }
            "script" if block.attributes.contains_key("setup") => {
                if sfc.script_setup.replace(block).is_some() {
                    return Err(Error::new(
                        ErrorKind::InvalidSfc,
                        "an SFC may only contain one <script setup> block",
                    ));
                }
            }
            "script" => {
                if sfc.script.replace(block).is_some() {
                    return Err(Error::new(
                        ErrorKind::InvalidSfc,
                        "an SFC may only contain one normal <script> block",
                    ));
                }
            }
            "style" => sfc.styles.push(block),
            _ => sfc.custom_blocks.push((header.name, block)),
        }
        cursor = close.end;
    }
    if sfc.template.is_none() {
        return Err(Error::new(
            ErrorKind::InvalidSfc,
            "the SFC does not contain a top-level <template> block",
        ));
    }
    Ok(sfc)
}

struct OpeningTag {
    name: String,
    attributes_start: usize,
    attributes_end: usize,
    content_start: usize,
}

struct ClosingTag {
    start: usize,
    end: usize,
}

fn find_next_opening(source: &str, mut cursor: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    while cursor < bytes.len() {
        let relative = source[cursor..].find('<')?;
        let position = cursor + relative;
        if source[position..].starts_with("<!--") {
            cursor = source[position + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| position + 4 + end + 3);
            continue;
        }
        let next = bytes.get(position + 1).copied();
        if next.is_some_and(is_name_start) {
            return Some(position);
        }
        cursor = position + 1;
    }
    None
}

fn parse_opening_tag(source: &str, open: usize) -> Result<OpeningTag> {
    let bytes = source.as_bytes();
    let mut cursor = open + 1;
    let name_start = cursor;
    while bytes.get(cursor).is_some_and(|byte| is_name_part(*byte)) {
        cursor += 1;
    }
    let name = source[name_start..cursor].to_ascii_lowercase();
    let attributes_start = cursor;
    let attributes_end = find_tag_end(source, cursor)?;
    if source[attributes_start..attributes_end]
        .trim_end()
        .ends_with('/')
    {
        return Err(Error::new(
            ErrorKind::InvalidSfc,
            format!("top-level <{name}> block cannot be self-closing"),
        ));
    }
    Ok(OpeningTag {
        name,
        attributes_start,
        attributes_end,
        content_start: attributes_end + 1,
    })
}

fn find_tag_end(source: &str, mut cursor: usize) -> Result<usize> {
    let bytes = source.as_bytes();
    let mut quote = None;
    while let Some(&byte) = bytes.get(cursor) {
        if let Some(active) = quote {
            if byte == active {
                quote = None;
            } else if byte == b'\\' {
                cursor += 1;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if byte == b'>' {
            return Ok(cursor);
        }
        cursor += 1;
    }
    Err(Error::new(
        ErrorKind::InvalidSfc,
        "unterminated top-level opening tag",
    ))
}

fn find_block_close(source: &str, content_start: usize, name: &str) -> Result<ClosingTag> {
    if name != "template" {
        return find_simple_close(source, content_start, name);
    }
    let mut depth = 1_u32;
    let mut cursor = content_start;
    while let Some(relative) = source[cursor..].find('<') {
        let position = cursor + relative;
        if source[position..].starts_with("<!--") {
            cursor = source[position + 4..]
                .find("-->")
                .map_or(source.len(), |end| position + 4 + end + 3);
            continue;
        }
        if source[position..].starts_with("</") {
            let (tag, end) = read_tag_name_and_end(source, position + 2)?;
            if tag.eq_ignore_ascii_case(name) {
                depth -= 1;
                if depth == 0 {
                    return Ok(ClosingTag {
                        start: position,
                        end,
                    });
                }
            }
            cursor = end;
            continue;
        }
        let next = source.as_bytes().get(position + 1).copied();
        if next.is_some_and(is_name_start) {
            let (tag, end) = read_tag_name_and_end(source, position + 1)?;
            let self_closing = source[position..end]
                .trim_end_matches('>')
                .trim_end()
                .ends_with('/');
            if tag.eq_ignore_ascii_case(name) && !self_closing {
                depth += 1;
            }
            cursor = end;
        } else {
            cursor = position + 1;
        }
    }
    Err(Error::new(
        ErrorKind::InvalidSfc,
        format!("missing closing </{name}> tag"),
    ))
}

fn find_simple_close(source: &str, cursor: usize, name: &str) -> Result<ClosingTag> {
    let needle = format!("</{name}");
    let lower = source[cursor..].to_ascii_lowercase();
    let relative = lower.find(&needle).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidSfc,
            format!("missing closing </{name}> tag"),
        )
    })?;
    let start = cursor + relative;
    let (_, end) = read_tag_name_and_end(source, start + 2)?;
    Ok(ClosingTag { start, end })
}

fn read_tag_name_and_end(source: &str, start: usize) -> Result<(&str, usize)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while bytes.get(cursor).is_some_and(|byte| is_name_part(*byte)) {
        cursor += 1;
    }
    let name = &source[start..cursor];
    let end = find_tag_end(source, cursor)? + 1;
    Ok((name, end))
}

fn parse_attributes(source: &str) -> Result<BTreeMap<String, Option<String>>> {
    let mut attributes = BTreeMap::new();
    let bytes = source.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if cursor == bytes.len() || bytes[cursor] == b'/' {
            break;
        }
        let start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'/' | b'>'))
        {
            cursor += 1;
        }
        if start == cursor {
            return Err(Error::new(ErrorKind::InvalidSfc, "invalid block attribute"));
        }
        let name = source[start..cursor].to_ascii_lowercase();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let value = if bytes.get(cursor) == Some(&b'=') {
            cursor += 1;
            while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                cursor += 1;
            }
            Some(read_attribute_value(source, &mut cursor)?)
        } else {
            None
        };
        attributes.insert(name, value);
    }
    Ok(attributes)
}

fn read_attribute_value(source: &str, cursor: &mut usize) -> Result<String> {
    let bytes = source.as_bytes();
    if let Some(&quote @ (b'\'' | b'"')) = bytes.get(*cursor) {
        *cursor += 1;
        let start = *cursor;
        while bytes.get(*cursor).is_some_and(|byte| *byte != quote) {
            *cursor += 1;
        }
        if *cursor == bytes.len() {
            return Err(Error::new(
                ErrorKind::InvalidSfc,
                "unterminated quoted block attribute",
            ));
        }
        let value = source[start..*cursor].to_owned();
        *cursor += 1;
        Ok(value)
    } else {
        let start = *cursor;
        while bytes
            .get(*cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'>')
        {
            *cursor += 1;
        }
        Ok(source[start..*cursor].to_owned())
    }
}

const fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

const fn is_name_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':')
}

#[cfg(test)]
mod tests {
    use super::parse_sfc;

    #[test]
    fn nested_template_is_not_the_sfc_close() {
        let source = "<template><template v-if=\"yes\">x</template></template>";
        let parsed = parse_sfc(source).expect("valid SFC");
        assert_eq!(
            parsed.template.expect("template").content,
            "<template v-if=\"yes\">x</template>"
        );
    }

    #[test]
    fn separates_setup_and_normal_scripts() {
        let source = concat!(
            "<script>export default {}</script>",
            "<script setup lang='js'>let x = 1</script>",
            "<template><div /></template>",
            "<style scoped>.x { color: red; }</style>"
        );
        let parsed = parse_sfc(source).expect("valid SFC");
        assert!(parsed.script.is_some());
        assert!(parsed.script_setup.is_some());
        assert_eq!(parsed.styles.len(), 1);
    }
}
