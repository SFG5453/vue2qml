use std::collections::BTreeMap;

use crate::expression::{self, qml_property_name, qml_string};
use crate::model::{Element, PropertyType, TemplateNode};

use super::EmissionContext;
use super::attributes::{self, ElementAttributes, EventBinding, RepeatBinding};
use super::writer::Writer;

pub(crate) struct NodeEmitter<'writer, 'context> {
    writer: &'writer mut Writer,
    context: &'context EmissionContext<'context>,
}

impl<'writer, 'context> NodeEmitter<'writer, 'context> {
    pub fn new(writer: &'writer mut Writer, context: &'context EmissionContext<'context>) -> Self {
        Self { writer, context }
    }

    pub fn emit_children(&mut self, children: &[TemplateNode]) {
        let mut branch_conditions = Vec::new();
        for child in children {
            match child {
                TemplateNode::Element(element) => {
                    let attributes = attributes::analyze(&element.attributes);
                    let condition = branch_condition(&attributes, &mut branch_conditions);
                    self.emit_element(element, &attributes, condition.as_deref());
                }
                TemplateNode::Text(text) => {
                    if !text.trim().is_empty() {
                        branch_conditions.clear();
                        self.emit_text(text);
                    }
                }
                TemplateNode::Comment(_) => {}
            }
        }
    }

    fn emit_element(
        &mut self,
        element: &Element,
        attributes: &ElementAttributes,
        condition: Option<&str>,
    ) {
        if let Some(repeat) = &attributes.repeat {
            self.writer.open("Repeater");
            self.writer.formatted_line(format_args!(
                "model: Vue2Qml.Runtime.toModel({})",
                expression::normalize(&repeat.source)
            ));
            let qml_type = self.qml_type(element);
            self.writer.open(format!("delegate: {qml_type}"));
            emit_repeat_aliases(self.writer, repeat);
            self.emit_element_body(element, attributes, condition);
            self.writer.close();
            self.writer.close();
            return;
        }
        let qml_type = self.qml_type(element);
        self.writer.open(qml_type);
        self.emit_element_body(element, attributes, condition);
        self.writer.close();
    }

    fn emit_element_body(
        &mut self,
        element: &Element,
        attributes: &ElementAttributes,
        condition: Option<&str>,
    ) {
        if let Some(reference) = attributes
            .reference
            .as_deref()
            .filter(|value| is_qml_id(value))
        {
            self.writer.formatted_line(format_args!("id: {reference}"));
        }
        self.writer
            .formatted_line(format_args!("tag: {}", qml_string(&element.tag)));
        emit_static_attributes(self.writer, attributes);
        emit_dynamic_attributes(self.writer, attributes);
        emit_directives(self.writer, attributes);
        self.emit_custom_properties(element, attributes);
        emit_metadata(self.writer, attributes, condition);
        emit_events(self.writer, &attributes.events);
        self.emit_children(&element.children);
    }

    fn emit_custom_properties(&mut self, element: &Element, attributes: &ElementAttributes) {
        let Some(component) = self.context.component(&element.tag) else {
            return;
        };
        for property in &component.properties {
            let qml_name = qml_property_name(&property.name);
            if let Some((_, value)) = attributes
                .bindings
                .iter()
                .find(|(name, _)| qml_property_name(name) == qml_name)
            {
                self.writer
                    .formatted_line(format_args!("{qml_name}: {}", expression::normalize(value)));
                continue;
            }
            if let Some((_, value)) = attributes
                .static_values
                .iter()
                .find(|(name, _)| qml_property_name(name) == qml_name)
            {
                self.writer.formatted_line(format_args!(
                    "{qml_name}: {}",
                    static_property_value(value.as_deref(), &property.property_type)
                ));
            }
        }
    }

    fn emit_text(&mut self, text: &str) {
        let value = expression::text_expression(text);
        if value == "\"\"" {
            return;
        }
        self.writer.open("Vue2Qml.VueElement");
        self.writer.line("tag: \"#text\"");
        self.writer
            .formatted_line(format_args!("textContent: {value}"));
        self.writer.close();
    }

    fn qml_type(&self, element: &Element) -> String {
        if self.context.component(&element.tag).is_some() {
            element.tag.clone()
        } else {
            "Vue2Qml.VueElement".to_owned()
        }
    }
}

fn branch_condition(attributes: &ElementAttributes, branches: &mut Vec<String>) -> Option<String> {
    if let Some(condition) = &attributes.condition {
        branches.clear();
        branches.push(condition.clone());
        return Some(condition.clone());
    }
    if let Some(condition) = &attributes.else_condition {
        let previous = branches.join(" || ");
        branches.push(condition.clone());
        return Some(if previous.is_empty() {
            condition.clone()
        } else {
            format!("!({previous}) && ({condition})")
        });
    }
    if attributes.is_else {
        let previous = branches.join(" || ");
        branches.clear();
        return (!previous.is_empty()).then(|| format!("!({previous})"));
    }
    branches.clear();
    None
}

fn emit_repeat_aliases(writer: &mut Writer, repeat: &RepeatBinding) {
    if repeat.value_name != "modelData" && repeat.value_name != "index" {
        writer.line("required property var modelData");
        writer.formatted_line(format_args!(
            "property var {}: modelData",
            qml_property_name(&repeat.value_name)
        ));
    } else if repeat.value_name == "modelData" {
        writer.line("required property var modelData");
    }
    if repeat.value_name == "index" || repeat.key_name.is_some() || repeat.index_name.is_some() {
        writer.line("required property int index");
    }
    if let Some(key) = &repeat.key_name {
        if key != "index" {
            writer.formatted_line(format_args!(
                "property int {}: index",
                qml_property_name(key)
            ));
        }
    }
    if let Some(index) = &repeat.index_name {
        if index != "index" {
            writer.formatted_line(format_args!(
                "property int {}: index",
                qml_property_name(index)
            ));
        }
    }
}

fn emit_static_attributes(writer: &mut Writer, attributes: &ElementAttributes) {
    let values = attributes
        .static_values
        .iter()
        .map(|(name, value)| {
            let value = value
                .as_deref()
                .map_or_else(|| "true".to_owned(), qml_string);
            format!("{}: {value}", qml_string(name))
        })
        .collect::<Vec<_>>()
        .join(", ");
    writer.formatted_line(format_args!("staticAttributes: ({{{values}}})"));
}

fn emit_dynamic_attributes(writer: &mut Writer, attributes: &ElementAttributes) {
    let values = attributes
        .bindings
        .iter()
        .map(|(name, value)| format!("{}: {}", qml_string(name), expression::normalize(value)))
        .collect::<Vec<_>>()
        .join(", ");
    writer.formatted_line(format_args!("dynamicAttributes: ({{{values}}})"));
}

fn emit_directives(writer: &mut Writer, attributes: &ElementAttributes) {
    if attributes.directives.is_empty() {
        return;
    }
    let values = attributes
        .directives
        .iter()
        .map(|(name, value)| {
            let value = value
                .as_deref()
                .map_or_else(|| "true".to_owned(), expression::normalize);
            format!("{}: {value}", qml_string(name))
        })
        .collect::<Vec<_>>()
        .join(", ");
    writer.formatted_line(format_args!("directives: ({{{values}}})"));
}

fn emit_metadata(writer: &mut Writer, attributes: &ElementAttributes, condition: Option<&str>) {
    let combined_condition = match (condition, attributes.show.as_deref()) {
        (Some(left), Some(right)) => Some(format!("({left}) && ({right})")),
        (Some(value), None) | (None, Some(value)) => Some(value.to_owned()),
        (None, None) => None,
    };
    if let Some(condition) = combined_condition {
        writer.formatted_line(format_args!(
            "condition: {}",
            expression::normalize(&condition)
        ));
    }
    if let Some(value) = &attributes.model {
        writer.formatted_line(format_args!("modelValue: {}", expression::normalize(value)));
    }
    if let Some(value) = &attributes.html {
        writer.formatted_line(format_args!(
            "htmlContent: {}",
            expression::normalize(value)
        ));
    }
    if let Some(value) = &attributes.key {
        writer.formatted_line(format_args!("vueKey: {}", expression::normalize(value)));
    }
    if let Some(value) = &attributes.reference {
        let value = if is_qml_id(value) {
            qml_string(value)
        } else {
            format!("Vue2Qml.Runtime.display({})", expression::normalize(value))
        };
        writer.formatted_line(format_args!("vueRef: {value}"));
    }
    if let Some(value) = &attributes.slot {
        writer.formatted_line(format_args!("slotName: {}", qml_string(value)));
    }
}

fn emit_events(writer: &mut Writer, events: &[EventBinding]) {
    let mut grouped = BTreeMap::<String, Vec<&EventBinding>>::new();
    for event in events {
        grouped
            .entry(attributes::event_signal_name(&event.name))
            .or_default()
            .push(event);
    }
    for (signal, bindings) in grouped {
        writer.open(format!("{}: event =>", attributes::handler_name(&signal)));
        for binding in bindings {
            let modifiers = binding
                .modifiers
                .iter()
                .map(|modifier| qml_string(modifier))
                .collect::<Vec<_>>()
                .join(", ");
            writer.open(format!(
                "if (Vue2Qml.Runtime.prepareEvent(event, [{modifiers}]))"
            ));
            let expression = event_expression(&binding.expression);
            writer.line(expression);
            writer.close();
        }
        writer.close();
    }
}

fn event_expression(source: &str) -> String {
    let normalized = expression::normalize(source.trim());
    if is_identifier(&normalized) {
        format!("{normalized}(event)")
    } else if normalized.is_empty() {
        "// No event expression was supplied.".to_owned()
    } else {
        normalized
    }
}

fn static_property_value(value: Option<&str>, property_type: &PropertyType) -> String {
    match (value, property_type) {
        (None, PropertyType::Bool) => "true".to_owned(),
        (Some("true"), PropertyType::Bool) => "true".to_owned(),
        (Some("false"), PropertyType::Bool) => "false".to_owned(),
        (Some(value), PropertyType::Int | PropertyType::Real) => value.to_owned(),
        (Some(value), _) => qml_string(value),
        (None, _) => qml_string(""),
    }
}

fn is_qml_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}
