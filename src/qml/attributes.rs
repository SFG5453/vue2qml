use crate::model::Attribute;

#[derive(Clone, Debug, Default)]
pub(crate) struct ElementAttributes {
    pub static_values: Vec<(String, Option<String>)>,
    pub bindings: Vec<(String, String)>,
    pub events: Vec<EventBinding>,
    pub condition: Option<String>,
    pub else_condition: Option<String>,
    pub is_else: bool,
    pub show: Option<String>,
    pub repeat: Option<RepeatBinding>,
    pub model: Option<String>,
    pub html: Option<String>,
    pub slot: Option<String>,
    pub key: Option<String>,
    pub reference: Option<String>,
    pub directives: Vec<(String, Option<String>)>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventBinding {
    pub name: String,
    pub expression: String,
    pub modifiers: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct RepeatBinding {
    pub source: String,
    pub value_name: String,
    pub key_name: Option<String>,
    pub index_name: Option<String>,
}

pub(crate) fn analyze(attributes: &[Attribute]) -> ElementAttributes {
    let mut result = ElementAttributes::default();
    for attribute in attributes {
        let name = attribute.name.as_str();
        let value = attribute.value.clone().unwrap_or_default();
        if let Some(binding) = name.strip_prefix(':') {
            add_binding(&mut result, binding, value);
        } else if let Some(event) = name.strip_prefix('@') {
            add_event(&mut result, event, value);
        } else if let Some(slot) = name.strip_prefix('#') {
            result.slot = Some(slot.to_owned());
        } else if let Some(binding) = name.strip_prefix("v-bind:") {
            add_binding(&mut result, binding, value);
        } else if let Some(event) = name.strip_prefix("v-on:") {
            add_event(&mut result, event, value);
        } else if name == "v-if" {
            result.condition = Some(value);
        } else if name == "v-else-if" {
            result.else_condition = Some(value);
        } else if name == "v-else" {
            result.is_else = true;
        } else if name == "v-show" {
            result.show = Some(value);
        } else if name == "v-for" {
            result.repeat = parse_repeat(&value);
        } else if name == "v-model" || name.starts_with("v-model.") {
            result.model = Some(value);
        } else if name == "v-html" {
            result.html = Some(value);
        } else if let Some(slot) = name.strip_prefix("v-slot:") {
            result.slot = Some(slot.to_owned());
        } else if name.starts_with("v-") {
            result
                .directives
                .push((name.to_owned(), attribute.value.clone()));
        } else if name == "ref" {
            result.reference = attribute.value.clone();
        } else {
            result
                .static_values
                .push((name.to_owned(), attribute.value.clone()));
        }
    }
    result
}

fn add_binding(result: &mut ElementAttributes, binding: &str, value: String) {
    let binding = binding.split('.').next().unwrap_or(binding);
    if binding == "key" {
        result.key = Some(value);
    } else if binding == "ref" {
        result.reference = Some(value);
    } else {
        result.bindings.push((binding.to_owned(), value));
    }
}

fn add_event(result: &mut ElementAttributes, event: &str, expression: String) {
    let mut parts = event.split('.');
    let name = parts.next().unwrap_or_default().to_owned();
    let modifiers = parts.map(ToOwned::to_owned).collect();
    result.events.push(EventBinding {
        name,
        expression,
        modifiers,
    });
}

fn parse_repeat(value: &str) -> Option<RepeatBinding> {
    let (aliases, source) = value
        .split_once(" in ")
        .or_else(|| value.split_once(" of "))?;
    let aliases = aliases
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| aliases.trim());
    let mut names = aliases.split(',').map(str::trim);
    let value_name = names.next()?.to_owned();
    if value_name.is_empty() {
        return None;
    }
    Some(RepeatBinding {
        source: source.trim().to_owned(),
        value_name,
        key_name: names
            .next()
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned),
        index_name: names
            .next()
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned),
    })
}

pub(crate) fn event_signal_name(name: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for character in name.chars() {
        if matches!(character, '-' | ':') {
            uppercase = true;
        } else if uppercase {
            output.extend(character.to_uppercase());
            uppercase = false;
        } else if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        }
    }
    let output = match output.as_str() {
        "click" => "clicked".to_owned(),
        "change" => "changed".to_owned(),
        "submit" => "submitted".to_owned(),
        "load" => "loaded".to_owned(),
        "play" => "played".to_owned(),
        "pause" => "paused".to_owned(),
        "drop" => "dropped".to_owned(),
        "paste" => "pasted".to_owned(),
        "pan" => "panned".to_owned(),
        _ => output,
    };
    let mut characters = output.chars();
    let Some(first) = characters.next() else {
        return "vueEvent".to_owned();
    };
    format!("vue{}{}", first.to_ascii_uppercase(), characters.as_str())
}

pub(crate) fn handler_name(signal: &str) -> String {
    let mut characters = signal.chars();
    let Some(first) = characters.next() else {
        return "onVueEvent".to_owned();
    };
    format!("on{}{}", first.to_ascii_uppercase(), characters.as_str())
}

#[cfg(test)]
mod tests {
    use crate::model::Attribute;

    use super::{analyze, event_signal_name};

    #[test]
    fn understands_shorthand_and_repeat_aliases() {
        let attributes = vec![
            Attribute {
                name: "v-for".to_owned(),
                value: Some("(item, index) in items".to_owned()),
            },
            Attribute {
                name: "@click.stop".to_owned(),
                value: Some("pick(item)".to_owned()),
            },
        ];
        let analyzed = analyze(&attributes);
        let repeat = analyzed.repeat.expect("repeat");
        assert_eq!(repeat.value_name, "item");
        assert_eq!(repeat.key_name.as_deref(), Some("index"));
        assert_eq!(
            event_signal_name("update:model-value"),
            "vueUpdateModelValue"
        );
    }
}
