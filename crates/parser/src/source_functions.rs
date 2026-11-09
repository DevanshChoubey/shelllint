// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: not in ShellCheck. `source-functions=a,b` (rc file) and
// `--source-functions=a,b` (command line) name wrapper functions that are
// followed like `source ARG` when their first argument is a literal.

use shelllint_core::ast::{Annotation, Id, Token};
use shelllint_core::astlib::get_literal_string;

use crate::engine::{P, PR};

impl<'a> P<'a> {
    /// Is the command word a literal naming a configured source function?
    pub(crate) fn is_source_function(&self, literal: &Option<String>) -> bool {
        let Some(name) = literal.as_deref() else { return false };
        if self.env.system_interface.source_functions().iter().any(|f| f == name) {
            return true;
        }
        self.get_current_annotations(true).iter().any(|a| match a {
            Annotation::SourceFunctions(list) => list.iter().any(|f| f == name),
            _ => false,
        })
    }

    /// Follow the call like `source`, but only with a literal first argument
    /// (or a `source=` directive); otherwise leave it alone, silently.
    pub(crate) fn read_source_function(&mut self, t: Id) -> PR<Id> {
        let ast = &self.user.ast;
        let literal_first = match ast.get(t) {
            Token::T_Redirecting(_, sc) => match ast.get(*sc) {
                Token::T_SimpleCommand(_, words) => words.get(1).is_some_and(|w| get_literal_string(ast, *w).is_some()),
                _ => false,
            },
            _ => false,
        };
        if literal_first || self.get_source_override().is_some() { self.read_source(t) } else { Ok(t) }
    }
}
