// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFG.hs (e11f964)
//
// Constructs a Control Flow Graph from an AST.
//
// DIVERGENCE: Haskell uses fgl's `Gr CFNode CFEdge` (a PatriciaTree). Here
// `CfGraph` is a compact Vec-indexed graph with u32 node ids. Node ids are
// exactly the Haskell ones (including the holes that
// `removeUnnecessaryStructuralNodes` leaves), and adjacency lists keep fgl's
// order (ascending neighbour), so iteration orders that affect the data flow
// analysis are unchanged. The fgl algorithms CFG.hs relies on (`ufold`
// contexts, `&`, `delNode`, `grev`, `dom`, `topsort`) are ported in
// `cfg/fgl.rs` with the same observable behaviour.

use std::collections::BTreeSet;

use shellcheck_core::ast::Id;

mod build;
mod fgl;

pub use build::build_graph;

/// fgl's `Node`.
pub type Node = u32;

/// Node labels in a Control Flow Graph
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CFNode {
    /// A no-op node for structural purposes
    CFStructuralNode,
    /// A no-op for graph inspection purposes
    CFEntryPoint(String),
    /// Drop current prefix assignments
    CFDropPrefixAssignments,
    /// A node with a certain effect on program state
    CFApplyEffects(Vec<IdTagged<CFEffect>>),
    /// The execution of a command or function by literal string if possible
    CFExecuteCommand(Option<String>),
    /// Execute a subshell. These are represented by disjoint graphs just like
    /// functions, but they don't require any form of name resolution
    CFExecuteSubshell(String, Node, Node),
    /// Assignment of $?
    CFSetExitCode(Id),
    /// The virtual 'exit' at the natural end of a subshell
    CFImpliedExit,
    /// An exit statement resolvable at CFG build time
    CFResolvedExit,
    /// An exit statement only resolvable at DFA time
    CFUnresolvedExit,
    /// An unreachable node, serving as the unconnected end point of a range
    CFUnreachable,
    /// Assignment of $!
    CFSetBackgroundPid(Id),
}

/// Edge labels in a Control Flow Graph
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CFEdge {
    CFEErrExit,
    /// Regular control flow edge
    CFEFlow,
    /// An edge that a human might think exists (e.g. from a backgrounded process to its parent)
    CFEFalseFlow,
    /// An edge followed on exit
    CFEExit,
}

/// Actions we track
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CFEffect {
    CFSetProps(Option<Scope>, String, PropSet),
    CFUnsetProps(Option<Scope>, String, PropSet),
    CFReadVariable(String),
    CFWriteVariable(String, CFValue),
    CFWriteGlobal(String, CFValue),
    CFWriteLocal(String, CFValue),
    CFWritePrefix(String, CFValue),
    CFDefineFunction(String, Id, Node, Node),
    CFUndefine(String),
    CFUndefineVariable(String),
    CFUndefineFunction(String),
    CFUndefineNameref(String),
    /// Usage implies that this is an array (e.g. it's expanded with index)
    CFHintArray(String),
    /// Operation implies that the variable will be defined (e.g. [ -z "$var" ])
    CFHintDefined(String),
}

/// `data IdTagged a = IdTagged Id a`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdTagged<A>(pub Id, pub A);

/// Where a variable's value comes from
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CFValue {
    /// The special 'uninitialized' value
    CFValueUninitialized,
    /// An arbitrary array value
    CFValueArray,
    /// An arbitrary string value
    CFValueString,
    /// An arbitrary integer
    CFValueInteger,
    /// Token 'Id' concatenates and assigns the given parts
    CFValueComputed(Id, Vec<CFStringPart>),
}

/// Simplified computed strings
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CFStringPart {
    /// A known literal string value, like 'foo'
    CFStringLiteral(String),
    /// The contents of a variable, like $foo (may not be a string)
    CFStringVariable(String),
    /// An value that is unknown but an integer
    CFStringInteger,
    /// An unknown string value, for things we can't handle
    CFStringUnknown,
}

/// The properties of a variable
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CFVariableProp {
    CFVPExport,
    CFVPArray,
    CFVPAssociative,
    CFVPInteger,
}

impl CFVariableProp {
    pub const ALL: [CFVariableProp; 4] = [
        CFVariableProp::CFVPExport,
        CFVariableProp::CFVPArray,
        CFVariableProp::CFVPAssociative,
        CFVariableProp::CFVPInteger,
    ];
    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// `S.Set CFVariableProp`
// DIVERGENCE: a 4-bit set instead of a `Data.Set`. `Ord` is Haskell's
// `Set` order (lexicographic on the ascending element list).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PropSet(u8);

impl PropSet {
    pub const EMPTY: PropSet = PropSet(0);

    pub fn from_props(props: &[CFVariableProp]) -> PropSet {
        PropSet(props.iter().fold(0, |a, p| a | p.bit()))
    }
    pub fn contains(self, p: CFVariableProp) -> bool {
        self.0 & p.bit() != 0
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn union(self, other: PropSet) -> PropSet {
        PropSet(self.0 | other.0)
    }
    pub fn difference(self, other: PropSet) -> PropSet {
        PropSet(self.0 & !other.0)
    }
    /// `S.toAscList`
    pub fn to_asc_list(self) -> Vec<CFVariableProp> {
        CFVariableProp::ALL.into_iter().filter(|p| self.contains(*p)).collect()
    }
    /// The raw 4-bit representation (bit n = `CFVariableProp` number n).
    pub fn bits(self) -> u8 {
        self.0
    }
    pub(crate) fn from_bits(b: u8) -> PropSet {
        PropSet(b & 0xf)
    }
}

impl Ord for PropSet {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_asc_list().cmp(&other.to_asc_list())
    }
}
impl PartialOrd for PropSet {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::fmt::Debug for PropSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.to_asc_list()).finish()
    }
}

/// Options when generating CFG
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CfgParameters {
    /// Whether the last element in a pipeline runs in the current shell
    pub cf_lastpipe: bool,
    /// Whether all elements in a pipeline count towards the exit status
    pub cf_pipefail: bool,
}

/// `data Scope = GlobalScope | LocalScope | PrefixScope`
// (Not to be confused with AnalyzerLib's `Scope`.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    GlobalScope,
    LocalScope,
    PrefixScope,
}

/// The control flow graph (`type CFGraph = G.Gr CFNode CFEdge`).
///
/// Nodes are `0..=max`; ids that `removeUnnecessaryStructuralNodes`
/// removed are holes (`label` returns `None`).
#[derive(Clone, Debug, Default)]
pub struct CfGraph {
    /// Label per node id; `None` for holes.
    labels: Vec<Option<CFNode>>,
    /// Outgoing edges per node, in fgl adjacency order.
    succs: Vec<Vec<(Node, CFEdge)>>,
    /// Incoming edges per node, in fgl adjacency order.
    preds: Vec<Vec<(Node, CFEdge)>>,
}

impl CfGraph {
    /// fgl `nodeRange`: the smallest and largest node id.
    pub fn node_range(&self) -> (Node, Node) {
        let min = self.labels.iter().position(Option::is_some);
        let max = self.labels.iter().rposition(Option::is_some);
        match (min, max) {
            (Some(a), Some(b)) => (a as Node, b as Node),
            _ => panic!("nodeRange of empty graph"),
        }
    }
    /// fgl `lab`: the label of a node, if it exists.
    pub fn label(&self, n: Node) -> Option<&CFNode> {
        self.labels.get(n as usize).and_then(Option::as_ref)
    }
    /// fgl `labNodes`, ascending.
    pub fn lab_nodes(&self) -> impl Iterator<Item = (Node, &CFNode)> + '_ {
        self.labels.iter().enumerate().filter_map(|(i, l)| l.as_ref().map(|l| (i as Node, l)))
    }
    /// fgl `nodes`, ascending.
    pub fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        self.lab_nodes().map(|(n, _)| n)
    }
    /// fgl `lsuc`: outgoing `(successor, label)`, self loops included.
    pub fn lsuc(&self, n: Node) -> &[(Node, CFEdge)] {
        self.succs.get(n as usize).map_or(&[], Vec::as_slice)
    }
    /// fgl `lpre`: incoming `(predecessor, label)`, self loops included.
    pub fn lpre(&self, n: Node) -> &[(Node, CFEdge)] {
        self.preds.get(n as usize).map_or(&[], Vec::as_slice)
    }
    /// fgl `labEdges`: every edge `(from, to, label)`.
    pub fn lab_edges(&self) -> impl Iterator<Item = (Node, Node, CFEdge)> + '_ {
        self.succs
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.iter().map(move |(to, l)| (i as Node, *to, *l)))
    }
}

/// `data CFGResult`
#[derive(Clone, Debug, Default)]
pub struct CfgResult {
    /// The graph itself
    pub cf_graph: CfGraph,
    /// `cfIdToRange`, indexed by token id (`NO_RANGE` when absent).
    id_to_range: Vec<(Node, Node)>,
    /// `cfIdToNodes`, built on first use from the token stack recorded per
    /// original node (see `cf_id_to_nodes`).
    assoc: build::Association,
    /// `cfPostDominators`: the immediate post-dominator of each node
    /// (`NO_NODE` for nodes that have no post-dominators, and for the root).
    post_idom: Vec<Node>,
    post_root: Node,
}

pub(crate) const NO_NODE: Node = Node::MAX;
const NO_RANGE: (Node, Node) = (NO_NODE, NO_NODE);

impl CfgResult {
    /// `M.lookup id cfIdToRange`: the nominal start&end node of a token
    /// (i.e. assuming normal execution without exits).
    pub fn cf_id_to_range(&self, id: Id) -> Option<(Node, Node)> {
        self.id_to_range.get(id.0 as usize).copied().filter(|r| *r != NO_RANGE)
    }

    /// `M.lookup id cfIdToNodes`: all nodes belonging to an Id, recursively.
    // Like Haskell's, the map is built lazily: on the first call, in one
    // pass over every node's token stack; later calls are lookups.
    pub fn cf_id_to_nodes(&self, id: Id) -> Option<BTreeSet<Node>> {
        self.assoc.nodes_for(id)
    }

    /// `cfPostDominators ! node`: the nodes that post-dominate `node`
    /// (`node` itself first, then its immediate post-dominator, up to the
    /// main exit), or `[]` for nodes the reversed graph doesn't reach.
    pub fn cf_post_dominators(&self, node: Node) -> Vec<Node> {
        let mut out = Vec::new();
        let n = node as usize;
        if n >= self.post_idom.len() || (self.post_idom[n] == NO_NODE && node != self.post_root) {
            return out;
        }
        let mut cur = node;
        loop {
            out.push(cur);
            if cur == self.post_root {
                break;
            }
            cur = self.post_idom[cur as usize];
        }
        out
    }

    /// `target `elem` (cfPostDominators ! base)`, without building the list.
    pub fn is_post_dominated_by(&self, base: Node, target: Node) -> bool {
        let n = base as usize;
        if n >= self.post_idom.len() || (self.post_idom[n] == NO_NODE && base != self.post_root) {
            return false;
        }
        let mut cur = base;
        loop {
            if cur == target {
                return true;
            }
            if cur == self.post_root {
                return false;
            }
            cur = self.post_idom[cur as usize];
        }
    }
}
