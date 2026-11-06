// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFG.hs (e11f964): `buildGraph`, the
// `CFM` graph-building monad, `build`, `handleCommand`, `buildAssignment`,
// `tokenToParts` and the graph post-processing (`removeUnnecessary-
// StructuralNodes`, `renumberGraph`, `renumberTopologically`).
//
// DIVERGENCE: the `RWS CFContext CFW Int` monad is a `Builder` struct: the
// Reader is `ctx` (saved and restored around `local`), the State is the
// next node number, and the Writer pieces are appended to vectors in the
// same order `tell` emits them. The `[(Id, Node)]` association list is not
// materialised (see `Association`).

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use shellcheck_core::ast::{AssignmentMode, Ast, CaseType, ConditionType, Id, Token};
use shellcheck_core::astlib::*;
use shellcheck_core::data::{FLAGS_FOR_MAPFILE, FLAGS_FOR_READ};
use shellcheck_core::prelude::please_report;
use shellcheck_core::regex::{match_regex, mk_regex};

use super::CFEdge::*;
use super::CFEffect::*;
use super::CFNode::*;
use super::CFStringPart::*;
use super::CFValue::*;
use super::CFVariableProp::*;
use super::fgl;
use super::{
    CFEdge, CFEffect, CFNode, CFStringPart, CFValue, CfgParameters, CfgResult, IdTagged, NO_RANGE, Node, PropSet,
    Scope,
};

/// `data Range = Range Node Node`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Range(Node, Node);

/// `nodeToRange`
fn node_to_range(n: Node) -> Range {
    Range(n, n)
}

/// `spanRange`: like linkRange but without actually linking
fn span_range(a: Range, b: Range) -> Range {
    Range(a.0, b.1)
}

/// `data CFContext`
// `cfLoopStack` is never read or written beyond its initial value and is
// not ported.
#[derive(Clone, Copy)]
struct CFContext {
    /// Set by `asCondition` but never read, as in Haskell.
    #[allow(dead_code)]
    cf_is_condition: bool,
    cf_is_function: bool,
    /// `cfTokenStack`, as an index into `Association::cells`.
    cf_token_stack: u32,
    cf_exit_target: Option<Node>,
    cf_return_target: Option<Node>,
    cf_parameters: CfgParameters,
}

const NO_CELL: u32 = u32::MAX;

/// The `[(Id, Node)]` association of `CFW`: every node created while a
/// token was on the `cfTokenStack` belongs to that token.
// DIVERGENCE: instead of one `(Id, Node)` pair per stack element per node,
// each node records the top of an immutable linked token stack. The
// `cfIdToNodes` map is built from these stacks in one pass, the first time
// `nodes_for` is called (Haskell's map is lazy, too).
#[derive(Clone, Debug, Default)]
pub(crate) struct Association {
    /// `(token, rest of the stack)`
    cells: Vec<(Id, u32)>,
    /// Top of the token stack per original (pre-collapse) node.
    node_cell: Vec<u32>,
    /// `remapHelper recursiveRemapping` per original node.
    remap: Vec<Node>,
    /// `cfIdToNodes`, built on first use; each list is sorted and
    /// deduplicated (a compact `S.Set Node`).
    id_to_nodes: OnceCell<HashMap<Id, Vec<Node>>>,
}

impl Association {
    /// `M.lookup id $ M.fromListWith S.union $ map (\(id, n) -> (id, S.singleton n)) association`
    pub(crate) fn nodes_for(&self, id: Id) -> Option<BTreeSet<Node>> {
        self.id_to_nodes.get_or_init(|| self.build_index()).get(&id).map(|ns| ns.iter().copied().collect())
    }

    fn build_index(&self) -> HashMap<Id, Vec<Node>> {
        let mut out: HashMap<Id, Vec<Node>> = HashMap::new();
        for (n, &top) in self.node_cell.iter().enumerate() {
            let node = self.remap[n];
            let mut c = top;
            while c != NO_CELL {
                let (t, rest) = self.cells[c as usize];
                out.entry(t).or_default().push(node);
                c = rest;
            }
        }
        for ns in out.values_mut() {
            ns.sort_unstable();
            ns.dedup();
        }
        out
    }
}

/// The graph-building monad `CFM`.
struct Builder<'a> {
    ast: &'a Ast,
    ctx: CFContext,
    /// Node labels; node `n` is `labels[n]` (the State counts up from 0).
    labels: Vec<CFNode>,
    edges: Vec<(Node, Node, CFEdge)>,
    /// `[(Id, (Node, Node))]`, in `tell` order.
    mapping: Vec<(Id, (Node, Node))>,
    assoc: Association,
}

impl<'a> Builder<'a> {
    fn local<T>(&mut self, f: impl FnOnce(&mut CFContext), p: impl FnOnce(&mut Self) -> T) -> T {
        let saved = self.ctx;
        f(&mut self.ctx);
        let r = p(self);
        self.ctx = saved;
        r
    }

    /// `newNode`
    fn new_node(&mut self, label: CFNode) -> Node {
        let n = self.labels.len() as Node;
        self.labels.push(label);
        self.assoc.node_cell.push(self.ctx.cf_token_stack);
        n
    }

    /// `newNodeRange`
    fn new_node_range(&mut self, label: CFNode) -> Range {
        node_to_range(self.new_node(label))
    }

    /// `newStructuralNode`, also `none`
    fn none(&mut self) -> Range {
        self.new_node_range(CFStructuralNode)
    }

    /// Build a disjoint piece of the graph and return a CFExecuteSubshell. The Id is used purely for debug naming.
    fn subshell(&mut self, id: Id, reason: &str, p: impl FnOnce(&mut Self) -> Range) -> Range {
        let start = self.new_node(CFEntryPoint(format!("Subshell Id {}: {}", id.0, reason)));
        let end = self.new_node(CFStructuralNode);
        let middle = self.local(
            |c| {
                c.cf_exit_target = Some(end);
                c.cf_return_target = Some(end);
            },
            p,
        );
        self.link_ranges(&[node_to_range(start), middle, node_to_range(end)]);
        self.new_node_range(CFExecuteSubshell(reason.to_string(), start, end))
    }

    /// `withFunctionScope`
    fn with_function_scope(&mut self, p: impl FnOnce(&mut Self) -> Range) -> Range {
        let end = self.new_node(CFStructuralNode);
        let body = self.local(
            |c| {
                c.cf_return_target = Some(end);
                c.cf_is_function = true;
            },
            p,
        );
        self.link_ranges(&[body, node_to_range(end)])
    }

    /// Anything that happens recursively in f will be attributed to this id
    fn under<T>(&mut self, id: Id, f: impl FnOnce(&mut Self) -> T) -> T {
        let cell = self.assoc.cells.len() as u32;
        self.assoc.cells.push((id, self.ctx.cf_token_stack));
        self.local(|c| c.cf_token_stack = cell, f)
    }

    /// `link`
    fn link(&mut self, from: Node, to: Node, label: CFEdge) {
        self.edges.push((from, to, label));
    }

    /// `registerNode`
    fn register_node(&mut self, id: Id, r: Range) {
        self.mapping.push((id, (r.0, r.1)));
    }

    /// `linkRange`
    fn link_range(&mut self, a: Range, b: Range) -> Range {
        self.link_range_as(CFEFlow, a, b)
    }

    /// `linkRangeAs`
    fn link_range_as(&mut self, label: CFEdge, a: Range, b: Range) -> Range {
        self.link(a.1, b.0, label);
        Range(a.0, b.1)
    }

    /// `linkRanges`
    fn link_ranges(&mut self, list: &[Range]) -> Range {
        let (first, rest) = list.split_first().unwrap_or_else(|| panic!("Empty range"));
        rest.iter().fold(*first, |acc, r| self.link_range(acc, *r))
    }

    /// `sequentially`
    fn sequentially(&mut self, list: &[Id]) -> Range {
        let first = self.none();
        let mut ranges = Vec::with_capacity(list.len() + 1);
        ranges.push(first);
        for &t in list {
            ranges.push(self.build(t));
        }
        self.link_ranges(&ranges)
    }

    /// `asCondition`
    fn as_condition(&mut self, p: impl FnOnce(&mut Self) -> Range) -> Range {
        self.local(|c| c.cf_is_condition = true, p)
    }

    /// `buildRoot`
    fn build_root(&mut self, t: Id) -> Range {
        self.under(t, |b| {
            let entry = b.new_node_range(CFEntryPoint("MAIN".to_string()));
            let implied_exit = b.new_node(CFImpliedExit);
            let end = b.new_node(CFStructuralNode);
            let start = b.local(
                |c| {
                    c.cf_exit_target = Some(end);
                    c.cf_return_target = Some(implied_exit);
                },
                |b| b.build(t),
            );
            let range = b.link_ranges(&[entry, start, node_to_range(implied_exit), node_to_range(end)]);
            b.register_node(t, range);
            range
        })
    }

    /// Build the CFG.
    fn build(&mut self, t: Id) -> Range {
        let range = self.under(t, |b| b.build_(t));
        self.register_node(t, range);
        range
    }

    fn build_(&mut self, t: Id) -> Range {
        let ast = self.ast;
        let id = t;
        match ast.get(t) {
            Token::T_Annotation(_, list) => self.build(*list),
            Token::T_Script(_, list) => self.sequentially(list),

            Token::TA_Assignment(op, lhs, rhs) if matches!(ast.get(*lhs), Token::TA_Variable(..)) => {
                let Token::TA_Variable(name, indices) = ast.get(*lhs) else { unreachable!() };
                // value first: (( var[x=1] = (x=2) )) runs x=1 last
                let value = self.build(*rhs);
                let subscript = self.sequentially(indices);
                let read = if op == "=" {
                    self.none()
                } else {
                    // This is += or something
                    self.new_node_range(apply_single(IdTagged(id, CFReadVariable(name.clone()))))
                };
                let v = if indices.is_empty() { CFValueInteger } else { CFValueArray };
                let write = self.new_node_range(apply_single(IdTagged(id, CFWriteVariable(name.clone(), v))));
                self.link_ranges(&[value, subscript, read, write])
            }

            // This is likely an invalid assignment like (( 1 = 2 )), but it
            // could be e.g. x=y; (( $x = 3 )); echo $y, so expand both sides
            // without updating anything
            Token::TA_Assignment(_, lhs, rhs) => self.sequentially(&[*lhs, *rhs]),

            Token::TA_Binary(_, a, b) => self.sequentially(&[*a, *b]),
            Token::TA_Expansion(list) => self.sequentially(list),
            Token::TA_Sequence(list) => self.sequentially(list),
            Token::TA_Parenthesis(t) => self.build(*t),

            Token::TA_Trinary(cond, a, b) => {
                let condition = self.build(*cond);
                let ifthen = self.build(*a);
                let elsethen = self.build(*b);
                let end = self.none();
                self.link_ranges(&[condition, ifthen, end]);
                self.link_ranges(&[condition, elsethen, end])
            }

            Token::TA_Variable(name, indices) => {
                let subscript = self.sequentially(indices);
                let hint = if indices.is_empty() {
                    self.none()
                } else {
                    node_to_range(self.new_node(apply_single(IdTagged(id, CFHintArray(name.clone())))))
                };
                let read = node_to_range(self.new_node(apply_single(IdTagged(id, CFReadVariable(name.clone())))));
                self.link_ranges(&[subscript, hint, read])
            }

            Token::TA_Unary(op, arg)
                if matches!(ast.get(*arg), Token::TA_Variable(..)) && (op.contains("--") || op.contains("++")) =>
            {
                let Token::TA_Variable(name, indices) = ast.get(*arg) else { unreachable!() };
                let subscript = self.sequentially(indices);
                let read = self.new_node_range(apply_single(IdTagged(id, CFReadVariable(name.clone()))));
                let v = if indices.is_empty() { CFValueInteger } else { CFValueArray };
                let write = self.new_node_range(apply_single(IdTagged(id, CFWriteVariable(name.clone(), v))));
                self.link_ranges(&[subscript, read, write])
            }
            Token::TA_Unary(_, arg) => self.build(*arg),

            Token::TC_And(ConditionType::SingleBracket, _, lhs, rhs) => self.sequentially(&[*lhs, *rhs]),

            Token::TC_And(ConditionType::DoubleBracket, _, lhs, rhs) => {
                let left = self.build(*lhs);
                let right = self.build(*rhs);
                let end = self.none();
                // complete
                self.link_ranges(&[left, right, end]);
                // short circuit
                self.link_range(left, end)
            }

            // TODO: Handle integer ops
            Token::TC_Binary(_, _, lhs, rhs) => {
                let left = self.build(*lhs);
                let right = self.build(*rhs);
                self.link_range(left, right)
            }

            Token::TC_Empty(..) => self.none(),

            Token::TC_Group(_, t) => self.build(*t),

            // TODO: Mark as checked
            Token::TC_Nullary(_, arg) => self.build(*arg),

            Token::TC_Or(ConditionType::SingleBracket, _, lhs, rhs) => self.sequentially(&[*lhs, *rhs]),

            Token::TC_Or(ConditionType::DoubleBracket, _, lhs, rhs) => {
                let left = self.build(*lhs);
                let right = self.build(*rhs);
                let end = self.none();
                // complete
                self.link_ranges(&[left, right, end]);
                // short circuit
                self.link_range(left, end)
            }

            // TODO: Handle -v, -z, -n
            Token::TC_Unary(_, _, arg) => self.build(*arg),

            Token::T_Arithmetic(root) => {
                let exe = self.build(*root);
                let status = self.new_node_range(CFSetExitCode(id));
                self.link_range(exe, status)
            }

            Token::T_AndIf(lhs, rhs) | Token::T_OrIf(lhs, rhs) => {
                let left = self.build(*lhs);
                let right = self.build(*rhs);
                let end = self.none();
                self.link_range(left, right);
                self.link_range(right, end);
                self.link_range(left, end)
            }

            Token::T_Array(list) => self.sequentially(list),

            Token::T_Assignment(..) => self.build_assignment(None, t),

            Token::T_Backgrounded(body) => {
                let body = *body;
                let start = self.none();
                let fork = self.subshell(id, "backgrounding '&'", |b| b.build(body));
                let pid = self.new_node_range(CFSetBackgroundPid(id));
                let status = self.new_node_range(CFSetExitCode(id));

                self.link_range(start, fork);
                // Add a join from the fork to warn about variable changes
                self.link_range_as(CFEFalseFlow, fork, pid);
                self.link_ranges(&[start, pid, status])
            }

            Token::T_Backticked(body) => self.subshell(id, "`..` expansion", |b| b.sequentially(body)),

            Token::T_Banged(cmd) => {
                let main = self.build(*cmd);
                let status = self.new_node_range(CFSetExitCode(id));
                self.link_range(main, status)
            }

            Token::T_BatsTest(_, body) => {
                // These are technically set by the 'run' command, but we'll just define them
                // up front to avoid figuring out which commands named "run" belong to Bats.
                let status = self.new_node_range(apply_single(IdTagged(
                    id,
                    CFWriteVariable("status".to_string(), CFValueInteger),
                )));
                let output = self.new_node_range(apply_single(IdTagged(
                    id,
                    CFWriteVariable("output".to_string(), CFValueString),
                )));
                let main = self.build(*body);
                self.link_ranges(&[status, output, main])
            }

            Token::T_BraceExpansion(list) => self.sequentially(list),

            Token::T_BraceGroup(body) => self.sequentially(body),

            Token::T_CaseExpression(t, list) if list.is_empty() => self.build(*t),

            Token::T_CaseExpression(t, list) => self.build_case(*t, list),

            Token::T_Condition(_, _, op) => {
                let cond = self.build(*op);
                let status = self.new_node_range(CFSetExitCode(id));
                self.link_range(cond, status)
            }

            Token::T_CoProc(maybe_name_token, t) => {
                let t = *t;
                // If unspecified, "COPROC". If not a constant string, Nothing.
                let maybe_name = match maybe_name_token {
                    Some(x) => get_literal_string(ast, *x),
                    None => Some("COPROC".to_string()),
                };

                let parent_node = match maybe_name {
                    Some(str) => apply_single(IdTagged(id, CFWriteVariable(str, CFValueArray))),
                    None => CFStructuralNode,
                };

                let start = self.none();
                let parent = self.new_node_range(parent_node);
                let child = self.subshell(id, "coproc", |b| b.build(t));
                let end = self.new_node_range(CFSetExitCode(id));

                self.link_range(start, parent);
                self.link_range(start, child);
                self.link_range(parent, end);
                self.link_range_as(CFEFalseFlow, child, end);

                span_range(start, end)
            }
            Token::T_CoProcBody(t) => self.build(*t),

            Token::T_DollarArithmetic(arith) => self.build(*arith),
            Token::T_DollarDoubleQuoted(list) => self.sequentially(list),
            Token::T_DollarSingleQuoted(_) => self.none(),
            Token::T_DollarBracket(t) => self.build(*t),

            Token::T_DollarBraced(_, t) => {
                let t = *t;
                let str = oversimplify(ast, t).concat();
                let modifier = get_braced_modifier(&str);
                let reference = get_braced_reference(&str);
                let indices = get_index_references(&str);
                let offsets = get_offset_references(&str);
                let vals = self.build(t);
                let mut deps = vec![vals];
                for x in indices.into_iter().chain(offsets) {
                    deps.push(node_to_range(self.new_node(apply_single(IdTagged(id, CFReadVariable(x))))));
                }
                let deps = self.link_ranges(&deps);
                let read =
                    node_to_range(self.new_node(apply_single(IdTagged(id, CFReadVariable(reference.clone())))));
                let total_read = self.link_range(deps, read);

                if ["=", ":="].iter().any(|p| modifier.starts_with(p)) {
                    let optional_assign =
                        self.new_node_range(apply_single(IdTagged(id, CFWriteVariable(reference, CFValueString))));
                    let result = self.none();
                    self.link_range(optional_assign, result);
                    self.link_range(total_read, result)
                } else {
                    total_read
                }
            }

            Token::T_DollarBraceCommandExpansion(_, body) => self.sequentially(body),

            Token::T_DoubleQuoted(list) => self.sequentially(list),

            Token::T_DollarExpansion(body) => self.subshell(id, "$(..) expansion", |b| b.sequentially(body)),

            Token::T_Extglob(_, list) => self.sequentially(list),

            Token::T_FdRedirect(name, op) if name.starts_with('{') => {
                let name: String = name[1..].chars().take_while(|&c| c != '}').collect();
                let expression = self.build(*op);
                let rw = self.new_node_range(if is_closing_file_op(ast, *op) {
                    apply_single(IdTagged(id, CFReadVariable(name)))
                } else {
                    apply_single(IdTagged(id, CFWriteVariable(name, CFValueInteger)))
                });
                self.link_range(expression, rw)
            }

            Token::T_FdRedirect(_, t) => self.build(*t),

            Token::T_ForArithmetic(init_t, cond_t, inc_t, body_t) => {
                let init = self.build(*init_t);
                let cond = self.build(*cond_t);
                let body = self.sequentially(body_t);
                let inc = self.build(*inc_t);
                let end = self.none();

                // Forward edges
                self.link_ranges(&[init, cond, body, inc]);
                self.link_range(cond, end);
                // Backward edge
                self.link_range(inc, cond);
                span_range(init, end)
            }

            Token::T_ForIn(name, words, body) => self.for_in_helper(id, name, words, body),

            // For functions we generate an unlinked subgraph, and mention that in its definition node
            Token::T_Function(_, _, name, body) => {
                let body = *body;
                let range = self.local(
                    |c| c.cf_exit_target = None,
                    |b| {
                        let entry = b.new_node_range(CFEntryPoint(format!("function {name}")));
                        let f = b.with_function_scope(|b| b.build(body));
                        b.link_range(entry, f)
                    },
                );
                let Range(entry, exit) = range;
                let definition =
                    self.new_node_range(apply_single(IdTagged(id, CFDefineFunction(name.clone(), id, entry, exit))));
                let exe = self.new_node_range(CFSetExitCode(id));
                self.link_range(definition, exe)
            }

            Token::T_Glob(..) => self.none(),

            Token::T_HereString(t) => self.build(*t),
            Token::T_HereDoc(_, _, _, list) => self.sequentially(list),

            Token::T_IfExpression(ifs, elses) => {
                let start = self.none();
                let branches = self.do_branches(id, start, ifs, elses);
                let end = self.none();
                for b in branches {
                    self.link_range(b, end);
                }
                span_range(start, end)
            }

            Token::T_Include(t) => self.build(*t),

            Token::T_IndexedElement(indices_t, value_t) => {
                let indices = self.sequentially(indices_t);
                let value = self.build(*value_t);
                self.link_range(indices, value)
            }

            Token::T_IoDuplicate(op, _) => self.build(*op),

            Token::T_IoFile(op, t) => {
                let exp = self.build(*t);
                let doesnt_do_much = self.build(*op);
                self.link_range(exp, doesnt_do_much)
            }

            Token::T_Literal(..) => self.none(),

            Token::T_NormalWord(list) => self.sequentially(list),

            Token::T_Pipeline(_, cmds) if cmds.len() == 1 => self.build(cmds[0]),
            Token::T_Pipeline(_, cmds) => {
                let start = self.none();
                let has_lastpipe = self.ctx.cf_parameters.cf_lastpipe;
                let (leading, last) = self.build_pipe(id, has_lastpipe, cmds);
                // Ideally we'd let this exit code be that of the last command in the pipeline but ok
                let end = self.new_node_range(CFSetExitCode(id));

                for &c in &leading {
                    self.link_range(start, c);
                }
                for &c in &leading {
                    self.link_range_as(CFEFalseFlow, c, end);
                }
                let mut all = vec![start];
                all.extend(last);
                all.push(end);
                self.link_ranges(&all)
            }

            Token::T_ProcSub(op, cmds) => {
                let start = self.none();
                let body = self.subshell(id, &format!("{op}() process substitution"), |b| b.sequentially(cmds));
                let end = self.none();

                self.link_range(start, body);
                self.link_range_as(CFEFalseFlow, body, end);
                self.link_range(start, end)
            }

            Token::T_Redirecting(redirs, cmd) => {
                // For simple commands, this is the other way around in bash
                // We do it in this order for comound commands like { x=name; } > "$x"
                let redir = self.sequentially(redirs);
                let body = self.build(*cmd);
                self.link_range(redir, body)
            }

            Token::T_SelectIn(name, words, body) => self.for_in_helper(id, name, words, body),

            Token::T_SimpleCommand(vars, cmds) if cmds.is_empty() => {
                // Vars can also be empty, as in the command "> foo"
                let assignments = self.sequentially(vars);
                let status = self.new_node_range(CFSetExitCode(id));
                self.link_range(assignments, status)
            }

            Token::T_SimpleCommand(vars, args) => {
                let literal = get_unquoted_literal(ast, args[0]);
                self.handle_command(t, vars, args, literal)
            }

            Token::T_SingleQuoted(_) => self.none(),

            Token::T_SourceCommand(original_command, inlined_source) => {
                let cmd = self.build(*original_command);
                let end = self.none();
                // `withReturn end` is the identity.
                let inline = self.build(*inlined_source);
                self.link_range(cmd, inline);
                self.link_range(inline, end);
                span_range(cmd, inline)
            }

            Token::T_Subshell(body) => {
                let main = self.subshell(id, "explicit (..) subshell", |b| b.sequentially(body));
                let status = self.new_node_range(CFSetExitCode(id));
                self.link_range(main, status)
            }

            Token::T_UntilExpression(cond, body) | Token::T_WhileExpression(cond, body) => {
                self.while_helper(id, cond, body)
            }

            Token::T_CLOBBER
            | Token::T_GREATAND
            | Token::T_LESSAND
            | Token::T_LESSGREAT
            | Token::T_DGREAT
            | Token::T_Greater
            | Token::T_Less
            | Token::T_ParamSubSpecialChar(_) => self.none(),

            // The `error ("Unimplemented: " ++ show x)` line is STRIPped from
            // release builds, leaving `none`.
            _ => self.none(),
        }
    }

    fn build_case(&mut self, t: Id, list: &[(CaseType, Vec<Id>, Vec<Id>)]) -> Range {
        let ast = self.ast;
        let start = self.none();
        let token = self.build(t);
        let branches: Vec<(CaseType, Range, Range)> = list
            .iter()
            .map(|(typ, cond, body)| {
                // buildBranch
                let c = self.build_cond(cond);
                let b = self.sequentially(body);
                self.link_range(c, b);
                (*typ, c, b)
            })
            .collect();
        let end = self.none();

        let (_, first_cond, _) = branches[0];
        let (_, _, last_body) = *branches.last().unwrap();

        self.link_range(start, token);
        self.link_range(token, first_cond);
        for w in branches.windows(2) {
            // linkBranch
            let (typ, cond, body) = w[0];
            let (_, next_cond, next_body) = w[1];
            // Failure case
            self.link_range(cond, next_cond);
            // After body
            match typ {
                CaseType::CaseBreak => self.link_range(body, end),
                CaseType::CaseFallThrough => self.link_range(body, next_body),
                CaseType::CaseContinue => self.link_range(body, next_cond),
            };
        }
        self.link_range(last_body, end);

        // Find a *) if any
        let is_catch_all = |c: Id| {
            word_to_exact_pseudo_glob(ast, c)
                .map(|pg| pseudo_glob_is_superset_of(&pg, &[PseudoGlob::PGMany]))
                .unwrap_or(false)
        };
        let has_catch_all = |(_, cond, _): &(CaseType, Vec<Id>, Vec<Id>)| cond.iter().any(|&c| is_catch_all(c));
        if !list.iter().any(has_catch_all) {
            // There's no *) branch, so assume we can fall through
            self.link_range(token, end);
        }

        span_range(start, end)
    }

    /// for a | b | c, evaluate each in turn and allow short circuiting
    fn build_cond(&mut self, list: &[Id]) -> Range {
        let start = self.none();
        let conds: Vec<Range> = list.iter().map(|&c| self.build(c)).collect();
        let end = self.none();
        let mut all = vec![start];
        all.extend(&conds);
        self.link_ranges(&all);
        for c in conds {
            self.link_range(c, end);
        }
        span_range(start, end)
    }

    /// `doBranches` of `T_IfExpression`
    fn do_branches(&mut self, id: Id, start: Range, ifs: &[(Vec<Id>, Vec<Id>)], elses: &[Id]) -> Vec<Range> {
        let mut start = start;
        let mut result = Vec::new();
        for (conds, thens) in ifs {
            let cond = self.as_condition(|b| b.sequentially(conds));
            let action = self.sequentially(thens);
            self.link_range(start, cond);
            self.link_range(cond, action);
            start = cond;
            result.insert(0, action);
        }
        let rest = if elses.is_empty() { self.new_node_range(CFSetExitCode(id)) } else { self.sequentially(elses) };
        self.link_range(start, rest);
        result.insert(0, rest);
        result
    }

    /// `buildPipe` of `T_Pipeline`
    fn build_pipe(&mut self, id: Id, lp: bool, cmds: &[Id]) -> (Vec<Range>, Vec<Range>) {
        match cmds {
            [x] if lp => {
                let last = self.build(*x);
                (vec![], vec![last])
            }
            [first, rest @ ..] => {
                let first = *first;
                let this = self.subshell(id, "pipeline", |b| b.build(first));
                let (mut leading, last) = self.build_pipe(id, lp, rest);
                leading.insert(0, this);
                (leading, last)
            }
            [] => (vec![], vec![]),
        }
    }

    /// `forInHelper`
    fn for_in_helper(&mut self, id: Id, name: &str, words: &[Id], body: &[Id]) -> Range {
        let ast = self.ast;
        let entry = self.none();
        let expansion = self.sequentially(words);
        let assignment_choice = self.none();
        let assignments: Vec<Range> = if words.is_empty() || words.iter().any(|&w| will_split(ast, w)) {
            vec![self.new_node_range(apply_single(IdTagged(id, CFWriteVariable(name.to_string(), CFValueString))))]
        } else {
            words
                .iter()
                .map(|&t| {
                    self.new_node_range(apply_single(IdTagged(
                        id,
                        CFWriteVariable(name.to_string(), CFValueComputed(t, token_to_parts(ast, t))),
                    )))
                })
                .collect()
        };
        let body = self.sequentially(body);
        let exit = self.none();
        // Forward edges
        self.link_ranges(&[entry, expansion, assignment_choice]);
        for t in assignments {
            self.link_ranges(&[assignment_choice, t, body]);
        }
        self.link_range(body, exit);
        self.link_range(expansion, exit);
        // Backward edge
        self.link_range(body, assignment_choice);
        span_range(entry, exit)
    }

    /// `whileHelper`
    fn while_helper(&mut self, id: Id, cond: &[Id], body: &[Id]) -> Range {
        let cond_range = self.as_condition(|b| b.sequentially(cond));
        let body_range = self.sequentially(body);
        let end = self.new_node_range(CFSetExitCode(id));

        self.link_range(cond_range, body_range);
        self.link_range(body_range, cond_range);
        self.link_range(cond_range, end)
    }

    /// `handleCommand cmd vars args literalCmd`; `args` is non-empty.
    fn handle_command(&mut self, cmd: Id, vars: &[Id], args: &[Id], literal_cmd: Option<String>) -> Range {
        let ast = self.ast;
        // TODO: Handle assignments in declaring commands
        match literal_cmd.as_deref() {
            Some("exit") => self.regular_expansion(vars, args, |b| b.handle_exit()),
            Some("return") => self.regular_expansion(vars, args, |b| b.handle_return()),
            Some("unset") => self.regular_expansion_with_status(vars, args, |b| b.handle_unset(args)),

            Some("declare") | Some("local") | Some("typeset") => self.handle_declare(args),

            Some("printf") => self.regular_expansion_with_status(vars, args, |b| b.handle_printf(args)),
            Some("wait") => self.regular_expansion_with_status(vars, args, |b| b.handle_wait(args)),

            Some("mapfile") | Some("readarray") => {
                self.regular_expansion_with_status(vars, args, |b| b.handle_mapfile(args))
            }

            Some("read") => self.regular_expansion_with_status(vars, args, |b| b.handle_read(args)),

            Some("DEFINE_boolean") | Some("DEFINE_float") | Some("DEFINE_integer") | Some("DEFINE_string") => {
                self.regular_expansion_with_status(vars, args, |b| b.handle_define(args))
            }

            // This will mostly behave like 'command' but ok
            Some("builtin") => match args {
                [_, newcmd, ..] => {
                    let newcmd = *newcmd;
                    self.handle_command(newcmd, vars, &args[1..], get_literal_string(ast, newcmd))
                }
                _ => self.handle_others(cmd, vars, args, literal_cmd),
            },
            Some("command") => match args {
                [_, newcmd, ..] => {
                    let newcmd = *newcmd;
                    self.handle_others(newcmd, vars, &args[1..], get_literal_string(ast, newcmd))
                }
                _ => self.handle_others(cmd, vars, args, literal_cmd),
            },
            _ => self.handle_others(cmd, vars, args, literal_cmd),
        }
    }

    /// `handleExit`
    fn handle_exit(&mut self) -> Range {
        match self.ctx.cf_exit_target {
            Some(target) => {
                let exit = self.new_node(CFResolvedExit);
                self.link(exit, target, CFEExit);
                let unreachable = self.new_node(CFUnreachable);
                Range(exit, unreachable)
            }
            None => {
                let exit = self.new_node(CFUnresolvedExit);
                let unreachable = self.new_node(CFUnreachable);
                Range(exit, unreachable)
            }
        }
    }

    /// `handleReturn`
    fn handle_return(&mut self) -> Range {
        match self.ctx.cf_return_target {
            None => panic!("{}", please_report("missing return target")),
            Some(target) => {
                let ret = self.new_node(CFStructuralNode);
                self.link(ret, target, CFEFlow);
                let unreachable = self.new_node(CFUnreachable);
                Range(ret, unreachable)
            }
        }
    }

    /// `handleUnset (cmd NE.:| args)`
    fn handle_unset(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let args = &all[1..];
        // [(Flag string, token)] e.g. [("-f", t), ("", myfunc)]
        let pairs: Vec<(String, Id)> = match get_gnu_opts(ast, "vfn", args) {
            Some(opts) => opts.into_iter().map(|(str, (flag, _val))| (str, flag)).collect(),
            None => args.iter().map(|&c| (String::new(), c)).collect(),
        };
        let (names, flags): (Vec<_>, Vec<_>) = pairs.into_iter().partition(|(s, _)| s.is_empty());
        let flag_names: Vec<String> = flags.into_iter().map(|(s, _)| s).collect();
        // Literal names to unset, e.g. [(myfuncToken, "myfunc")]
        let literal_names: Vec<(Id, String)> =
            names.iter().filter_map(|&(_, t)| get_literal_string(ast, t).map(|s| (t, s))).collect();
        let has = |f: &str| flag_names.iter().any(|x| x == f);
        let c: fn(String) -> CFEffect = if has("n") {
            CFUndefineNameref
        } else if has("v") {
            CFUndefineVariable
        } else if has("f") {
            CFUndefineFunction
        } else {
            CFUndefine
        };
        // Apply a constructor like CFUndefineVariable to each literalName, and tag with its id
        self.new_node_range(CFApplyEffects(
            literal_names.into_iter().map(|(token, name)| IdTagged(token, c(name))).collect(),
        ))
    }

    /// `handleDeclare (cmd NE.:| args)`
    fn handle_declare(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let cmd = all[0];
        let args = &all[1..];
        let is_func = self.ctx.cf_is_function;

        let opts: Vec<String> = get_generic_opts(ast, args).into_iter().map(|(s, _)| s).collect();
        let has = |f: &str| opts.iter().any(|x| x == f);
        let associative = has("A");
        let array = has("a") || associative;
        let integer = has("i");
        // `func = "f" `elem` opts || "F" `elem` opts` is unused.
        let global = has("g");
        let export = has("x");
        let writer: fn(String, CFValue) -> CFEffect = if global {
            CFWriteGlobal
        } else if is_func {
            CFWriteLocal
        } else {
            CFWriteVariable
        };
        let scope = if global {
            Some(Scope::GlobalScope)
        } else if is_func {
            Some(Scope::LocalScope)
        } else {
            None
        };

        let mut added_list = vec![];
        if array {
            added_list.push(CFVPArray);
        }
        if integer {
            added_list.push(CFVPInteger);
        }
        if export {
            added_list.push(CFVPExport);
        }
        if associative {
            added_list.push(CFVPAssociative);
        }
        let added_props = PropSet::from_props(&added_list);

        // find "ia" from `define +i +a`
        let unset_options: String = args
            .iter()
            .filter_map(|&a| get_literal_string(ast, a))
            .filter(|s| s.starts_with('+'))
            .flat_map(|s| s.chars().skip(1).collect::<Vec<_>>())
            .collect();
        let mut removed_list = vec![];
        // Array property can't be unset
        if unset_options.contains('i') {
            removed_list.push(CFVPInteger);
        }
        if unset_options.contains('e') {
            removed_list.push(CFVPExport);
        }
        let removed_props = PropSet::from_props(&removed_list);

        // `mconcat $ map (toEffects isFunc) args`
        let mut evaluated: Vec<Id> = vec![];
        let mut assignments: Vec<IdTagged<CFEffect>> = vec![];
        let mut added: Vec<IdTagged<CFEffect>> = vec![];
        let mut removed: Vec<IdTagged<CFEffect>> = vec![];

        for &t in args {
            let id = t;
            match ast.get(t) {
                Token::T_Assignment(mode, var, idx, value) => {
                    evaluated.extend(idx);
                    evaluated.push(*value);
                    let mut parts = vec![];
                    if *mode == AssignmentMode::Append {
                        parts.push(CFStringVariable(var.clone()));
                    }
                    parts.extend(token_to_parts(ast, *value));
                    assignments.push(IdTagged(id, writer(var.clone(), CFValueComputed(*value, parts))));
                    if !added_props.is_empty() {
                        added.push(IdTagged(id, CFSetProps(scope, var.clone(), added_props)));
                    }
                    if !removed_props.is_empty() {
                        // Haskell passes addedProps (not removedProps) here.
                        removed.push(IdTagged(id, CFUnsetProps(scope, var.clone(), added_props)));
                    }
                }
                _ => {
                    evaluated.push(t);
                    let literal = get_literal_string_def(ast, "\0", t);
                    let is_known = !literal.contains('\0');
                    let m = match_regex(&VARIABLE_ASSIGN_REGEX, &literal).map(|g| g[0].clone());
                    let name = m.clone().unwrap_or_else(|| literal.clone());

                    let as_literal = || {
                        let value: String = match literal.find('=') {
                            Some(i) => literal[i + 1..].to_string(),
                            None => String::new(),
                        };
                        IdTagged(id, writer(name.clone(), CFValueComputed(t, vec![CFStringLiteral(value)])))
                    };
                    let as_unknown = || IdTagged(id, writer(name.clone(), CFValueString));
                    let my_added = IdTagged(id, CFSetProps(scope, name.clone(), added_props));
                    let my_removed = IdTagged(id, CFUnsetProps(scope, name.clone(), removed_props));

                    if !is_variable_name(&name) {
                        // (pre, [], [], [])
                    } else if m.is_some() && is_known {
                        assignments.push(as_literal());
                        added.push(my_added);
                        removed.push(my_removed);
                    } else if m.is_some() {
                        assignments.push(as_unknown());
                        added.push(my_added);
                        removed.push(my_removed);
                    } else {
                        // e.g. declare -i x
                        added.push(my_added);
                        removed.push(my_removed);
                    }
                }
            }
        }

        // This is a bit of a kludge: we don't have great support for things like
        // 'declare -i x=$x' so do one round with declare x=$x, followed by declare -i x
        let before = self.sequentially(&evaluated);
        let assignments = self.new_node_range(CFApplyEffects(assignments));
        let added_props = if added.is_empty() { self.none() } else { self.new_node_range(CFApplyEffects(added)) };
        let removed_props =
            if removed.is_empty() { self.none() } else { self.new_node_range(CFApplyEffects(removed)) };
        let result = self.new_node_range(CFSetExitCode(cmd));
        self.link_ranges(&[before, assignments, added_props, removed_props, result])
    }

    /// `handlePrintf (cmd NE.:| args)`
    fn handle_printf(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let args = &all[1..];
        let find_var = || {
            let flags = get_bsd_opts(ast, "v:", args)?;
            let (_flag, arg) = flags.iter().find(|(f, _)| f == "v")?.1;
            let name = get_literal_string(ast, arg)?;
            Some(IdTagged(arg, CFWriteVariable(name, CFValueString)))
        };
        self.new_node_range(CFApplyEffects(find_var().into_iter().collect()))
    }

    /// `handleWait (cmd NE.:| args)`
    fn handle_wait(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let args = &all[1..];
        let find_var = || {
            let flags = get_generic_opts(ast, args);
            let (_flag, arg) = flags.iter().find(|(f, _)| f == "p")?.1;
            let name = get_literal_string(ast, arg)?;
            Some(IdTagged(arg, CFWriteVariable(name, CFValueInteger)))
        };
        self.new_node_range(CFApplyEffects(find_var().into_iter().collect()))
    }

    /// `handleMapfile (cmd NE.:| args)`
    fn handle_mapfile(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let cmd = all[0];
        let args = &all[1..];
        let get_from_arg = || {
            let flags = get_gnu_opts(ast, FLAGS_FOR_MAPFILE, args)?;
            let (_, arg) = flags.iter().find(|(f, _)| f.is_empty())?.1;
            let name = get_literal_string(ast, arg)?;
            Some((arg, name))
        };
        let get_if_var = |c: Id| {
            let name = get_literal_string(ast, c)?;
            if !is_variable_name(&name) {
                return None;
            }
            Some((c, name))
        };
        let get_from_fallback = || args.iter().rev().find_map(|&c| get_if_var(c));
        let (id, name) =
            get_from_arg().or_else(get_from_fallback).unwrap_or_else(|| (cmd, "MAPFILE".to_string()));
        self.new_node_range(CFApplyEffects(vec![IdTagged(id, CFWriteVariable(name, CFValueArray))]))
    }

    /// `handleRead (cmd NE.:| args)`
    fn handle_read(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let cmd = all[0];
        let args = &all[1..];

        let with_array = |flags: &[Opt]| -> Option<Vec<IdTagged<CFEffect>>> {
            let (_, token) = flags.iter().find(|(f, _)| f == "a")?.1;
            Some(match get_literal_string(ast, token) {
                Some(name) => vec![IdTagged(token, CFWriteVariable(name, CFValueArray))],
                None => vec![],
            })
        };
        let with_fields = |flags: &[Opt]| -> Vec<IdTagged<CFEffect>> {
            flags
                .iter()
                .filter_map(|(f, (t, _))| {
                    if !f.is_empty() {
                        return None;
                    }
                    let name = get_literal_string(ast, *t)?;
                    Some(IdTagged(*t, CFWriteVariable(name, CFValueString)))
                })
                .collect()
        };
        let fallback = || {
            let mut names: Vec<(Id, String)> =
                args.iter().rev().map_while(|&c| get_literal_string(ast, c).map(|s| (c, s))).collect();
            names.reverse();
            let names_or_default = if names.is_empty() { vec![(cmd, "REPLY".to_string())] } else { names };
            let has_dash_a = get_generic_opts(ast, args).iter().any(|(f, _)| f == "a");
            names_or_default
                .into_iter()
                .map(|(id, name)| {
                    IdTagged(id, CFWriteVariable(name, if has_dash_a { CFValueArray } else { CFValueString }))
                })
                .collect::<Vec<_>>()
        };
        let main = match get_gnu_opts(ast, FLAGS_FOR_READ, args) {
            Some(flags) => with_array(&flags).unwrap_or_else(|| with_fields(&flags)),
            None => fallback(),
        };
        self.new_node_range(CFApplyEffects(main))
    }

    /// `handleDEFINE (cmd NE.:| args)`
    fn handle_define(&mut self, all: &[Id]) -> Range {
        let ast = self.ast;
        let args = &all[1..];
        let find_var = || {
            let name = *args.get(1)?;
            let str = get_literal_string(ast, name)?;
            if !is_variable_name(&str) {
                return None;
            }
            Some(IdTagged(name, CFWriteVariable(str, CFValueString)))
        };
        self.new_node_range(CFApplyEffects(find_var().into_iter().collect()))
    }

    /// `handleOthers id vars args cmd`
    fn handle_others(&mut self, id: Id, vars: &[Id], args: &[Id], cmd: Option<String>) -> Range {
        self.regular_expansion(vars, args, |b| {
            let exe = b.new_node_range(CFExecuteCommand(cmd));
            let status = b.new_node_range(CFSetExitCode(id));
            b.link_range(exe, status)
        })
    }

    /// `regularExpansion vars args p`
    fn regular_expansion(&mut self, vars: &[Id], args: &[Id], p: impl FnOnce(&mut Self) -> Range) -> Range {
        let args = self.sequentially(args);
        let assignments: Vec<Range> =
            vars.iter().map(|&v| self.build_assignment(Some(Scope::PrefixScope), v)).collect();
        let exe = p(self);
        let drop_assignments =
            if vars.is_empty() { None } else { Some(self.new_node_range(CFDropPrefixAssignments)) };

        let mut all = vec![args];
        all.extend(assignments);
        all.push(exe);
        all.extend(drop_assignments);
        self.link_ranges(&all)
    }

    /// `regularExpansionWithStatus vars args@(cmd NE.:| _) p`
    fn regular_expansion_with_status(
        &mut self,
        vars: &[Id],
        args: &[Id],
        p: impl FnOnce(&mut Self) -> Range,
    ) -> Range {
        let initial = self.regular_expansion(vars, args, p);
        let status = self.new_node_range(CFSetExitCode(args[0]));
        self.link_range(initial, status)
    }

    /// `buildAssignment scope t`
    fn build_assignment(&mut self, scope: Option<Scope>, t: Id) -> Range {
        let ast = self.ast;
        let Token::T_Assignment(mode, var, indices, value) = ast.get(t) else {
            panic!("{}", please_report("buildAssignment: not an assignment"))
        };
        let id = t;
        let expand = self.build(*value);
        let index = self.sequentially(indices);
        let read = match mode {
            AssignmentMode::Append => self.new_node_range(apply_single(IdTagged(id, CFReadVariable(var.clone())))),
            AssignmentMode::Assign => self.none(),
        };
        let f = |id: Id, value: Id| -> CFValue {
            match ast.get(value) {
                Token::T_NormalWord(..) | Token::T_Literal(..) => {
                    let mut parts = vec![];
                    if *mode == AssignmentMode::Append {
                        parts.push(CFStringVariable(var.clone()));
                    }
                    parts.extend(token_to_parts(ast, value));
                    CFValueComputed(id, parts)
                }
                Token::T_Array(..) => CFValueArray,
                // DIVERGENCE: Haskell's `f` is partial here and would crash
                // when the DFA forces the value; the parser never produces
                // another value token.
                _ => CFValueString,
            }
        };
        let value_type = if indices.is_empty() { f(id, *value) } else { CFValueArray };
        let scoper: fn(String, CFValue) -> CFEffect = match scope {
            Some(Scope::PrefixScope) => CFWritePrefix,
            Some(Scope::LocalScope) => CFWriteLocal,
            Some(Scope::GlobalScope) => CFWriteGlobal,
            None => CFWriteVariable,
        };
        let write = self.new_node_range(apply_single(IdTagged(id, scoper(var.clone(), value_type))));
        let op = self.link_ranges(&[expand, index, read, write]);

        self.register_node(t, op);
        op
    }
}

static VARIABLE_ASSIGN_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("^([_a-zA-Z][_a-zA-Z0-9]*)="));

/// `applySingle e = CFApplyEffects [e]`
fn apply_single(e: IdTagged<CFEffect>) -> CFNode {
    CFApplyEffects(vec![e])
}

/// `tokenToParts`
pub(crate) fn token_to_parts(ast: &Ast, t: Id) -> Vec<CFStringPart> {
    match ast.get(t) {
        Token::T_NormalWord(list) | Token::T_DoubleQuoted(list) => {
            list.iter().flat_map(|&x| token_to_parts(ast, x)).collect()
        }
        Token::T_SingleQuoted(str) | Token::T_Literal(str) => vec![CFStringLiteral(str.clone())],
        Token::T_DollarArithmetic(..) | Token::T_DollarBracket(..) => vec![CFStringInteger],
        Token::T_DollarBraced(_, list) if is_unmodified_parameter_expansion(ast, t) => {
            vec![CFStringVariable(get_braced_reference(&oversimplify(ast, *list).concat()))]
        }
        // Check if getLiteralString can handle it, if not it's unknown
        _ => vec![get_literal_string(ast, t).map_or(CFStringUnknown, CFStringLiteral)],
    }
}

/// `type CFW = ([LNode CFNode], [LEdge CFEdge], [(Id, (Node, Node))], [(Id, Node)])`
///
/// Only used by the ported `prop_` tests; `build_graph` works on a compact
/// form of the same data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Cfw {
    pub nodes: Vec<(Node, CFNode)>,
    pub edges: Vec<(Node, Node, CFEdge)>,
    pub mapping: Vec<(Id, (Node, Node))>,
    pub assoc: Vec<(Id, Node)>,
}

/// `remapNode`'s label part (`remapEffect` included)
fn remap_label(label: CFNode, f: &dyn Fn(Node) -> Node) -> CFNode {
    match label {
        CFApplyEffects(effects) => CFApplyEffects(
            effects
                .into_iter()
                .map(|IdTagged(tid, e)| match e {
                    CFDefineFunction(name, id, start, end) => {
                        IdTagged(id, CFDefineFunction(name, id, f(start), f(end)))
                    }
                    e => IdTagged(tid, e),
                })
                .collect(),
        ),
        CFExecuteSubshell(s, a, b) => CFExecuteSubshell(s, f(a), f(b)),
        l => l,
    }
}

/// `remapGraph`
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn remap_graph(remap: &BTreeMap<Node, Node>, g: Cfw) -> Cfw {
    // remapHelper map n = M.findWithDefault n n map
    let f = |n: Node| remap.get(&n).copied().unwrap_or(n);
    Cfw {
        nodes: g.nodes.into_iter().map(|(n, l)| (f(n), remap_label(l, &f))).collect(),
        edges: g.edges.into_iter().map(|(a, b, l)| (f(a), f(b), l)).collect(),
        mapping: g.mapping.into_iter().map(|(id, (a, b))| (id, (f(a), f(b)))).collect(),
        assoc: g.assoc.into_iter().map(|(id, n)| (id, f(n))).collect(),
    }
}

/// Renumber the graph for prettiness, so there are no gaps in node numbers
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn renumber_graph(g: Cfw) -> Cfw {
    let mut ns: Vec<Node> = g.nodes.iter().map(|(n, _)| *n).collect();
    ns.sort();
    let renumbering: BTreeMap<Node, Node> = ns.into_iter().zip(0..).collect();
    remap_graph(&renumbering, g)
}

/// Renumber the graph in topological order
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn renumber_topologically(g: Cfw) -> Cfw {
    let order = fgl::topsort(&g.nodes.iter().map(|(n, _)| *n).collect::<Vec<_>>(), &g.edges);
    let renumbering: BTreeMap<Node, Node> = order.into_iter().zip(0..).collect();
    remap_graph(&renumbering, g)
}

/// The core of `removeUnnecessaryStructuralNodes`: which edges collapse,
/// and the `recursiveRemapping` that results.
fn structural_collapse(
    max_node: usize,
    is_structural: &dyn Fn(Node) -> bool,
    edges: &[(Node, Node, CFEdge)],
) -> (BTreeSet<(Node, Node, CFEdge)>, BTreeMap<Node, Node>) {
    // `inDegree` counts sources and `outDegree` targets (Haskell's naming);
    // `isLinear` needs both to be 1, so the swap is harmless.
    let mut in_degree = vec![0u32; max_node + 1];
    let mut out_degree = vec![0u32; max_node + 1];
    for &(from, to, l) in edges {
        if l == CFEFlow {
            in_degree[from as usize] += 1;
            out_degree[to as usize] += 1;
        }
    }
    let is_candidate = |n: Node| is_structural(n) && in_degree[n as usize] == 1 && out_degree[n as usize] == 1;
    let edges_to_collapse: BTreeSet<(Node, Node, CFEdge)> = edges
        .iter()
        .filter(|&&(a, b, l)| l == CFEFlow && is_candidate(a) && is_candidate(b))
        .copied()
        .collect();

    // `M.fromList $ map orderEdge $ S.toList edgesToCollapse`: later keys win.
    let mut remapping: BTreeMap<Node, Node> = BTreeMap::new();
    for &(a, b, _) in &edges_to_collapse {
        let (k, v) = if a < b { (b, a) } else { (a, b) };
        remapping.insert(k, v);
    }
    // `recursiveRemapping = M.mapWithKey (\c _ -> recursiveLookup remapping c) remapping`
    //
    // DIVERGENCE (performance only): every key maps to a smaller node, so
    // walking the keys in ascending order lets each lookup reuse the result
    // for its target instead of following the chain again. Following every
    // chain afresh is quadratic in the chain length (one simple command with
    // 20000 words); Haskell avoids that only because the values are lazy.
    // Haskell loops forever on a structural self loop (which the builder
    // never creates); a self loop maps to itself here.
    let mut recursive: BTreeMap<Node, Node> = BTreeMap::new();
    for (&c, &x) in &remapping {
        let target = if x == c { c } else { recursive.get(&x).copied().unwrap_or(x) };
        recursive.insert(c, target);
    }
    (edges_to_collapse, recursive)
}

/// Collapse structural nodes that just form long chains like x->x->x.
/// This way we can generate them with abandon, without making DFA slower.
///
/// Note in particular that we can't remove a structural node x in
/// foo -> x -> bar , because then the pre/post-condition for tokens
/// previously pointing to x would be wrong.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn remove_unnecessary_structural_nodes(g: Cfw) -> Cfw {
    let max_node = g
        .nodes
        .iter()
        .map(|(n, _)| *n)
        .chain(g.edges.iter().flat_map(|(a, b, _)| [*a, *b]))
        .max()
        .unwrap_or(0) as usize;
    let structural: BTreeSet<Node> =
        g.nodes.iter().filter(|(_, l)| *l == CFStructuralNode).map(|(n, _)| *n).collect();
    let (collapse, recursive) = structural_collapse(max_node, &|n| structural.contains(&n), &g.edges);
    remap_graph(
        &recursive,
        Cfw {
            nodes: g.nodes.into_iter().filter(|(n, _)| !recursive.contains_key(n)).collect(),
            edges: g.edges.into_iter().filter(|e| !collapse.contains(e)).collect(),
            mapping: g.mapping,
            assoc: g.assoc,
        },
    )
}

/// `buildGraph :: CFGParameters -> Token -> CFGResult`
pub fn build_graph(params: &CfgParameters, ast: &Ast, root: Id) -> CfgResult {
    let mut b = Builder {
        ast,
        ctx: CFContext {
            cf_is_condition: false,
            cf_is_function: false,
            cf_token_stack: NO_CELL,
            cf_exit_target: None,
            cf_return_target: None,
            cf_parameters: *params,
        },
        labels: vec![],
        edges: vec![],
        mapping: vec![],
        assoc: Association::default(),
    };
    b.build_root(root);
    let Builder { labels, edges, mapping, mut assoc, .. } = b;

    // removeUnnecessaryStructuralNodes (renumberTopologically is commented
    // out in Haskell too)
    let count = labels.len();
    let (collapse, recursive) =
        structural_collapse(count.saturating_sub(1), &|n| labels[n as usize] == CFStructuralNode, &edges);
    let mut remap: Vec<Node> = (0..count as Node).collect();
    for (&k, &v) in &recursive {
        remap[k as usize] = v;
    }
    recursive_keys_check(&recursive, &remap);
    let f = |n: Node| remap[n as usize];
    let mut final_labels: Vec<Option<CFNode>> = Vec::with_capacity(count);
    for (n, l) in labels.into_iter().enumerate() {
        final_labels.push(if remap[n] != n as Node { None } else { Some(remap_label(l, &f)) });
    }
    let final_edges: Vec<(Node, Node, CFEdge)> =
        edges.into_iter().filter(|e| !collapse.contains(e)).map(|(a, b, l)| (f(a), f(b), l)).collect();

    // idToRange = M.fromList mapping (later entries win)
    let mut id_to_range = vec![NO_RANGE; ast.len() as usize];
    for (id, (a, bb)) in mapping {
        id_to_range[id.0 as usize] = (f(a), f(bb));
    }
    let (_, main_exit) = id_to_range[root.0 as usize];
    assoc.remap = remap;

    let graph = fgl::mk_graph(final_labels, &final_edges);
    // isRealEdge: CFEFlow and CFEExit
    let only_real_edges: Vec<(Node, Node, CFEdge)> =
        final_edges.iter().filter(|(_, _, l)| matches!(l, CFEFlow | CFEExit)).copied().collect();
    let (post_idom, post_root) = fgl::find_post_dominators(main_exit, &graph, &only_real_edges);

    CfgResult { cf_graph: graph, id_to_range, assoc, post_idom, post_root }
}

/// A removed node always maps to a smaller surviving node, so "remap[n] != n"
/// identifies exactly the removed ones.
fn recursive_keys_check(recursive: &BTreeMap<Node, Node>, remap: &[Node]) {
    debug_assert!(recursive.iter().all(|(&k, &v)| v < k && remap[v as usize] == v));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> CFNode {
        CFStructuralNode
    }

    #[test]
    fn prop_test_renumbering() {
        let before = Cfw {
            nodes: vec![(1, s()), (3, s()), (4, s()), (8, s())],
            edges: vec![(1, 3, CFEFlow), (3, 4, CFEFlow), (4, 8, CFEFlow)],
            mapping: vec![(Id(0), (3, 4))],
            assoc: vec![(Id(1), 3), (Id(2), 4)],
        };
        let after = Cfw {
            nodes: vec![(0, s()), (1, s()), (2, s()), (3, s())],
            edges: vec![(0, 1, CFEFlow), (1, 2, CFEFlow), (2, 3, CFEFlow)],
            mapping: vec![(Id(0), (1, 2))],
            assoc: vec![(Id(1), 1), (Id(2), 2)],
        };
        assert_eq!(after, renumber_graph(before));
    }

    #[test]
    fn prop_test_renumber_topologically() {
        let before = Cfw {
            nodes: vec![(4, s()), (2, s()), (3, s())],
            edges: vec![(4, 2, CFEFlow), (2, 3, CFEFlow)],
            mapping: vec![(Id(0), (4, 2))],
            assoc: vec![],
        };
        let after = Cfw {
            nodes: vec![(0, s()), (1, s()), (2, s())],
            edges: vec![(0, 1, CFEFlow), (1, 2, CFEFlow)],
            mapping: vec![(Id(0), (0, 1))],
            assoc: vec![],
        };
        assert_eq!(after, renumber_topologically(before));
    }

    #[test]
    fn prop_test_remove_structural() {
        let before = Cfw {
            nodes: vec![(1, s()), (2, s()), (3, s()), (4, s())],
            edges: vec![(1, 2, CFEFlow), (2, 3, CFEFlow), (3, 4, CFEFlow)],
            mapping: vec![(Id(0), (2, 3))],
            assoc: vec![(Id(0), 3)],
        };
        let after = Cfw {
            nodes: vec![(1, s()), (2, s()), (4, s())],
            edges: vec![(1, 2, CFEFlow), (2, 4, CFEFlow)],
            mapping: vec![(Id(0), (2, 2))],
            assoc: vec![(Id(0), 2)],
        };
        assert_eq!(after, remove_unnecessary_structural_nodes(before));
    }
}
