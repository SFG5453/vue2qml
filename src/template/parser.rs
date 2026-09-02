use crate::error::{Error, ErrorKind, Result};
use crate::model::{Attribute, Element, Template, TemplateNode};

pub fn parse_template(source: &str) -> Result<Template> {
    let mut parser = Parser { source, cursor: 0 };
    let children = parser.parse_children(None)?;
    Ok(Template { children })
}

struct Parser<'source> {
    source: &'source str,
    cursor: usize,
}

impl Parser<'_> {
    fn parse_children(&mut self, expected: Option<&str>) -> Result<Vec<TemplateNode>> {
        let mut children = Vec::new();
        while self.cursor < self.source.len() {
            if self.remaining().starts_with("<!--") {
                children.push(self.parse_comment()?);
            } else if self.remaining().starts_with("</") {
                let closing = self.parse_closing_tag()?;
                let Some(expected) = expected else {
                    return Err(self.error(format!("unexpected closing </{closing}> tag")));
                };
                if !closing.eq_ignore_ascii_case(expected) {
                    return Err(self.error(format!(
                        "expected closing </{expected}> tag, found </{closing}>"
                    )));
                }
                return Ok(children);
            } else if self.remaining().starts_with("<!") {
                children.push(self.parse_declaration()?);
            } else if self.is_opening_tag() {
                children.push(TemplateNode::Element(self.parse_element()?));
            } else {
                children.push(TemplateNode::Text(self.parse_text()?));
            }
        }
        if let Some(expected) = expected {
            Err(self.error(format!("missing closing </{expected}> tag")))
        } else {
            Ok(children)
        }
    }

    fn parse_element(&mut self) -> Result<Element> {
        let start = self.cursor;
        self.cursor += 1;
        let tag = self.read_name()?;
        let mut attributes = Vec::new();
        let mut self_closing = false;
        loop {
            self.skip_whitespace();
            if self.remaining().starts_with("/>") {
                self.cursor += 2;
                self_closing = true;
                break;
            }
            if self.remaining().starts_with('>') {
                self.cursor += 1;
                break;
            }
            if self.cursor == self.source.len() {
                return Err(self.error(format!("unterminated <{tag}> opening tag")));
            }
            attributes.push(self.parse_attribute()?);
        }
        if is_void_tag(&tag) {
            self_closing = true;
        }
        let children = if self_closing {
            Vec::new()
        } else {
            self.parse_children(Some(&tag))?
        };
        Ok(Element {
            tag,
            attributes,
            children,
            self_closing,
            start,
            end: self.cursor,
        })
    }

    fn parse_attribute(&mut self) -> Result<Attribute> {
        let start = self.cursor;
        while let Some(byte) = self.byte() {
            if byte.is_ascii_whitespace() || matches!(byte, b'=' | b'>' | b'/') {
                break;
            }
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err(self.error("invalid template attribute"));
        }
        let name = self.source[start..self.cursor].to_owned();
        self.skip_whitespace();
        let value = if self.byte() == Some(b'=') {
            self.cursor += 1;
            self.skip_whitespace();
            Some(self.read_attribute_value()?)
        } else {
            None
        };
        Ok(Attribute { name, value })
    }

    fn read_attribute_value(&mut self) -> Result<String> {
        if let Some(quote @ (b'\'' | b'"')) = self.byte() {
            self.cursor += 1;
            let start = self.cursor;
            while let Some(byte) = self.byte() {
                if byte == quote {
                    let value = self.source[start..self.cursor].to_owned();
                    self.cursor += 1;
                    return Ok(value);
                }
                self.cursor += 1;
            }
            Err(self.error("unterminated quoted template attribute"))
        } else {
            let start = self.cursor;
            while let Some(byte) = self.byte() {
                if byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/') {
                    break;
                }
                self.cursor += 1;
            }
            if start == self.cursor {
                Err(self.error("template attribute is missing its value"))
            } else {
                Ok(self.source[start..self.cursor].to_owned())
            }
        }
    }

    fn parse_closing_tag(&mut self) -> Result<String> {
        self.cursor += 2;
        self.skip_whitespace();
        let tag = self.read_name()?;
        self.skip_whitespace();
        if self.byte() != Some(b'>') {
            return Err(self.error(format!("invalid closing </{tag}> tag")));
        }
        self.cursor += 1;
        Ok(tag)
    }

    fn parse_comment(&mut self) -> Result<TemplateNode> {
        self.cursor += 4;
        let Some(relative_end) = self.remaining().find("-->") else {
            return Err(self.error("unterminated template comment"));
        };
        let end = self.cursor + relative_end;
        let comment = self.source[self.cursor..end].to_owned();
        self.cursor = end + 3;
        Ok(TemplateNode::Comment(comment))
    }

    fn parse_declaration(&mut self) -> Result<TemplateNode> {
        let start = self.cursor + 2;
        let Some(relative_end) = self.source[start..].find('>') else {
            return Err(self.error("unterminated template declaration"));
        };
        let end = start + relative_end;
        let declaration = self.source[start..end].to_owned();
        self.cursor = end + 1;
        Ok(TemplateNode::Comment(declaration))
    }

    fn parse_text(&mut self) -> Result<String> {
        let start = self.cursor;
        while self.cursor < self.source.len() {
            if self.remaining().starts_with("{{") {
                self.skip_interpolation()?;
                continue;
            }
            if self.byte() == Some(b'<') && self.starts_markup() {
                break;
            }
            self.cursor += self.current_char_len();
        }
        if start == self.cursor {
            return Err(self.error("could not parse template text"));
        }
        Ok(self.source[start..self.cursor].to_owned())
    }

    fn skip_interpolation(&mut self) -> Result<()> {
        self.cursor += 2;
        let mut quote = None;
        let mut escaped = false;
        while self.cursor < self.source.len() {
            let byte = self.byte().expect("cursor is in bounds");
            if let Some(active) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == active {
                    quote = None;
                }
                self.cursor += self.current_char_len();
                continue;
            }
            if matches!(byte, b'\'' | b'"' | b'`') {
                quote = Some(byte);
                self.cursor += 1;
            } else if self.remaining().starts_with("}}") {
                self.cursor += 2;
                return Ok(());
            } else {
                self.cursor += self.current_char_len();
            }
        }
        Err(self.error("unterminated template interpolation"))
    }

    fn read_name(&mut self) -> Result<String> {
        let start = self.cursor;
        while self.byte().is_some_and(is_name_part) {
            self.cursor += 1;
        }
        if start == self.cursor {
            Err(self.error("expected a template tag name"))
        } else {
            Ok(self.source[start..self.cursor].to_owned())
        }
    }

    fn starts_markup(&self) -> bool {
        let bytes = self.source.as_bytes();
        let Some(next) = bytes.get(self.cursor + 1).copied() else {
            return false;
        };
        next == b'!' || next == b'/' || is_name_start(next)
    }

    fn is_opening_tag(&self) -> bool {
        self.byte() == Some(b'<')
            && self
                .source
                .as_bytes()
                .get(self.cursor + 1)
                .copied()
                .is_some_and(is_name_start)
    }

    fn skip_whitespace(&mut self) {
        while self.byte().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.cursor += 1;
        }
    }

    fn remaining(&self) -> &str {
        &self.source[self.cursor..]
    }

    fn byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.cursor).copied()
    }

    fn current_char_len(&self) -> usize {
        self.remaining().chars().next().map_or(1, char::len_utf8)
    }

    fn error(&self, message: impl Into<String>) -> Error {
        Error::new(
            ErrorKind::InvalidTemplate,
            format!("byte {}: {}", self.cursor, message.into()),
        )
    }
}

const fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

const fn is_name_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_template;
    use crate::model::TemplateNode;

    #[test]
    fn parses_vue_attributes_and_interpolation() {
        let template = parse_template(
            r#"<button v-if="ready" @click.stop="go(item)">{{ item?.name }}</button>"#,
        )
        .expect("valid template");
        let TemplateNode::Element(button) = &template.children[0] else {
            panic!("expected an element");
        };
        assert_eq!(button.tag, "button");
        assert_eq!(button.attributes.len(), 2);
        assert_eq!(button.children.len(), 1);
    }

    #[test]
    fn less_than_inside_interpolation_is_text() {
        let template =
            parse_template("<p>{{ count < limit ? count : limit }}</p>").expect("valid template");
        assert_eq!(template.children.len(), 1);
    }
}
