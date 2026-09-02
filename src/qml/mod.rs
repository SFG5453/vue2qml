mod attributes;
mod context;
mod emitter;
mod node;
mod runtime;
mod usage;
mod writer;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::model::{ComponentProperty, Sfc, Template};
use crate::{parse_sfc, parse_template};

pub(crate) use context::ComponentRegistry;
pub(crate) use runtime::write_runtime;

pub fn convert_component(source: &str) -> Result<String> {
    let sfc = parse_sfc(source)?;
    let template = parse_sfc_template(&sfc)?;
    let script = crate::script::analyze(&sfc);
    let context = EmissionContext {
        source_path: Path::new("Component.vue"),
        output_path: Path::new("Component.qml"),
        output_root: Path::new("."),
        registry: &BTreeMap::new(),
    };
    emitter::emit(&sfc, &template, &script, &context)
}

pub(crate) fn convert_project_component(
    source: &str,
    source_path: &Path,
    output_path: &Path,
    output_root: &Path,
    registry: &ComponentRegistry,
) -> Result<String> {
    let sfc = parse_sfc(source)?;
    let template = parse_sfc_template(&sfc)?;
    let script = crate::script::analyze(&sfc);
    let context = EmissionContext {
        source_path,
        output_path,
        output_root,
        registry,
    };
    emitter::emit(&sfc, &template, &script, &context)
}

fn parse_sfc_template(sfc: &Sfc) -> Result<Template> {
    let block = sfc
        .template
        .as_ref()
        .expect("parse_sfc guarantees a template block");
    parse_template(&block.content)
}

#[derive(Clone, Debug)]
pub(crate) struct RegisteredComponent {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub properties: Vec<ComponentProperty>,
}

pub(crate) struct EmissionContext<'context> {
    pub source_path: &'context Path,
    pub output_path: &'context Path,
    pub output_root: &'context Path,
    pub registry: &'context ComponentRegistry,
}

impl EmissionContext<'_> {
    pub fn component(&self, name: &str) -> Option<&RegisteredComponent> {
        self.registry.get(name)
    }

    pub fn component_import(&self, name: &str) -> Option<String> {
        let component = self.component(name)?;
        let target = component.output_path.parent()?;
        let current = self.output_path.parent()?;
        if target == current {
            return None;
        }
        Some(relative_path(current, target))
    }

    pub fn runtime_import(&self) -> String {
        let current = self.output_path.parent().unwrap_or_else(|| Path::new("."));
        relative_path(current, &self.output_root.join(".vue2qml"))
    }
}

fn relative_path(from: &Path, to: &Path) -> String {
    let from = normalize_components(from);
    let to = normalize_components(to);
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &to[common..] {
        result.push(component);
    }
    let value = result.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        ".".to_owned()
    } else {
        value
    }
}

fn normalize_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            std::path::Component::ParentDir => Some("..".to_owned()),
            _ => None,
        })
        .collect()
}
