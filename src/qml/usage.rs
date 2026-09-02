use std::collections::BTreeSet;

use crate::expression::{self, IdentifierUsage};
use crate::model::{Template, TemplateNode};

use super::attributes;
use super::{ComponentRegistry, EmissionContext};

#[derive(Clone, Debug, Default)]
pub(crate) struct TemplateUsage {
    pub identifiers: IdentifierUsage,
    pub custom_tags: BTreeSet<String>,
}

pub(crate) fn collect(template: &Template, context: &EmissionContext<'_>) -> TemplateUsage {
    let mut usage = TemplateUsage::default();
    for child in &template.children {
        visit(child, context.registry, &mut usage);
    }
    usage
}

fn visit(node: &TemplateNode, registry: &ComponentRegistry, usage: &mut TemplateUsage) {
    match node {
        TemplateNode::Text(text) => merge(
            &mut usage.identifiers,
            expression::identifiers(&expression::text_expression(text)),
        ),
        TemplateNode::Comment(_) => {}
        TemplateNode::Element(element) => {
            if registry.contains_key(&element.tag) {
                usage.custom_tags.insert(element.tag.clone());
            }
            let analyzed = attributes::analyze(&element.attributes);
            for (_, value) in &analyzed.bindings {
                merge_expression(&mut usage.identifiers, value);
            }
            for event in &analyzed.events {
                merge_expression(&mut usage.identifiers, &event.expression);
            }
            for value in [
                analyzed.condition.as_deref(),
                analyzed.else_condition.as_deref(),
                analyzed.show.as_deref(),
                analyzed.model.as_deref(),
                analyzed.html.as_deref(),
                analyzed.key.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                merge_expression(&mut usage.identifiers, value);
            }
            if let Some(reference) = &analyzed.reference {
                if !is_plain_reference(reference) {
                    merge_expression(&mut usage.identifiers, reference);
                }
            }
            if let Some(repeat) = &analyzed.repeat {
                merge_expression(&mut usage.identifiers, &repeat.source);
            }
            for child in &element.children {
                visit(child, registry, usage);
            }
        }
    }
}

fn merge_expression(usage: &mut IdentifierUsage, source: &str) {
    merge(usage, expression::identifiers(source));
}

fn merge(usage: &mut IdentifierUsage, other: IdentifierUsage) {
    usage.values.extend(other.values);
    usage.calls.extend(other.calls);
}

fn is_plain_reference(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
