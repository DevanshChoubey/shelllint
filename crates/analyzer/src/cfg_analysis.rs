// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/CFGAnalysis.hs (e11f964)
//
// Data Flow Analysis on a Control Flow Graph.
//
// This module implements a pretty standard iterative Data Flow Analysis.
// Since shell scripts rely heavily on global variables, this DFA includes
// tracking the value of globals across calls. Each function invocation is
// treated as a separate DFA problem, and a caching mechanism (hopefully)
// avoids any exponential explosions.
//
// The public API (this file) mirrors the module's export list. The engine
// (Haskell's ST code with a manual `Ctx`) lives in `cfg_analysis/dfa.rs`.
//
// DIVERGENCE: `ProgramState.variablesInScope` is a `Map String
// VariableState` built per node with `M.unions`. Storing that map for every
// node is quadratic in memory, so `ProgramState` keeps the three scopes
// (persistent maps shared with the DFA and other nodes) and
// `variables_in_scope` / `variable_in_scope` compute the union on demand.
// Each scope is kept as two layers, the node's state and its invocation's
// base state, so `patchState base` isn't materialized per node either.
// Haskell censors `literalValue` with `M.map censor`, another full copy per
// node; here the accessors censor the values they return instead. The
// results are the same.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use shellcheck_core::ast::{Ast, Id};

pub use crate::cfg::{CFVariableProp, CfGraph, CfgParameters, Node, PropSet};
use crate::cfg::CfgResult;

mod dfa;

/// The result of the data flow analysis (`data CFGAnalysis`).
#[derive(Clone, Debug, Default)]
pub struct CfgAnalysis {
    pub(crate) cfg: CfgResult,
    /// `nodeToData`, indexed by `node - data_base`, covering `nodeRange`.
    pub(crate) node_to_data: Vec<(ProgramState, ProgramState)>,
    pub(crate) data_base: Node,
}

impl CfgAnalysis {
    /// `graph`
    pub fn graph(&self) -> &CfGraph {
        &self.cfg.cf_graph
    }
    /// `M.lookup id (tokenToRange a)`
    pub fn token_to_range(&self, id: Id) -> Option<(Node, Node)> {
        self.cfg.cf_id_to_range(id)
    }
    /// `M.lookup id (tokenToNodes a)` (computed on demand, see CfgResult)
    pub fn token_to_nodes(&self, id: Id) -> Option<BTreeSet<Node>> {
        self.cfg.cf_id_to_nodes(id)
    }
    /// `postDominators a ! node`
    pub fn post_dominators(&self, node: Node) -> Vec<Node> {
        self.cfg.cf_post_dominators(node)
    }
    /// `M.lookup node (nodeToData a)` (Haskell's `getDataForNode`)
    pub fn node_to_data(&self, node: Node) -> Option<&(ProgramState, ProgramState)> {
        let i = node.checked_sub(self.data_base)? as usize;
        self.node_to_data.get(i)
    }
}

/// The persistent (structurally shared) map used for all DFA state maps.
pub(crate) type PMap<V> = rpds::RedBlackTreeMap<Rc<str>, V>;

/// A variable scope (or one layer of it) as stored in a `ProgramState`.
#[derive(Clone, Debug)]
pub(crate) enum Layer {
    /// A DFA state map, shared with the DFA and other nodes.
    Map(Rc<PMap<VariableState>>),
    /// A compact sorted map, for the per-invocation states built from the
    /// dependencies (one per function invocation, never modified).
    Sorted(Rc<[(Rc<str>, VariableState)]>),
}

impl Layer {
    fn get(&self, name: &str) -> Option<&VariableState> {
        match self {
            Layer::Map(m) => m.get(name),
            Layer::Sorted(v) => v.binary_search_by(|(k, _)| (**k).cmp(name)).ok().map(|i| &v[i].1),
        }
    }
    /// The address of the shared storage (an identity for deduplication).
    pub(crate) fn addr(&self) -> usize {
        match self {
            Layer::Map(m) => Rc::as_ptr(m) as *const u8 as usize,
            Layer::Sorted(v) => Rc::as_ptr(v) as *const u8 as usize,
        }
    }
    fn for_each(&self, mut f: impl FnMut(&Rc<str>, &VariableState)) {
        match self {
            Layer::Map(m) => m.iter().for_each(|(k, v)| f(k, v)),
            Layer::Sorted(v) => v.iter().for_each(|(k, v)| f(k, v)),
        }
    }
}

/// `censor`: drop the literal value (it's just for debugging).
fn censor(v: &VariableState) -> VariableState {
    let mut v = v.clone();
    v.variable_value.literal_value = None;
    v
}

/// The program state we expose externally (`data ProgramState`).
// Shared: most nodes have the same state as a neighbour, and the DFA
// deduplicates them (see `Externalizer` in dfa.rs).
#[derive(Clone, Debug)]
pub struct ProgramState(pub(crate) Rc<StateData>);

#[derive(Debug)]
pub(crate) struct StateData {
    /// `[sPrefixValues, sLocalValues, sGlobalValues]`, uncensored, each as
    /// two layers (a node's state, then the base state of its invocation;
    /// `patchState` is their left-biased union). `variablesInScope` is the
    /// left-biased union of all six, censored.
    pub(crate) scopes: [Layer; 6],
    pub(crate) exit_codes: Rc<BTreeSet<Id>>,
    pub(crate) state_is_reachable: bool,
}

impl ProgramState {
    /// `variablesInScope`: every variable visible here, as a map.
    /// Prefer `variable_in_scope` for a single lookup.
    pub fn variables_in_scope(&self) -> BTreeMap<String, VariableState> {
        let mut out = BTreeMap::new();
        for m in &self.0.scopes {
            m.for_each(|k, v| {
                out.entry(k.to_string()).or_insert_with(|| censor(v));
            });
        }
        out
    }
    /// `M.lookup name (variablesInScope state)`
    pub fn variable_in_scope(&self, name: &str) -> Option<VariableState> {
        self.0.scopes.iter().find_map(|m| m.get(name)).map(censor)
    }
    /// `exitCodes`: the set of commands that could have provided `$?`.
    pub fn exit_codes(&self) -> &BTreeSet<Id> {
        &self.0.exit_codes
    }
    /// `stateIsReachable`
    pub fn state_is_reachable(&self) -> bool {
        self.0.state_is_reachable
    }
}

impl PartialEq for ProgramState {
    fn eq(&self, other: &Self) -> bool {
        self.variables_in_scope() == other.variables_in_scope()
            && self.0.exit_codes == other.0.exit_codes
            && self.0.state_is_reachable == other.0.state_is_reachable
    }
}
impl Eq for ProgramState {}

/// The information about the value of a single variable (`data VariableValue`)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableValue {
    /// For debugging. Always `None` in a `ProgramState` (censored).
    pub literal_value: Option<Rc<str>>,
    pub space_status: SpaceStatus,
    pub numerical_status: NumericalStatus,
}

/// `data VariableState`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableState {
    pub variable_value: VariableValue,
    pub variable_properties: VariableProperties,
}

/// Whether or not the value needs quoting (has spaces/globs), or we don't know
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpaceStatus {
    SpaceStatusEmpty,
    SpaceStatusClean,
    SpaceStatusDirty,
}

/// Whether or not the value is an integer, or we don't know
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumericalStatus {
    NumericalStatusUnknown,
    NumericalStatusEmpty,
    NumericalStatusMaybe,
    NumericalStatusDefinitely,
}

/// The set of possible sets of properties for this variable
/// (`type VariableProperties = S.Set (S.Set CFVariableProp)`).
// DIVERGENCE: a 16-bit set over the 16 possible `PropSet`s (bit n = the
// `PropSet` with bits n). `Ord` and iteration follow Haskell's `Set` order.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableProperties(u16);

impl VariableProperties {
    /// `S.empty`
    pub const EMPTY: VariableProperties = VariableProperties(0);
    /// `defaultProperties = S.singleton S.empty`
    pub const DEFAULT: VariableProperties = VariableProperties(1);

    pub fn singleton(s: PropSet) -> Self {
        VariableProperties(1 << s.bits())
    }
    /// `S.toAscList`: the member sets, in Haskell `Set` order.
    pub fn sets(self) -> Vec<PropSet> {
        let ranked = self.ranked();
        (0..16u8).filter(|r| ranked & (1 << r) != 0).map(PropSet::from_rank).collect()
    }
    /// The member sets as a bit set over `PropSet::rank` (bit r = the set
    /// at rank r), so that ascending bits are Haskell `Set` order.
    fn ranked(self) -> u16 {
        let mut out = 0u16;
        let mut bits = self.0;
        while bits != 0 {
            let b = bits.trailing_zeros() as u8;
            out |= 1 << PropSet::from_bits(b).rank();
            bits &= bits - 1;
        }
        out
    }
    pub fn contains(self, s: PropSet) -> bool {
        self.0 & (1 << s.bits()) != 0
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// The `PropSet`s (as bits of `self.0`) that contain `prop`.
    fn having(prop: CFVariableProp) -> u16 {
        // Bit pattern b contains prop number i iff bit i of b is set.
        const HAVING: [u16; 4] = [0xAAAA, 0xCCCC, 0xF0F0, 0xFF00];
        HAVING[prop as usize]
    }
    /// `any (S.member prop)`: some execution path has the property.
    pub fn any_has(self, prop: CFVariableProp) -> bool {
        self.0 & Self::having(prop) != 0
    }
    /// `all (S.member prop)`: every execution path has the property
    /// (vacuously true for the empty set).
    pub fn all_have(self, prop: CFVariableProp) -> bool {
        self.0 & !Self::having(prop) == 0
    }
    /// `S.union`
    pub fn union(self, other: Self) -> Self {
        VariableProperties(self.0 | other.0)
    }
    /// `S.map f`
    pub fn map(self, f: impl Fn(PropSet) -> PropSet) -> Self {
        let mut out = 0u16;
        for b in 0..16u8 {
            if self.0 & (1 << b) != 0 {
                out |= 1 << f(PropSet::from_bits(b)).bits();
            }
        }
        VariableProperties(out)
    }
}

impl Ord for VariableProperties {
    /// Lexicographic on the ascending lists of member sets (`self.sets()`),
    /// without building them: the lists agree up to the least set `d` in
    /// only one of them. The list holding `d` is smaller if the other one
    /// continues past `d` (with a set greater than `d`), and greater if the
    /// other one ends there.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        let (a, b) = (self.ranked(), other.ranked());
        let diff = a ^ b;
        if diff == 0 {
            return Equal;
        }
        let d = diff.trailing_zeros();
        let above = |x: u16| d < 15 && x >> (d + 1) != 0;
        if a & (1 << d) != 0 {
            if above(b) { Less } else { Greater }
        } else if above(a) {
            Greater
        } else {
            Less
        }
    }
}
impl PartialOrd for VariableProperties {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::fmt::Debug for VariableProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.sets()).finish()
    }
}

/// `analyzeControlFlow :: CFGParameters -> Token -> CFGAnalysis`
pub fn analyze_control_flow(params: &CfgParameters, ast: &Ast, root: Id) -> CfgAnalysis {
    dfa::analyze_control_flow(params, ast, root)
}

/// Conveniently get the state before a token id
pub fn get_incoming_state(analysis: &CfgAnalysis, id: Id) -> Option<&ProgramState> {
    let (start, _end) = analysis.token_to_range(id)?;
    analysis.node_to_data(start).map(|(a, _)| a)
}

/// Conveniently get the state after a token id
pub fn get_outgoing_state(analysis: &CfgAnalysis, id: Id) -> Option<&ProgramState> {
    let (_start, end) = analysis.token_to_range(id)?;
    analysis.node_to_data(end).map(|(_, b)| b)
}

/// Conveniently determine whether one node postdominates another,
/// i.e. whether 'target' always unconditionally runs after 'base'.
pub fn does_post_dominate(analysis: &CfgAnalysis, target: Id, base: Id) -> bool {
    let Some((_, base_end)) = analysis.token_to_range(base) else { return false };
    let Some((target_start, _)) = analysis.token_to_range(target) else { return false };
    analysis.cfg.is_post_dominated_by(base_end, target_start)
}

/// See if any execution path results in the variable containing a state
fn variable_may_have_state(state: &ProgramState, var: &str, property: CFVariableProp) -> Option<bool> {
    let value = state.variable_in_scope(var)?;
    Some(value.variable_properties.any_has(property))
}

/// See if any execution path declares the variable an integer (declare -i).
pub fn variable_may_be_declared_integer(state: &ProgramState, var: &str) -> Option<bool> {
    variable_may_have_state(state, var, CFVariableProp::CFVPInteger)
}

/// See if any execution path suggests the variable may contain an integer value
pub fn variable_may_be_assigned_integer(state: &ProgramState, var: &str) -> Option<bool> {
    let value = state.variable_in_scope(var)?;
    Some(value.variable_value.numerical_status >= NumericalStatus::NumericalStatusMaybe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_properties_order_and_queries() {
        // A deterministic sample of the 2^16 values, plus the small ones.
        let mut xs: Vec<u16> = (0..256).collect();
        let mut seed = 0x2545_f491u32;
        for _ in 0..600 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            xs.push(seed as u16);
        }
        for &a in &xs {
            let va = VariableProperties(a);
            let mut naive: Vec<PropSet> = (0..16u8).filter(|b| a & (1 << b) != 0).map(PropSet::from_bits).collect();
            naive.sort();
            assert_eq!(va.sets(), naive);
            for prop in CFVariableProp::ALL {
                assert_eq!(va.any_has(prop), naive.iter().any(|s| s.contains(prop)));
                assert_eq!(va.all_have(prop), naive.iter().all(|s| s.contains(prop)));
            }
            for &b in &xs {
                let vb = VariableProperties(b);
                assert_eq!(va.cmp(&vb), va.sets().cmp(&vb.sets()), "{a:#x} {b:#x}");
            }
        }
    }
}
