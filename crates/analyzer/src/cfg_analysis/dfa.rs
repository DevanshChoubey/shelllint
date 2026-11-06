// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFGAnalysis.hs (e11f964): the DFA engine.
//
// Haskell runs this in plain ST with a manual `Ctx` of STRefs. Here `Ctx` is
// a struct passed as `&mut self`:
// * `cNode`/`cInput`/`cOutput` are plain fields. `withNewStackFrame` (which
//   in Haskell allocates fresh STRefs for the callee) saves them, runs the
//   callee and restores them, which is observably the same.
// * `cStack` is a Vec with the top of the stack at the end, and each entry
//   owns its `dependencies` set (an STRef in Haskell).
// * The shared STRefs (`cCounter`, `cCache`, `cEnableCache`,
//   `cInvocations`) are fields too.
//
// Maps are `Rc<BTreeMap>` with copy-on-write instead of Haskell's persistent
// `Data.Map`; iteration order, versions and merge order follow Haskell.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use shellcheck_core::ast::{Ast, Id};
use shellcheck_core::data;
use shellcheck_core::prelude::please_report;

use super::NumericalStatus::*;
use super::SpaceStatus::*;
use super::{
    CFVariableProp, CfgAnalysis, CfgParameters, NumericalStatus, ProgramState, SpaceStatus, VarMap,
    VariableProperties, VariableState, VariableValue,
};
use crate::cfg::{
    CFEdge, CFEffect, CFNode, CFStringPart, CFValue, CfGraph, IdTagged, Node, PropSet, Scope,
};

/// The number of iterations for DFA to stabilize
const ITERATION_COUNT: u32 = 1_000_000;
/// There have been multiple bugs where bad caching caused oscillations.
/// As a precaution, disable caching if there's this many iterations left.
const FALLBACK_THRESHOLD: u32 = 10_000;
/// The number of cache entries to keep per node
const CACHE_ENTRIES: usize = 10;

type Str = Rc<str>;

/// A function definition, or lack thereof
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FunctionDefinition {
    FunctionUnknown,
    FunctionDefinition(Str, Node, Node),
}
use FunctionDefinition::FunctionUnknown;

/// The Set of places a command name can point (it's a Set to handle
/// conditionally defined functions)
type FunctionValue = Rc<BTreeSet<FunctionDefinition>>;

fn unknown_function_value() -> FunctionValue {
    Rc::new(BTreeSet::from([FunctionUnknown]))
}

// ---- VariableState helpers ----

fn unknown_variable_value() -> VariableValue {
    VariableValue { literal_value: None, space_status: SpaceStatusDirty, numerical_status: NumericalStatusUnknown }
}
fn empty_variable_value() -> VariableValue {
    VariableValue {
        literal_value: Some(Rc::from("")),
        space_status: SpaceStatusEmpty,
        numerical_status: NumericalStatusEmpty,
    }
}
fn unknown_integer_value() -> VariableValue {
    VariableValue { literal_value: None, space_status: SpaceStatusClean, numerical_status: NumericalStatusDefinitely }
}
fn unknown_variable_state() -> VariableState {
    VariableState { variable_value: unknown_variable_value(), variable_properties: VariableProperties::DEFAULT }
}
fn unset_variable_state() -> VariableState {
    VariableState { variable_value: empty_variable_value(), variable_properties: VariableProperties::DEFAULT }
}

fn add_properties(props: PropSet, mut state: VariableState) -> VariableState {
    state.variable_properties = state.variable_properties.map(|s| s.union(props));
    state
}
fn remove_properties(props: PropSet, mut state: VariableState) -> VariableState {
    state.variable_properties = state.variable_properties.map(|s| s.difference(props));
    state
}

fn merge_variable_state(a: &VariableState, b: &VariableState) -> VariableState {
    VariableState {
        variable_value: merge_variable_value(&a.variable_value, &b.variable_value),
        variable_properties: a.variable_properties.union(b.variable_properties),
    }
}

fn merge_variable_value(a: &VariableValue, b: &VariableValue) -> VariableValue {
    VariableValue {
        literal_value: if a.literal_value == b.literal_value { a.literal_value.clone() } else { None },
        space_status: merge_space_status(a.space_status, b.space_status),
        numerical_status: merge_numerical_status(a.numerical_status, b.numerical_status),
    }
}

fn merge_space_status(a: SpaceStatus, b: SpaceStatus) -> SpaceStatus {
    match (a, b) {
        (SpaceStatusEmpty, y) => y,
        (x, SpaceStatusEmpty) => x,
        (SpaceStatusClean, SpaceStatusClean) => SpaceStatusClean,
        _ => SpaceStatusDirty,
    }
}

fn merge_numerical_status(a: NumericalStatus, b: NumericalStatus) -> NumericalStatus {
    match (a, b) {
        (NumericalStatusDefinitely, NumericalStatusDefinitely) => NumericalStatusDefinitely,
        (NumericalStatusDefinitely, _) => NumericalStatusMaybe,
        (_, NumericalStatusDefinitely) => NumericalStatusMaybe,
        (NumericalStatusMaybe, _) => NumericalStatusMaybe,
        (_, NumericalStatusMaybe) => NumericalStatusMaybe,
        (NumericalStatusEmpty, NumericalStatusEmpty) => NumericalStatusEmpty,
        _ => NumericalStatusUnknown,
    }
}

/// Append two VariableValues as if with z="$x$y"
fn append_variable_value(a: &VariableValue, b: &VariableValue) -> VariableValue {
    VariableValue {
        literal_value: match (&a.literal_value, &b.literal_value) {
            (Some(x), Some(y)) => Some(Rc::from(format!("{x}{y}"))),
            _ => None,
        },
        space_status: append_space_status(a.space_status, b.space_status),
        numerical_status: append_numerical_status(a.numerical_status, b.numerical_status),
    }
}

fn append_space_status(a: SpaceStatus, b: SpaceStatus) -> SpaceStatus {
    match (a, b) {
        (SpaceStatusEmpty, _) => b,
        (_, SpaceStatusEmpty) => a,
        (SpaceStatusClean, SpaceStatusClean) => a,
        _ => SpaceStatusDirty,
    }
}

fn append_numerical_status(a: NumericalStatus, b: NumericalStatus) -> NumericalStatus {
    match (a, b) {
        (NumericalStatusEmpty, x) => x,
        (x, NumericalStatusEmpty) => x,
        (NumericalStatusDefinitely, NumericalStatusDefinitely) => NumericalStatusDefinitely,
        (NumericalStatusUnknown, _) => NumericalStatusUnknown,
        (_, NumericalStatusUnknown) => NumericalStatusUnknown,
        _ => NumericalStatusMaybe,
    }
}

fn literal_to_variable_value(s: &str) -> VariableValue {
    VariableValue {
        literal_value: Some(Rc::from(s)),
        space_status: literal_to_space_status(s),
        numerical_status: literal_to_numerical_status(s),
    }
}

/// Get the SpaceStatus for a literal string, i.e. if it needs quoting
fn literal_to_space_status(s: &str) -> SpaceStatus {
    if s.is_empty() {
        SpaceStatusEmpty
    } else if s.chars().all(|c| !" \t\n*?[".contains(c)) {
        SpaceStatusClean
    } else {
        SpaceStatusDirty
    }
}

/// Get the NumericalStatus for a literal string, i.e. whether it's an integer
fn literal_to_numerical_status(s: &str) -> NumericalStatus {
    if s.is_empty() {
        return NumericalStatusEmpty;
    }
    let rest = s.strip_prefix('-').unwrap_or(s);
    if rest.chars().all(|c| c.is_ascii_digit()) { NumericalStatusDefinitely } else { NumericalStatusUnknown }
}

// ---- VersionedMap ----

/// A Map that keeps an additional integer version to quickly determine if it
/// has changed.
/// * Version -1 means it's unknown (possibly and presumably changed)
/// * Version 0 means it's empty
/// * Version N means it's equal to any other map with Version N
#[derive(Clone, Debug)]
struct VMap<V> {
    version: i64,
    storage: Rc<BTreeMap<Str, V>>,
}

impl<V: Clone + PartialEq> VMap<V> {
    /// `vmEmpty`
    fn empty() -> Self {
        VMap { version: 0, storage: Rc::new(BTreeMap::new()) }
    }
    /// `vmLookup`
    fn lookup(&self, k: &str) -> Option<&V> {
        self.storage.get(k)
    }
    /// `vmInsert`
    fn insert(&mut self, k: &str, v: V) {
        self.version = -1;
        Rc::make_mut(&mut self.storage).insert(Rc::from(k), v);
    }
    /// `vmIsQuickEqual`
    fn quick_eq(&self, other: &Self) -> bool {
        self.version >= 0 && other.version >= 0 && self.version == other.version
    }
    /// `instance Eq VersionedMap`
    fn equals(&self, other: &Self) -> bool {
        self.quick_eq(other) || Rc::ptr_eq(&self.storage, &other.storage) || *self.storage == *other.storage
    }
    /// `vmPatch`: overwrite all keys in the first map with values from the second
    fn patch(base: &Self, diff: &Self) -> Self {
        if base.version == 0 {
            diff.clone()
        } else if diff.version == 0 {
            base.clone()
        } else if base.quick_eq(diff) {
            diff.clone()
        } else if Rc::ptr_eq(&base.storage, &diff.storage) {
            // M.union m m == m
            VMap { version: -1, storage: diff.storage.clone() }
        } else if diff.storage.len() <= base.storage.len() {
            let mut storage = (*base.storage).clone();
            for (k, v) in diff.storage.iter() {
                storage.insert(k.clone(), v.clone());
            }
            VMap { version: -1, storage: Rc::new(storage) }
        } else {
            let mut storage = (*diff.storage).clone();
            for (k, v) in base.storage.iter() {
                storage.entry(k.clone()).or_insert_with(|| v.clone());
            }
            VMap { version: -1, storage: Rc::new(storage) }
        }
    }
}

// ---- InternalState ----

/// The current state of data flow at a point in the program, potentially as a diff
#[derive(Clone, Debug)]
struct IState {
    version: i64,
    globals: VMap<VariableState>,
    locals: VMap<VariableState>,
    prefix: VMap<VariableState>,
    funcs: VMap<FunctionValue>,
    exit_codes: Option<Rc<BTreeSet<Id>>>,
    is_reachable: Option<bool>,
}

/// `newInternalState`
fn new_internal_state() -> IState {
    IState {
        version: 0,
        globals: VMap::empty(),
        locals: VMap::empty(),
        prefix: VMap::empty(),
        funcs: VMap::empty(),
        exit_codes: None,
        is_reachable: None,
    }
}

/// `unreachableState = modified newInternalState { sIsReachable = Just False }`
fn unreachable_state() -> IState {
    IState { version: -1, is_reachable: Some(false), ..new_internal_state() }
}

impl IState {
    /// `modified`
    fn modified(&mut self) {
        self.version = -1;
    }
    fn insert_global(&mut self, name: &str, value: VariableState) {
        self.modified();
        self.globals.insert(name, value);
    }
    fn insert_local(&mut self, name: &str, value: VariableState) {
        self.modified();
        self.locals.insert(name, value);
    }
    fn insert_prefix(&mut self, name: &str, value: VariableState) {
        self.modified();
        self.prefix.insert(name, value);
    }
    fn insert_function(&mut self, name: &str, value: FunctionValue) {
        self.modified();
        self.funcs.insert(name, value);
    }
    fn set_exit_codes(&mut self, set: Rc<BTreeSet<Id>>) {
        self.modified();
        self.exit_codes = Some(set);
    }
    /// `stateIsQuickEqual`
    fn quick_eq(&self, other: &IState) -> bool {
        self.version >= 0 && other.version >= 0 && self.version == other.version
    }
    /// `stateIsSlowEqual` (exit codes are not compared)
    fn slow_eq(&self, other: &IState) -> bool {
        self.globals.equals(&other.globals)
            && self.locals.equals(&other.locals)
            && self.prefix.equals(&other.prefix)
            && self.funcs.equals(&other.funcs)
            && self.is_reachable == other.is_reachable
    }
    /// `instance Eq InternalState`
    fn equals(&self, other: &IState) -> bool {
        self.quick_eq(other) || self.slow_eq(other)
    }
}

/// Overwrite a base state with the contents of a diff state
/// This is unrelated to join/merge.
fn patch_state(base: &IState, diff: &IState) -> IState {
    if diff.version == 0 {
        base.clone()
    } else if base.version == 0 || base.quick_eq(diff) {
        diff.clone()
    } else {
        IState {
            version: -1,
            globals: VMap::patch(&base.globals, &diff.globals),
            locals: VMap::patch(&base.locals, &diff.locals),
            prefix: VMap::patch(&base.prefix, &diff.prefix),
            funcs: VMap::patch(&base.funcs, &diff.funcs),
            exit_codes: diff.exit_codes.clone().or_else(|| base.exit_codes.clone()),
            is_reachable: diff.is_reachable.or(base.is_reachable),
        }
    }
}

/// The default state we assume we get from the environment
fn create_environment_state() -> IState {
    let mut s = new_internal_state();
    for name in data::INTERNAL_VARIABLES {
        s.insert_global(name, unknown_variable_state());
    }
    let spaceless = VariableState {
        variable_value: VariableValue {
            literal_value: None,
            space_status: SpaceStatusClean,
            numerical_status: NumericalStatusUnknown,
        },
        variable_properties: VariableProperties::DEFAULT,
    };
    for name in data::VARIABLES_WITHOUT_SPACES {
        s.insert_global(name, spaceless.clone());
    }
    let integer = VariableState {
        variable_value: unknown_integer_value(),
        variable_properties: VariableProperties::DEFAULT,
    };
    for name in data::SPECIAL_INTEGER_VARIABLES {
        s.insert_global(name, integer.clone());
    }
    s
}

// ---- Dependencies ----

/// Dependencies on values, e.g. "if there is a global variable named 'foo'
/// without spaces". This is used to see if the DFA of a function would result
/// in the same state, so anything that affects DFA must be tracked.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum StateDependency {
    /// Complete variable state
    DepState(Scope, Str, VariableState),
    /// Only variable properties (we need properties but not values for x=1)
    DepProperties(Scope, Str, VariableProperties),
    /// Function definition
    DepFunction(Str, FunctionValue),
    /// Whether invoking the node would result in recursion (i.e., is the
    /// function on the stack?)
    DepIsRecursive(Node, bool),
    /// The set of commands that could have provided the exit code $?
    DepExitCodes(Rc<BTreeSet<Id>>),
}
use StateDependency::*;

type Deps = Rc<BTreeSet<StateDependency>>;

/// Create an InternalState that fulfills the given dependencies
fn deps_to_state(set: &BTreeSet<StateDependency>) -> IState {
    let mut state = new_internal_state();
    fn insert_in(state: &mut IState, overwrite: bool, scope: Scope, name: &str, val: VariableState) {
        let map = match scope {
            Scope::PrefixScope => &state.prefix,
            Scope::LocalScope => &state.locals,
            Scope::GlobalScope => &state.globals,
        };
        let already_exists = map.lookup(name).is_some();
        if overwrite || !already_exists {
            match scope {
                Scope::PrefixScope => state.insert_prefix(name, val),
                Scope::LocalScope => state.insert_local(name, val),
                Scope::GlobalScope => state.insert_global(name, val),
            }
        }
    }
    for dep in set {
        match dep {
            DepFunction(name, val) => state.insert_function(name, val.clone()),
            DepState(scope, name, val) => insert_in(&mut state, true, *scope, name, val.clone()),
            // State includes properties and more, so don't overwrite a state with properties
            DepProperties(scope, name, props) => insert_in(
                &mut state,
                false,
                *scope,
                name,
                VariableState { variable_properties: *props, ..unknown_variable_state() },
            ),
            DepIsRecursive(..) => {}
            DepExitCodes(s) => state.set_exit_codes(s.clone()),
        }
    }
    state
}

// ---- Ctx ----

/// Whenever a function (or subshell) is invoked, a value like this is pushed onto the stack
struct StackEntry {
    /// The entry point of this stack entry for the purpose of detecting recursion
    entry_point: Node,
    /// Whether this is a function call (as opposed to a subshell)
    is_function_call: bool,
    /// The node where this entry point was invoked
    call_site: Node,
    /// A mutable set of dependencies we fetched from here or higher in the stack
    dependencies: BTreeSet<StateDependency>,
    /// The original input state for this stack entry
    stack_state: IState,
}

type StateMap = BTreeMap<Node, (IState, IState)>;

/// What `runCached` computes on a cache miss (the Haskell closures `f`).
enum Job {
    Subshell(Node, Node),
    Function(Node, Node),
}

struct Ctx<'g> {
    /// The current node
    node: Node,
    /// The current input state
    input: IState,
    /// The current output state
    output: IState,
    /// The current functions/subshells stack (top last)
    stack: Vec<StackEntry>,
    /// The input graph
    graph: &'g CfGraph,
    /// Per node: the `CFEFlow` predecessors as `context` lists them
    /// (ascending, with multiplicity, no self loops).
    flow_incoming: Vec<Vec<Node>>,
    /// An incrementing counter to version maps
    counter: i64,
    /// A cache of input state dependencies to output effects (newest first)
    cache: BTreeMap<Node, Vec<(Deps, IState)>>,
    /// Whether the cache is enabled (see FALLBACK_THRESHOLD)
    enable_cache: bool,
    /// The states resulting from data flows per invocation path
    invocations: BTreeMap<Vec<Node>, (Deps, StateMap)>,
}

/// Which `lookupStack` getter to use; a closure can't borrow `Ctx` mutably.
fn get_variable_with_scope(s: &IState, name: &str) -> Option<(VariableState, Scope)> {
    if let Some(v) = s.prefix.lookup(name) {
        Some((v.clone(), Scope::PrefixScope))
    } else if let Some(v) = s.locals.lookup(name) {
        Some((v.clone(), Scope::LocalScope))
    } else {
        s.globals.lookup(name).map(|v| (v.clone(), Scope::GlobalScope))
    }
}

impl<'g> Ctx<'g> {
    /// `newCtx`
    fn new(graph: &'g CfGraph) -> Self {
        let n = graph.node_range().1 as usize + 1;
        let mut flow_incoming = vec![Vec::new(); n];
        for (node, incoming) in flow_incoming.iter_mut().enumerate() {
            let node = node as Node;
            if graph.label(node).is_some() {
                *incoming = graph
                    .lpre(node)
                    .iter()
                    .filter(|(p, l)| *p != node && *l == CFEdge::CFEFlow)
                    .map(|(p, _)| *p)
                    .collect();
            }
        }
        Ctx {
            node: 0,
            input: new_internal_state(),
            output: new_internal_state(),
            stack: Vec::new(),
            graph,
            flow_incoming,
            counter: 1,
            cache: BTreeMap::new(),
            enable_cache: true,
            invocations: BTreeMap::new(),
        }
    }

    /// The next incrementing version for VersionedMaps
    fn next_version(&mut self) -> i64 {
        let n = self.counter;
        self.counter += 1;
        n
    }

    /// `lookupStack'`: look up each state on the stack until a value is found
    /// (or the default is used), then add this value as a StateDependency to
    /// every frame visited.
    fn lookup_stack<V: Clone>(
        &mut self,
        function_only: bool,
        get: impl Fn(&IState) -> Option<V>,
        dep: impl Fn(&V) -> StateDependency,
        def: V,
    ) -> V {
        if let Some(v) = get(&self.input) {
            return v;
        }
        let mut visited = 0;
        let mut res = None;
        for s in self.stack.iter().rev() {
            if function_only && s.is_function_call {
                break;
            }
            visited += 1;
            if let Some(v) = get(&s.stack_state) {
                res = Some(v);
                break;
            }
        }
        let res = res.unwrap_or(def);
        if visited > 0 {
            let d = dep(&res);
            let len = self.stack.len();
            for s in &mut self.stack[len - visited..] {
                s.dependencies.insert(d.clone());
            }
        }
        res
    }

    /// Like lookupStack but without adding dependencies
    fn peek_stack<V>(&self, get: impl Fn(&IState) -> Option<V>, def: V) -> V {
        if let Some(v) = get(&self.input) {
            return v;
        }
        self.stack.iter().rev().find_map(|s| get(&s.stack_state)).unwrap_or(def)
    }

    /// Look up a variable value, and also return its scope
    fn read_variable_with_scope(&mut self, name: &str) -> (VariableState, Scope) {
        self.lookup_stack(
            false,
            |s| get_variable_with_scope(s, name),
            |(val, scope)| DepState(*scope, Rc::from(name), val.clone()),
            (unknown_variable_state(), Scope::GlobalScope),
        )
    }

    /// Look up the variable's properties. This can be done independently to
    /// avoid incurring a dependency on the value.
    fn read_variable_properties_with_scope(&mut self, name: &str) -> (VariableProperties, Scope) {
        self.lookup_stack(
            false,
            |s| get_variable_with_scope(s, name).map(|(v, scope)| (v.variable_properties, scope)),
            |(val, scope)| DepProperties(*scope, Rc::from(name), *val),
            (VariableProperties::DEFAULT, Scope::GlobalScope),
        )
    }

    fn read_variable_scope(&mut self, name: &str) -> Scope {
        self.read_variable_properties_with_scope(name).1
    }

    fn read_variable(&mut self, name: &str) -> VariableState {
        self.read_variable_with_scope(name).0
    }

    fn read_global(&mut self, name: &str) -> VariableState {
        self.lookup_stack(
            false,
            |s| s.globals.lookup(name).cloned(),
            |v| DepState(Scope::GlobalScope, Rc::from(name), v.clone()),
            // could come from the environment
            unknown_variable_state(),
        )
    }

    fn read_global_properties(&mut self, name: &str) -> VariableProperties {
        self.lookup_stack(
            false,
            |s| s.globals.lookup(name).map(|v| v.variable_properties),
            // This dependency will fail to match if it's shadowed by a local
            // variable, such as in  x=1; f() { local -i x; declare -ag x; }
            // (see the Haskell FIXME).
            |v| DepProperties(Scope::GlobalScope, Rc::from(name), *v),
            VariableProperties::DEFAULT,
        )
    }

    fn read_local(&mut self, name: &str) -> VariableState {
        self.lookup_stack(
            true,
            |s| s.locals.lookup(name).cloned(),
            |v| DepState(Scope::LocalScope, Rc::from(name), v.clone()),
            // can't come from the environment
            unset_variable_state(),
        )
    }

    /// We only want to look up the local properties of the current function,
    /// though preferably even if we're in a subshell.
    fn read_local_properties(&mut self, name: &str) -> VariableProperties {
        self.lookup_stack(
            true,
            |s| {
                s.locals
                    .lookup(name)
                    .map(|v| (v.variable_properties, Scope::LocalScope))
                    .or_else(|| s.prefix.lookup(name).map(|v| (v.variable_properties, Scope::PrefixScope)))
            },
            |(val, scope)| DepProperties(*scope, Rc::from(name), *val),
            (VariableProperties::DEFAULT, Scope::LocalScope),
        )
        .0
    }

    fn read_function(&mut self, name: &str) -> FunctionValue {
        self.lookup_stack(
            false,
            |s| s.funcs.lookup(name).cloned(),
            |v| DepFunction(Rc::from(name), v.clone()),
            unknown_function_value(),
        )
    }

    fn read_exit_codes(&mut self) -> Rc<BTreeSet<Id>> {
        self.lookup_stack(false, |s| s.exit_codes.clone(), |v| DepExitCodes(v.clone()), Rc::new(BTreeSet::new()))
    }

    /// Set a variable. This includes properties. Applies it to the appropriate scope.
    fn write_variable(&mut self, name: &str, val: VariableState) {
        match self.read_variable_scope(name) {
            Scope::GlobalScope => self.output.insert_global(name, val),
            Scope::LocalScope => self.output.insert_local(name, val),
            // Prefixed variables actually become local variables in the invoked function
            Scope::PrefixScope => self.output.insert_local(name, val),
        }
    }

    fn write_function(&mut self, name: &str, val: FunctionDefinition) {
        self.output.insert_function(name, Rc::new(BTreeSet::from([val])));
    }

    fn update_variable_value(&mut self, name: &str, val: VariableValue) {
        let (props, scope) = self.read_variable_properties_with_scope(name);
        let state = VariableState { variable_value: val, variable_properties: props };
        match scope {
            Scope::GlobalScope => self.output.insert_global(name, state),
            // Updates become local
            Scope::LocalScope | Scope::PrefixScope => self.output.insert_local(name, state),
        }
    }

    fn update_global_value(&mut self, name: &str, val: VariableValue) {
        let props = self.read_global_properties(name);
        self.output.insert_global(name, VariableState { variable_value: val, variable_properties: props });
    }

    fn update_local_value(&mut self, name: &str, val: VariableValue) {
        let props = self.read_local_properties(name);
        self.output.insert_local(name, VariableState { variable_value: val, variable_properties: props });
    }

    fn update_prefix_value(&mut self, name: &str, val: VariableValue) {
        // Prefix variables don't inherit properties
        self.output
            .insert_prefix(name, VariableState { variable_value: val, variable_properties: VariableProperties::DEFAULT });
    }

    fn undefine_variable(&mut self, name: &str) {
        self.write_variable(name, unset_variable_state());
    }

    fn undefine_function(&mut self, name: &str) {
        self.write_function(name, FunctionUnknown);
    }

    fn patch_output(&mut self, diff: &IState) {
        self.output = patch_state(&self.output, diff);
    }

    // ---- merging ----

    /// Merge (aka Join) two states. This needs the context to look up values.
    fn merge_state(&mut self, a: &IState, b: &IState) -> IState {
        // Kludge: we want `readVariable` & friends not to read from an
        // intermediate state, so temporarily set a blank input.
        let old = std::mem::replace(&mut self.input, new_internal_state());
        let x = self.merge(a, b);
        self.input = old;
        x
    }

    fn merge(&mut self, a: &IState, b: &IState) -> IState {
        match (a.is_reachable, b.is_reachable) {
            (Some(true), Some(false)) | (Some(false), Some(true)) => {
                panic!("{}", please_report("Unexpected merge of reachable and unreachable state"))
            }
            (Some(false), Some(false)) => return unreachable_state(),
            _ => {}
        }
        if a.version >= 0 && b.version >= 0 && a.version == b.version {
            return a.clone();
        }
        let globals = self.merge_maps(&a.globals, &b.globals, merge_variable_state, |c, k| c.read_global(k));
        let locals = self.merge_maps(&a.locals, &b.locals, merge_variable_state, |c, k| c.read_variable(k));
        let prefix = self.merge_maps(&a.prefix, &b.prefix, merge_variable_state, |c, k| c.read_variable(k));
        let funcs = self.merge_maps(&a.funcs, &b.funcs, union_functions, |c, k| c.read_function(k));
        let exit_codes = match (&a.exit_codes, &b.exit_codes) {
            (None, None) => None,
            (Some(v), None) | (None, Some(v)) => {
                let other = self.read_exit_codes();
                Some(union_ids(v, &other))
            }
            (Some(v1), Some(v2)) => Some(union_ids(v1, v2)),
        };
        IState {
            version: -1,
            globals,
            locals,
            prefix,
            funcs,
            exit_codes,
            is_reachable: match (a.is_reachable, b.is_reachable) {
                (Some(x), Some(y)) => Some(x && y),
                _ => None,
            },
        }
    }

    /// Merge a number of states, or return a default if there are no states
    fn merge_states(&mut self, def: &IState, list: &[IState]) -> IState {
        match list {
            [] => def.clone(),
            [first, rest @ ..] => {
                let mut acc = first.clone();
                for s in rest {
                    acc = self.merge_state(&acc, s);
                }
                acc
            }
        }
    }

    /// Merge two maps, key by key. If both maps have a key, the 'merger' is
    /// used. If only one has the key, the 'reader' is used to fetch a second,
    /// and the two are merged as above.
    fn merge_maps<V: Clone + PartialEq>(
        &mut self,
        a: &VMap<V>,
        b: &VMap<V>,
        merger: fn(&V, &V) -> V,
        reader: impl Fn(&mut Self, &str) -> V,
    ) -> VMap<V> {
        if a.quick_eq(b) {
            return a.clone();
        }
        // DIVERGENCE (performance only): merging a map with itself walks every
        // key with `merger v v`, which is the identity for both mergers.
        if Rc::ptr_eq(&a.storage, &b.storage) {
            return VMap { version: -1, storage: a.storage.clone() };
        }
        let mut out = BTreeMap::new();
        let mut i1 = a.storage.iter().peekable();
        let mut i2 = b.storage.iter().peekable();
        loop {
            match (i1.peek(), i2.peek()) {
                (None, None) => break,
                (Some((k, v)), None) | (None, Some((k, v))) => {
                    let (k, v) = ((*k).clone(), (*v).clone());
                    let other = reader(self, &k);
                    out.insert(k, merger(&v, &other));
                    if i1.peek().is_some() {
                        i1.next();
                    } else {
                        i2.next();
                    }
                }
                (Some((k1, v1)), Some((k2, v2))) => match k1.cmp(k2) {
                    std::cmp::Ordering::Equal => {
                        out.insert((*k1).clone(), merger(v1, v2));
                        i1.next();
                        i2.next();
                    }
                    std::cmp::Ordering::Less => {
                        let (k, v) = ((*k1).clone(), (*v1).clone());
                        let nv2 = reader(self, &k);
                        out.insert(k, merger(&v, &nv2));
                        i1.next();
                    }
                    std::cmp::Ordering::Greater => {
                        let (k, v) = ((*k2).clone(), (*v2).clone());
                        let nv1 = reader(self, &k);
                        out.insert(k, merger(&nv1, &v));
                        i2.next();
                    }
                },
            }
        }
        VMap { version: -1, storage: Rc::new(out) }
    }

    /// Give a VersionedMap a version if it does not already have one.
    fn version_map<V>(&mut self, map: &mut VMap<V>) {
        if map.version < 0 {
            map.version = self.next_version();
        }
    }

    /// Give an InternalState a version if it does not already have one.
    fn version_state(&mut self, mut state: IState) -> IState {
        if state.version >= 0 {
            return state;
        }
        state.version = self.next_version();
        self.version_map(&mut state.globals);
        self.version_map(&mut state.locals);
        self.version_map(&mut state.funcs);
        state
    }

    // ---- stack frames ----

    /// Call a function with a new stack entry on the stack
    fn with_new_stack_frame<T>(&mut self, node: Node, is_call: bool, f: impl FnOnce(&mut Self) -> T) -> (T, StackEntry) {
        let entry = StackEntry {
            entry_point: node,
            is_function_call: is_call,
            call_site: self.node,
            dependencies: BTreeSet::new(),
            stack_state: self.output.clone(),
        };
        let saved_input = std::mem::replace(&mut self.input, new_internal_state());
        let saved_output = std::mem::replace(&mut self.output, new_internal_state());
        let saved_node = std::mem::replace(&mut self.node, node);
        self.stack.push(entry);
        let x = f(self);
        let entry = self.stack.pop().expect("stack frame");
        self.input = saved_input;
        self.output = saved_output;
        self.node = saved_node;
        (x, entry)
    }

    /// Check if invoking this function would be a recursive loop
    /// (i.e. we already have the function on the stack)
    fn would_be_recursive(&mut self, node: Node) -> bool {
        let found = self.stack.iter().rposition(|s| s.entry_point == node);
        let (from, res) = match found {
            Some(i) => (i, true),
            None => (0, false),
        };
        for s in &mut self.stack[from..] {
            s.dependencies.insert(DepIsRecursive(node, res));
        }
        res
    }

    // ---- transfer ----

    /// The main DFA 'transfer' function, applying the effects of a node to the output state
    fn transfer(&mut self, label: &CFNode) {
        match label {
            CFNode::CFStructuralNode
            | CFNode::CFEntryPoint(_)
            | CFNode::CFImpliedExit
            | CFNode::CFResolvedExit => {}
            CFNode::CFExecuteCommand(cmd) => self.transfer_command(cmd.as_deref()),
            CFNode::CFExecuteSubshell(_reason, entry, exit) => self.transfer_subshell(*entry, *exit),
            CFNode::CFApplyEffects(effects) => {
                for IdTagged(_, f) in effects {
                    self.transfer_effect(f);
                }
            }
            CFNode::CFSetExitCode(id) => self.output.set_exit_codes(Rc::new(BTreeSet::from([*id]))),
            CFNode::CFUnresolvedExit | CFNode::CFUnreachable => self.patch_output(&unreachable_state()),
            // TODO
            CFNode::CFSetBackgroundPid(_) => {}
            CFNode::CFDropPrefixAssignments => {
                self.output.modified();
                self.output.prefix = VMap::empty();
            }
        }
    }

    /// Transfer the effects of a subshell invocation. This is similar to a
    /// function call to allow easily discarding the effects.
    fn transfer_subshell(&mut self, entry: Node, exit: Node) {
        let initial = self.output.clone();
        self.run_cached(entry, Job::Subshell(entry, exit));
        // Clear subshell changes. TODO: track this to warn about modifications.
        let exit_codes = self.output.exit_codes.clone();
        self.output = IState { exit_codes, ..initial };
    }

    /// Transfer the effects of executing a command, i.e. the merged union of
    /// all possible function definitions.
    fn transfer_command(&mut self, name: Option<&str>) {
        let Some(name) = name else { return };
        let targets = self.read_function(name);
        self.transfer_multiple(&targets);
    }

    /// Transfer a set of function definitions and merge the output states.
    fn transfer_multiple(&mut self, funcs: &BTreeSet<FunctionDefinition>) {
        let original = self.output.clone();
        let mut branches = Vec::with_capacity(funcs.len());
        for f in funcs {
            self.output = original.clone();
            self.transfer_function_value(f);
            branches.push(self.output.clone());
        }
        let merged = self.merge_states(&original, &branches);
        self.output = patch_state(&original, &merged);
    }

    /// Transfer the effects of a single function definition.
    fn transfer_function_value(&mut self, func: &FunctionDefinition) {
        match func {
            FunctionUnknown => {}
            FunctionDefinition::FunctionDefinition(_name, entry, exit) => {
                // TODO: Find a better strategy for recursion
                if !self.would_be_recursive(*entry) {
                    self.run_cached(*entry, Job::Function(*entry, *exit));
                }
            }
        }
    }

    fn run_job(&mut self, job: Job) -> (Deps, IState) {
        match job {
            Job::Subshell(entry, exit) => {
                let (states, frame) = self.with_new_stack_frame(entry, false, |c| c.dataflow(entry));
                let res = match states.get(&exit) {
                    Some((_, res)) => res.clone(),
                    None => panic!("{}", please_report("Subshell has no exit")),
                };
                let deps = Rc::new(frame.dependencies);
                self.register_flow_result(entry, states, deps.clone());
                (deps, res)
            }
            Job::Function(entry, exit) => {
                let (states, frame) = self.with_new_stack_frame(entry, true, |c| c.dataflow(entry));
                let deps = Rc::new(frame.dependencies);
                let res = match states.get(&exit) {
                    Some((_input, output)) => {
                        // Discard local variables. TODO: track&retain
                        // variables declared local in previous scopes?
                        IState { version: -1, locals: VMap::empty(), ..output.clone() }
                    }
                    // e.g. f() { exit; }
                    None => unreachable_state(),
                };
                self.register_flow_result(entry, states, deps.clone());
                (deps, res)
            }
        }
    }

    /// Register/save the result of a dataflow of a function.
    fn register_flow_result(&mut self, entry: Node, states: StateMap, deps: Deps) {
        // A unique path to this flow context, so that 'x=1; f; x=2; f' won't
        // overwrite each other.
        let mut path = Vec::with_capacity(self.stack.len() + 2);
        path.push(entry);
        path.push(self.node);
        path.extend(self.stack.iter().rev().map(|s| s.call_site));
        self.invocations.insert(path, (deps, states));
    }

    /// Look up a node in the cache and see if the dependencies of any entries
    /// are matched. In that case, reuse the previous result instead of doing
    /// a new data flow.
    fn run_cached(&mut self, node: Node, job: Job) {
        if let Some(v) = self.get_cache(node) {
            self.patch_output(&v);
        } else {
            let (deps, diff) = self.run_job(job);
            let list = self.cache.entry(node).or_default();
            list.insert(0, (deps, diff.clone()));
            list.truncate(CACHE_ENTRIES + 1);
            self.patch_output(&diff);
        }
    }

    /// Get a cached version whose dependencies are currently fulfilled, if any.
    fn get_cache(&self, node: Node) -> Option<IState> {
        if !self.enable_cache {
            return None;
        }
        let list = self.cache.get(&node)?;
        list.iter().find(|(deps, _)| self.fulfills_dependencies(node, deps)).map(|(_, v)| v.clone())
    }

    /// Check if the current context fulfills all StateDependencies
    fn fulfills_dependencies(&self, entry: Node, deps: &BTreeSet<StateDependency>) -> bool {
        deps.iter().all(|d| self.fulfills_dependency(entry, d))
    }

    /// Check if the current context fulfills a StateDependency if entering `entry`
    fn fulfills_dependency(&self, entry: Node, dep: &StateDependency) -> bool {
        let peek = |scope: Scope, name: &str| {
            let def = if scope == Scope::GlobalScope {
                (unknown_variable_state(), Scope::GlobalScope)
            } else {
                (unset_variable_state(), Scope::LocalScope)
            };
            self.peek_stack(|s| get_variable_with_scope(s, name), def)
        };
        match dep {
            DepState(scope, name, val) => {
                let (v, s) = peek(*scope, name);
                v == *val && s == *scope
            }
            DepProperties(scope, name, props) => {
                let (state, s) = peek(*scope, name);
                *scope == s && state.variable_properties == *props
            }
            DepFunction(name, val) => {
                self.peek_stack(|s| s.funcs.lookup(name).cloned(), unknown_function_value()) == *val
            }
            // Hack. Since we haven't pushed the soon-to-be invoked function
            // on the stack, it won't be found by the normal check.
            DepIsRecursive(node, _) if *node == entry => true,
            DepIsRecursive(node, val) => *val == self.stack.iter().any(|f| f.entry_point == *node),
            DepExitCodes(val) => {
                self.peek_stack(|s| s.exit_codes.clone(), Rc::new(BTreeSet::new())) == *val
            }
        }
    }

    /// Transfer a single CFEffect to the output state.
    fn transfer_effect(&mut self, effect: &CFEffect) {
        match effect {
            CFEffect::CFReadVariable(name) => {
                if name == "?" {
                    self.read_exit_codes();
                } else {
                    self.read_variable(name);
                }
            }
            CFEffect::CFWriteVariable(name, value) => {
                let val = self.cf_value_to_variable_value(value);
                self.update_variable_value(name, val);
            }
            CFEffect::CFWriteGlobal(name, value) => {
                let val = self.cf_value_to_variable_value(value);
                self.update_global_value(name, val);
            }
            CFEffect::CFWriteLocal(name, value) => {
                let val = self.cf_value_to_variable_value(value);
                self.update_local_value(name, val);
            }
            CFEffect::CFWritePrefix(name, value) => {
                let val = self.cf_value_to_variable_value(value);
                self.update_prefix_value(name, val);
            }
            CFEffect::CFSetProps(scope, name, props) => self.change_props(*scope, name, |s| add_properties(*props, s)),
            CFEffect::CFUnsetProps(scope, name, props) => {
                self.change_props(*scope, name, |s| remove_properties(*props, s))
            }
            CFEffect::CFUndefineVariable(name) => self.undefine_variable(name),
            CFEffect::CFUndefineFunction(name) => self.undefine_function(name),
            CFEffect::CFUndefine(name) => {
                // This should really just unset one or the other
                self.undefine_variable(name);
                self.undefine_function(name);
            }
            CFEffect::CFDefineFunction(name, _id, entry, exit) => self.write_function(
                name,
                FunctionDefinition::FunctionDefinition(Rc::from(name.as_str()), *entry, *exit),
            ),
            // TODO
            CFEffect::CFUndefineNameref(name) => self.undefine_variable(name),
            CFEffect::CFHintArray(_) | CFEffect::CFHintDefined(_) => {}
        }
    }

    /// The shared body of `CFSetProps` and `CFUnsetProps`.
    fn change_props(&mut self, scope: Option<Scope>, name: &str, f: impl Fn(VariableState) -> VariableState) {
        match scope {
            None => {
                let state = self.read_variable(name);
                self.write_variable(name, f(state));
            }
            Some(Scope::GlobalScope) => {
                let state = self.read_global(name);
                self.output.insert_global(name, f(state));
            }
            // Prefix values become local
            Some(Scope::LocalScope) | Some(Scope::PrefixScope) => {
                let state = self.read_local(name);
                self.output.insert_local(name, f(state));
            }
        }
    }

    /// Transfer the CFG's idea of a value into our VariableState
    fn cf_value_to_variable_value(&mut self, val: &CFValue) -> VariableValue {
        match val {
            // TODO: Track array status
            CFValue::CFValueArray => unknown_variable_value(),
            CFValue::CFValueComputed(_, parts) => {
                let mut acc = empty_variable_value();
                for part in parts {
                    let next = self.compute_value(part);
                    acc = append_variable_value(&acc, &next);
                }
                acc
            }
            CFValue::CFValueInteger => unknown_integer_value(),
            CFValue::CFValueString => unknown_variable_value(),
            CFValue::CFValueUninitialized => empty_variable_value(),
        }
    }

    /// A value can be computed from 0 or more parts, such as x="literal$y$z"
    fn compute_value(&mut self, part: &CFStringPart) -> VariableValue {
        match part {
            CFStringPart::CFStringLiteral(s) => literal_to_variable_value(s),
            CFStringPart::CFStringInteger => unknown_integer_value(),
            CFStringPart::CFStringUnknown => unknown_variable_value(),
            CFStringPart::CFStringVariable(name) => {
                let state = self.read_variable(name);
                if state.variable_properties.all_have(CFVariableProp::CFVPInteger) {
                    unknown_integer_value()
                } else {
                    state.variable_value
                }
            }
        }
    }

    // ---- the data flow ----

    /// Classic, iterative Data Flow Analysis.
    fn dataflow(&mut self, entry: Node) -> StateMap {
        let mut pending: BTreeSet<Node> = BTreeSet::from([entry]);
        let mut states: StateMap = BTreeMap::new();
        // withoutChanges
        let prev_input = self.input.clone();
        let prev_output = self.output.clone();
        let mut n = ITERATION_COUNT;
        loop {
            if n == 0 {
                panic!("{}", please_report("DFA did not reach fix point"));
            }
            if n == FALLBACK_THRESHOLD {
                // This should never happen, but has historically been due to
                // caching bugs. Try disabling the cache and continuing.
                self.enable_cache = false;
            }
            let Some(next) = pending.pop_first() else { break };
            self.process(&mut states, &mut pending, next);
            n -= 1;
        }
        self.input = prev_input;
        self.output = prev_output;
        states
    }

    fn process(&mut self, states: &mut StateMap, pending: &mut BTreeSet<Node>, node: Node) {
        let graph = self.graph;
        let incoming = std::mem::take(&mut self.flow_incoming[node as usize]);
        let input = if incoming.is_empty() {
            new_internal_state()
        } else {
            let mut inputs = incoming
                .iter()
                .filter_map(|c| states.get(c).map(|s| &s.1))
                .filter(|c| c.is_reachable != Some(false));
            match inputs.next() {
                None => unreachable_state(),
                Some(first) => {
                    let rest: Vec<&IState> = inputs.collect();
                    let mut acc = first.clone();
                    for s in rest {
                        acc = self.merge_state(&acc, s);
                    }
                    acc
                }
            }
        };
        self.flow_incoming[node as usize] = incoming;
        self.input = input.clone();
        self.output = input.clone();
        self.node = node;
        let label = graph.label(node).expect("dataflow on a missing node");
        self.transfer(label);
        let new_output = std::mem::replace(&mut self.output, new_internal_state());
        let outgoing = graph.lsuc(node);
        let result = if outgoing.len() >= 2 {
            // Version the state because we split and will probably merge later
            self.version_state(new_output)
        } else {
            new_output
        };
        let changed = match states.get(&node) {
            None => true,
            Some((_, old_output)) => !old_output.equals(&result),
        };
        states.insert(node, (input, result));
        if changed {
            pending.extend(outgoing.iter().map(|(s, _)| *s));
        }
    }

    fn run_root(&mut self, env: &IState, entry: Node, exit: Node) -> IState {
        self.input = env.clone();
        self.output = env.clone();
        self.node = entry;
        let (states, frame) = self.with_new_stack_frame(entry, false, |c| c.dataflow(entry));
        let deps = Rc::new(frame.dependencies);
        // Return the final state, used to invoke functions that were declared but not invoked
        let res = match states.get(&exit) {
            Some((_, out)) => out.clone(),
            None => panic!("{}", please_report("Missing exit state")),
        };
        self.register_flow_result(entry, states, deps);
        res
    }
}

fn union_functions(a: &FunctionValue, b: &FunctionValue) -> FunctionValue {
    if Rc::ptr_eq(a, b) || b.is_subset(a) {
        return a.clone();
    }
    Rc::new(a.union(b).cloned().collect())
}

fn union_ids(a: &Rc<BTreeSet<Id>>, b: &Rc<BTreeSet<Id>>) -> Rc<BTreeSet<Id>> {
    if Rc::ptr_eq(a, b) || b.is_subset(a) {
        return a.clone();
    }
    if a.is_subset(b) {
        return b.clone();
    }
    Rc::new(a.union(b).copied().collect())
}

/// Get the all the functions defined in an InternalState
fn get_function_targets(state: &IState) -> BTreeMap<Node, FunctionDefinition> {
    let declared: BTreeSet<&FunctionDefinition> = state.funcs.storage.values().flat_map(|s| s.iter()).collect();
    let mut out = BTreeMap::new();
    for d in declared {
        if let FunctionDefinition::FunctionDefinition(_, entry, _) = d {
            out.insert(*entry, d.clone());
        }
    }
    out
}

/// `internalToExternal`, sharing the censored maps between nodes.
struct Externalizer {
    censored: HashMap<*const BTreeMap<Str, VariableState>, VarMap>,
    empty_codes: Rc<BTreeSet<Id>>,
}

impl Externalizer {
    fn censor(&mut self, m: &VarMap) -> VarMap {
        if let Some(c) = self.censored.get(&Rc::as_ptr(m)) {
            return c.clone();
        }
        // Censor the literal value to avoid introducing dependencies on it.
        let c = if m.values().all(|v| v.variable_value.literal_value.is_none()) {
            m.clone()
        } else {
            Rc::new(
                m.iter()
                    .map(|(k, v)| {
                        let mut v = v.clone();
                        v.variable_value.literal_value = None;
                        (k.clone(), v)
                    })
                    .collect(),
            )
        };
        self.censored.insert(Rc::as_ptr(m), c.clone());
        c
    }

    fn convert(&mut self, s: &IState) -> ProgramState {
        ProgramState {
            scopes: [self.censor(&s.prefix.storage), self.censor(&s.locals.storage), self.censor(&s.globals.storage)],
            exit_codes: s.exit_codes.clone().unwrap_or_else(|| self.empty_codes.clone()),
            state_is_reachable: s.is_reachable.unwrap_or(true),
        }
    }
}

pub(super) fn analyze_control_flow(params: &CfgParameters, ast: &Ast, root: Id) -> CfgAnalysis {
    let cfg = crate::cfg::build_graph(params, ast, root);
    let Some((entry, exit)) = cfg.cf_id_to_range(root) else {
        panic!("{}", please_report("Missing root"));
    };
    let env = create_environment_state();
    let (lo, hi) = cfg.cf_graph.node_range();

    let node_to_data = {
        let mut ctx = Ctx::new(&cfg.cf_graph);
        // Do a dataflow analysis starting on the root node
        let exit_state = ctx.run_root(&env, entry, exit);

        // All nodes we've touched
        let invoked_nodes: BTreeSet<Node> =
            ctx.invocations.values().flat_map(|(_, states)| states.keys().copied()).collect();

        // Invoke all functions that were declared but not invoked. This is so
        // that we still get warnings for dead code (it's probably not
        // actually dead, just used by a script that sources ours).
        let declared_functions = get_function_targets(&exit_state);
        let uninvoked: Vec<FunctionDefinition> = declared_functions
            .into_iter()
            .filter(|(entry, _)| !invoked_nodes.contains(entry))
            .map(|(_, d)| d)
            .collect();

        // We don't want `die() { exit $?; }; echo "Sourced"` to assume $? is always echo
        let straggler_input = IState { exit_codes: None, ..patch_state(&env, &exit_state) };

        // analyzeStragglers
        for def in &uninvoked {
            if let FunctionDefinition::FunctionDefinition(_, entry, _) = def {
                ctx.input = straggler_input.clone();
                ctx.output = straggler_input.clone();
                ctx.node = *entry;
                ctx.transfer_function_value(def);
            }
        }

        // Now round up all the states from all data flows (FIXME: this
        // excludes functions that were defined in straggling functions).
        // groupByNode: `M.fromListWith (++)`, so each list is in reverse
        // order of occurrence.
        let invocations = std::mem::take(&mut ctx.invocations);
        let mut grouped: BTreeMap<Node, Vec<(IState, IState)>> = BTreeMap::new();
        for (_, (deps, states)) in invocations {
            // Include the dependencies in the state of each function, e.g. if
            // it depends on `x=foo` then add that.
            let base = deps_to_state(&deps);
            for (node, (a, b)) in states {
                grouped.entry(node).or_default().push((patch_state(&base, &a), patch_state(&base, &b)));
            }
        }

        // flattenByNode: merge all the pre/post states for each node.
        let mut ext = Externalizer { censored: HashMap::new(), empty_codes: Rc::new(BTreeSet::new()) };
        let unreachable = ext.convert(&unreachable_state());
        let mut merged: BTreeMap<Node, (ProgramState, ProgramState)> = BTreeMap::new();
        for (node, mut list) in grouped {
            list.reverse();
            let (pres, posts): (Vec<IState>, Vec<IState>) = list.into_iter().unzip();
            // The lists are never empty ("Null node states").
            let none = new_internal_state();
            let pre = ctx.merge_states(&none, &pres);
            let post = ctx.merge_states(&none, &posts);
            merged.insert(node, (ext.convert(&pre), ext.convert(&post)));
        }
        // Fill in unreachable states for anything we didn't get to
        // (`baseStates` covers the whole `nodeRange`).
        (lo..=hi)
            .map(|n| merged.remove(&n).unwrap_or_else(|| (unreachable.clone(), unreachable.clone())))
            .collect::<Vec<_>>()
    };

    CfgAnalysis { cfg, node_to_data, data_base: lo }
}
