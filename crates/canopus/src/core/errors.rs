// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ValidationDiagnosticKind {
    Syntax,
    DanglingGlobPattern,
}

impl Display for ValidationDiagnosticKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationDiagnosticKind::Syntax => write!(f, "codeowners-syntax"),
            ValidationDiagnosticKind::DanglingGlobPattern => write!(f, "dangling-glob-pattern"),
        }
    }
}

#[derive(Debug)]
pub struct ValidationDiagnostic {
    pub line: usize,
    pub context: String,
    pub kind: ValidationDiagnosticKind,
}

impl ValidationDiagnostic {
    pub fn new_syntax_issue(line: usize, context: &str) -> Self {
        Self {
            line: line + 1,
            context: context.to_string(),
            kind: ValidationDiagnosticKind::Syntax,
        }
    }

    pub fn new_dangling_glob_issue(line: usize, context: &str) -> Self {
        Self {
            line: line + 1,
            context: context.to_string(),
            kind: ValidationDiagnosticKind::DanglingGlobPattern,
        }
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

#[derive(Debug)]
pub struct CodeownersValidationError {
    pub diagnostics: Vec<ValidationDiagnostic>,
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
