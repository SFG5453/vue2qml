use std::collections::BTreeMap;

use super::RegisteredComponent;

pub(crate) type ComponentRegistry = BTreeMap<String, RegisteredComponent>;
