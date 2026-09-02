use crate::model::{ComponentProperty, PropertyType};

use super::scanner::{object_at, split_top_level, top_level_colon};

pub(crate) fn parse(source: &str) -> Vec<ComponentProperty> {
    split_top_level(source, b',')
        .into_iter()
        .filter_map(parse_entry)
        .collect()
}

fn parse_entry(entry: &str) -> Option<ComponentProperty> {
    let entry = entry.trim();
    if entry.is_empty() || entry.starts_with("...") {
        return None;
    }
    let colon = top_level_colon(entry)?;
    let name = unquote(entry[..colon].trim())?;
    let specification = entry[colon + 1..].trim();
    if specification.starts_with('{') {
        parse_specification(name, specification)
    } else {
        Some(ComponentProperty {
            name: name.to_owned(),
            property_type: refine_type(name, type_from_expression(specification)),
            required: false,
            default_value: None,
        })
    }
}

fn parse_specification(name: &str, specification: &str) -> Option<ComponentProperty> {
    let body = object_at(specification, 0)?;
    let mut property_type = PropertyType::Var;
    let mut required = false;
    let mut default_value = None;
    for field in split_top_level(body, b',') {
        let Some(colon) = top_level_colon(field) else {
            continue;
        };
        let key = field[..colon].trim();
        let value = field[colon + 1..].trim();
        match key {
            "type" => property_type = type_from_expression(value),
            "required" => required = value == "true",
            "default" => default_value = normalize_default(value),
            _ => {}
        }
    }
    Some(ComponentProperty {
        name: name.to_owned(),
        property_type: refine_type(name, property_type),
        required,
        default_value,
    })
}

fn refine_type(name: &str, property_type: PropertyType) -> PropertyType {
    if property_type == PropertyType::String && name.to_ascii_lowercase().ends_with("url") {
        PropertyType::Url
    } else if property_type == PropertyType::Real && name.to_ascii_lowercase().contains("index") {
        PropertyType::Int
    } else {
        property_type
    }
}

fn type_from_expression(value: &str) -> PropertyType {
    let value = value.trim();
    if value.contains("Boolean") {
        PropertyType::Bool
    } else if value.contains("Number") {
        PropertyType::Real
    } else if value.contains("String") {
        PropertyType::String
    } else {
        PropertyType::Var
    }
}

fn normalize_default(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with("()") {
        let arrow = value.find("=>")?;
        let result = value[arrow + 2..].trim();
        return Some(
            result
                .strip_prefix('(')
                .and_then(|v| v.strip_suffix(')'))
                .unwrap_or(result)
                .to_owned(),
        );
    }
    if value.starts_with("function") {
        return None;
    }
    Some(value.to_owned())
}

fn unquote(value: &str) -> Option<&str> {
    if let Some(value) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(value)
    } else if let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(value)
    } else if super::scanner::is_identifier(value) {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::model::PropertyType;

    #[test]
    fn parses_vue_property_specifications() {
        let props = parse(
            "app: { type: Object, required: true }, enabled: { type: Boolean, default: false }",
        );
        assert_eq!(props.len(), 2);
        assert!(props[0].required);
        assert_eq!(props[1].property_type, PropertyType::Bool);
        assert_eq!(props[1].default_value.as_deref(), Some("false"));
    }
}
