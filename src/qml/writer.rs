use std::fmt::{self, Write};

pub(crate) struct Writer {
    output: String,
    indent: usize,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn line(&mut self, value: impl AsRef<str>) {
        if value.as_ref().is_empty() {
            self.output.push('\n');
            return;
        }
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(value.as_ref());
        self.output.push('\n');
    }

    pub fn formatted_line(&mut self, arguments: fmt::Arguments<'_>) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output
            .write_fmt(arguments)
            .expect("writing into a String cannot fail");
        self.output.push('\n');
    }

    pub fn open(&mut self, header: impl AsRef<str>) {
        self.line(format!("{} {{", header.as_ref()));
        self.indent += 1;
    }

    pub fn close(&mut self) {
        self.indent = self.indent.saturating_sub(1);
        self.line("}");
    }

    pub fn finish(self) -> String {
        self.output
    }
}
