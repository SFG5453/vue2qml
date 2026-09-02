mod props;
mod scanner;

use std::collections::BTreeSet;

use crate::model::{ComponentImport, ScriptModel, Sfc};

pub(crate) fn analyze(sfc: &Sfc) -> ScriptModel {
    let mut model = ScriptModel::default();
    for block in [sfc.script.as_ref(), sfc.script_setup.as_ref()]
        .into_iter()
        .flatten()
    {
        let source = block.content.as_str();
        if model.name.is_none() {
            model.name = scanner::string_property(source, "name");
        }
        model.component_imports.extend(component_imports(source));
        model.declarations.extend(scanner::declarations(source));
        model.spreads_app |= source.contains("...props.app");
    }
    let prop_source = sfc
        .script_setup
        .as_ref()
        .and_then(|block| scanner::call_object(&block.content, "defineProps"))
        .or_else(|| {
            sfc.script
                .as_ref()
                .and_then(|block| scanner::property_object(&block.content, "props"))
        });
    if let Some(source) = prop_source {
        model.properties = props::parse(source);
    }
    deduplicate(&mut model);
    model
}

fn component_imports(source: &str) -> Vec<ComponentImport> {
    let mut imports = Vec::new();
    for statement in scanner::import_statements(source) {
        let Some((binding, path)) = scanner::parse_import(statement) else {
            continue;
        };
        if !path.ends_with(".vue") || binding.starts_with('{') || binding.starts_with('*') {
            continue;
        }
        let local_name = binding.split(',').next().map(str::trim).unwrap_or_default();
        if scanner::is_identifier(local_name) {
            imports.push(ComponentImport {
                local_name: local_name.to_owned(),
                source: path.to_owned(),
            });
        }
    }
    imports
}

fn deduplicate(model: &mut ScriptModel) {
    let mut seen_imports = BTreeSet::new();
    model
        .component_imports
        .retain(|import| seen_imports.insert(import.local_name.clone()));
    let mut seen_declarations = BTreeSet::new();
    model
        .declarations
        .retain(|name| seen_declarations.insert(name.clone()));
}

#[cfg(test)]
mod tests {
    use crate::parse_sfc;

    use super::analyze;

    #[test]
    fn finds_setup_props_and_vue_component_imports() {
        let sfc = parse_sfc(
            r#"<script setup>
               import Card from './Card.vue';
               const props = defineProps({ active: { type: Boolean, default: true } });
               </script><template><Card /></template>"#,
        )
        .expect("valid SFC");
        let model = analyze(&sfc);
        assert_eq!(model.component_imports[0].local_name, "Card");
        assert_eq!(model.properties[0].name, "active");
    }
}
