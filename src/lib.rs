//! Vue single-file component to QML conversion.

mod error;
mod expression;
mod model;
mod project;
mod qml;
mod script;
mod sfc;
mod template;
mod validation;

pub use error::{Error, ErrorKind, Result};
pub use project::{ConversionReport, Converter, ConverterOptions, FileReport};
pub use qml::convert_component;
pub use sfc::parse_sfc;
pub use template::parse_template;
pub use validation::{QmlValidationReport, validate_qml_tree};
