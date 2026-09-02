use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, ErrorKind, Result};
use crate::qml::{self, ComponentRegistry, RegisteredComponent};
use crate::{parse_sfc, script};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConverterOptions {
    pub check_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReport {
    pub source: PathBuf,
    pub output: PathBuf,
    pub output_bytes: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConversionReport {
    pub files: Vec<FileReport>,
    pub runtime_written: bool,
}

impl ConversionReport {
    #[must_use]
    pub fn converted_files(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn output_bytes(&self) -> usize {
        self.files.iter().map(|file| file.output_bytes).sum()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Converter {
    options: ConverterOptions,
}

impl Converter {
    #[must_use]
    pub const fn new(options: ConverterOptions) -> Self {
        Self { options }
    }

    pub fn convert(
        &self,
        input: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Result<ConversionReport> {
        let input = input.as_ref();
        let output = output.as_ref();
        let metadata = fs::metadata(input).map_err(|error| Error::io(error, input))?;
        if metadata.is_file() {
            self.convert_file(input, output)
        } else if metadata.is_dir() {
            self.convert_directory(input, output)
        } else {
            Err(Error::new(
                ErrorKind::InvalidArguments,
                "input must be a Vue file or project directory",
            )
            .at(input))
        }
    }

    fn convert_file(&self, input: &Path, output: &Path) -> Result<ConversionReport> {
        if input.extension().and_then(|value| value.to_str()) != Some("vue") {
            return Err(Error::new(
                ErrorKind::InvalidArguments,
                "single-file input must have a .vue extension",
            )
            .at(input));
        }
        let output_file = if output.extension().and_then(|value| value.to_str()) == Some("qml") {
            output.to_path_buf()
        } else {
            output
                .join(input.file_stem().unwrap_or_default())
                .with_extension("qml")
        };
        let output_root = output_file.parent().unwrap_or_else(|| Path::new("."));
        let source = read_source(input)?;
        let registry = registry_for_file(input, &output_file, &source)?;
        let generated =
            qml::convert_project_component(&source, input, &output_file, output_root, &registry)
                .map_err(|error| error.at(input))?;
        if !self.options.check_only {
            qml::write_runtime(output_root)?;
            write_output(&output_file, &generated)?;
        }
        Ok(ConversionReport {
            files: vec![FileReport {
                source: input.to_path_buf(),
                output: output_file,
                output_bytes: generated.len(),
            }],
            runtime_written: !self.options.check_only,
        })
    }

    fn convert_directory(&self, input: &Path, output: &Path) -> Result<ConversionReport> {
        let mut source_files = Vec::new();
        collect_vue_files(input, &mut source_files)?;
        source_files.sort();
        if source_files.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidArguments,
                "input directory does not contain any .vue files",
            )
            .at(input));
        }
        let mut sources = BTreeMap::new();
        let mut registry = ComponentRegistry::new();
        for source_path in &source_files {
            let source = read_source(source_path)?;
            let relative = source_path.strip_prefix(input).map_err(|_| {
                Error::new(ErrorKind::InvalidOutput, "source escaped the input root")
                    .at(source_path)
            })?;
            let output_path = output.join(relative).with_extension("qml");
            register_component(&mut registry, source_path, &output_path, &source)?;
            sources.insert(source_path.clone(), (source, output_path));
        }
        if !self.options.check_only {
            qml::write_runtime(output)?;
        }
        let mut report = ConversionReport {
            files: Vec::with_capacity(sources.len()),
            runtime_written: !self.options.check_only,
        };
        for (source_path, (source, output_path)) in sources {
            let generated = qml::convert_project_component(
                &source,
                &source_path,
                &output_path,
                output,
                &registry,
            )
            .map_err(|error| error.at(&source_path))?;
            if !self.options.check_only {
                write_output(&output_path, &generated)?;
            }
            report.files.push(FileReport {
                source: source_path,
                output: output_path,
                output_bytes: generated.len(),
            });
        }
        Ok(report)
    }
}

fn registry_for_file(input: &Path, output: &Path, source: &str) -> Result<ComponentRegistry> {
    let mut registry = ComponentRegistry::new();
    register_component(&mut registry, input, output, source)?;
    Ok(registry)
}

fn register_component(
    registry: &mut ComponentRegistry,
    source_path: &Path,
    output_path: &Path,
    source: &str,
) -> Result<()> {
    let sfc = parse_sfc(source).map_err(|error| error.at(source_path))?;
    let script = script::analyze(&sfc);
    let name = script
        .name
        .or_else(|| {
            source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| {
            Error::new(ErrorKind::InvalidScript, "component has no usable name").at(source_path)
        })?;
    let registered = RegisteredComponent {
        source_path: source_path.to_path_buf(),
        output_path: output_path.to_path_buf(),
        properties: script.properties,
    };
    if let Some(existing) = registry.insert(name.clone(), registered) {
        return Err(Error::new(
            ErrorKind::InvalidScript,
            format!(
                "component name {name:?} is also provided by {}",
                existing.source_path.display()
            ),
        )
        .at(source_path));
    }
    Ok(())
}

fn collect_vue_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(directory).map_err(|error| Error::io(error, directory))?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::io(error, directory))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| Error::io(error, &path))?;
        if file_type.is_dir() {
            if !excluded_directory(&entry.file_name().to_string_lossy()) {
                collect_vue_files(&path, output)?;
            }
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("vue")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn excluded_directory(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "artifacts" | ".vue2qml"
    )
}

fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| Error::io(error, path))
}

fn write_output(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| Error::io(error, parent))?;
    }
    fs::write(path, content).map_err(|error| Error::io(error, path))
}
