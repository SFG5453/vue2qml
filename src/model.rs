use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub attributes: BTreeMap<String, Option<String>>,
    pub content: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sfc {
    pub template: Option<Block>,
    pub script: Option<Block>,
    pub script_setup: Option<Block>,
    pub styles: Vec<Block>,
    pub custom_blocks: Vec<(String, Block)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateNode {
    Element(Element),
    Text(String),
    Comment(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Element {
    pub tag: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<TemplateNode>,
    pub self_closing: bool,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Template {
    pub children: Vec<TemplateNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyType {
    Bool,
    Int,
    Real,
    String,
    Url,
    Var,
}

impl PropertyType {
    pub const fn qml_name(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Real => "real",
            Self::String => "string",
            Self::Url => "url",
            Self::Var => "var",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentProperty {
    pub name: String,
    pub property_type: PropertyType,
    pub required: bool,
    pub default_value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentImport {
    pub local_name: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScriptModel {
    pub name: Option<String>,
    pub properties: Vec<ComponentProperty>,
    pub component_imports: Vec<ComponentImport>,
    pub declarations: Vec<String>,
    pub spreads_app: bool,
}
