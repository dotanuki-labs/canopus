// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq)]
pub enum DiagnosticKind {
    InvalidSyntax,
    DanglingGlobPattern,
    DuplicateOwnership,
}

impl Display for DiagnosticKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticKind::InvalidSyntax => write!(f, "codeowners-syntax"),
            DiagnosticKind::DanglingGlobPattern => write!(f, "dangling-glob-pattern"),
            DiagnosticKind::DuplicateOwnership => write!(f, "duplicated-ownership"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationDiagnostic {
    kind: DiagnosticKind,
    line: usize,
    context: String,
}

#[derive(Default)]
pub struct ValidationDiagnosticBuilder {
    kind: Option<DiagnosticKind>,
    line: Option<usize>,
    context: Option<String>,
}

impl ValidationDiagnosticBuilder {
    pub fn kind(mut self, kind: DiagnosticKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn line_number(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn description(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());
        self
    }

    pub fn message(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    pub fn build(self) -> ValidationDiagnostic {
        ValidationDiagnostic {
            kind: self.kind.expect("missing diagnostic kind"),
            line: self.line.expect("missing related line in codeowners file"),
            context: self.context.expect("missing context for this diagnostic"),
        }
    }
}

impl ValidationDiagnostic {
    pub fn builder() -> ValidationDiagnosticBuilder {
        ValidationDiagnosticBuilder::default()
    }
}

impl Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "L{} : {} ({})", self.line, self.context, self.kind)
    }
}

impl std::error::Error for ValidationDiagnostic {
    // Already satisfied
}

impl From<ValidationDiagnostic> for CodeownersValidationError {
    fn from(value: ValidationDiagnostic) -> Self {
        CodeownersValidationError {
            diagnostics: vec![value],
        }
    }
}

impl From<CodeownersValidationError> for Vec<ValidationDiagnostic> {
    fn from(value: CodeownersValidationError) -> Self {
        value.diagnostics
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeownersValidationError {
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl CodeownersValidationError {
    pub fn with(diagnostics: Vec<ValidationDiagnostic>) -> Self {
        Self { diagnostics }
    }
}

impl Display for CodeownersValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let messages = self
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>();

        f.write_str(&messages.join("\n"))
    }
}

impl std::error::Error for CodeownersValidationError {
    // Already satisfied
}

impl From<anyhow::Result<()>> for CodeownersValidationError {
    fn from(value: anyhow::Result<()>) -> Self {
        value.expect_err("expecting an error").downcast().unwrap()
    }
}
