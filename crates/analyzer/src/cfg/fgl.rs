// SPDX-License-Identifier: GPL-3.0-only
// The parts of fgl (Data.Graph.Inductive, PatriciaTree) that ShellCheck's
// CFG.hs uses, plus CFG.hs `inlineSubshells`, `findTerminalNodes` and
// `findPostDominators` (e11f964).
//
// fgl semantics that matter here and are reproduced on purpose:
// * `mkGraph`/`insEdge` keep adjacency per neighbour in ascending order,
//   with parallel edges newest first.
// * `ufold` decomposes the graph one node at a time, smallest first, so the
//   context it hands out for node n only contains edges to nodes that have
//   not been decomposed yet: predecessors > n and successors >= n.
//   `inlineSubshells` re-inserts the subshell node with exactly that partial
//   context, which drops edges from earlier nodes into it.
// * `context`/`match` give predecessors without self loops and successors
//   with them.
// * `dom` returns, for every node reachable from the root, the node itself,
//   its immediate dominator, and so on up to the root. Unreachable nodes are
//   absent (and get `[]` in `cfPostDominators`).

use super::CFEdge;
use super::CFEffect::CFDefineFunction;
use super::CFNode::{self, CFApplyEffects, CFExecuteSubshell, CFUnresolvedExit};
use super::{CfGraph, IdTagged, NO_NODE, Node};

/// `mkGraph nodes edges`
pub(super) fn mk_graph(labels: Vec<Option<CFNode>>, edges: &[(Node, Node, CFEdge)]) -> CfGraph {
    let n = labels.len();
    let mut succs: Vec<Vec<(Node, CFEdge)>> = vec![Vec::new(); n];
    let mut preds: Vec<Vec<(Node, CFEdge)>> = vec![Vec::new(); n];
    // `insEdges` folds left; `insertWith addLists w [l]` puts the newest
    // label first. A stable sort by neighbour of the reversed list gives
    // ascending neighbours with newest-first labels.
    for &(from, to, l) in edges.iter().rev() {
        // `IM.adjust` on a missing node is a no-op.
        if labels.get(from as usize).is_some_and(Option::is_some) {
            succs[from as usize].push((to, l));
        }
        if labels.get(to as usize).is_some_and(Option::is_some) {
            preds[to as usize].push((from, l));
        }
    }
    for v in succs.iter_mut().chain(preds.iter_mut()) {
        v.sort_by_key(|(n, _)| *n);
        v.shrink_to_fit();
    }
    CfGraph { labels, succs, preds }
}

/// `topsort = reverse . postorderF . dff` over `nodes g` (ascending).
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn topsort(nodes: &[Node], edges: &[(Node, Node, CFEdge)]) -> Vec<Node> {
    let mut ns: Vec<Node> = nodes.to_vec();
    ns.sort();
    ns.dedup();
    let max = ns.iter().chain(edges.iter().flat_map(|(a, b, _)| [a, b])).copied().max().unwrap_or(0) as usize;
    let mut present = vec![false; max + 1];
    for &n in &ns {
        present[n as usize] = true;
    }
    let mut succ: Vec<Vec<Node>> = vec![Vec::new(); max + 1];
    for &(a, b, _) in edges {
        if present[a as usize] && present[b as usize] {
            succ[a as usize].push(b);
        }
    }
    for s in &mut succ {
        s.sort();
    }
    let mut seen = vec![false; max + 1];
    let mut post = Vec::new();
    for &root in &ns {
        if seen[root as usize] {
            continue;
        }
        seen[root as usize] = true;
        let mut stack: Vec<(Node, usize)> = vec![(root, 0)];
        while let Some((v, i)) = stack.last_mut() {
            let v = *v;
            if let Some(&w) = succ[v as usize].get(*i) {
                *i += 1;
                if !seen[w as usize] {
                    seen[w as usize] = true;
                    stack.push((w, 0));
                }
            } else {
                post.push(v);
                stack.pop();
            }
        }
    }
    post.reverse();
    post
}

/// A mutable, unlabelled multigraph-as-set view used to replay
/// `inlineSubshells` and `findPostDominators`. Edge labels and parallel
/// edges do not affect dominators, so adjacency is a sorted set.
struct DynGraph {
    succ: Vec<Vec<Node>>,
    pred: Vec<Vec<Node>>,
}

fn set_insert(v: &mut Vec<Node>, x: Node) {
    if let Err(i) = v.binary_search(&x) {
        v.insert(i, x);
    }
}
fn set_remove(v: &mut Vec<Node>, x: Node) {
    if let Ok(i) = v.binary_search(&x) {
        v.remove(i);
    }
}

impl DynGraph {
    fn from_edges(n: usize, edges: &[(Node, Node, CFEdge)]) -> DynGraph {
        let mut succ: Vec<Vec<Node>> = vec![Vec::new(); n];
        let mut pred: Vec<Vec<Node>> = vec![Vec::new(); n];
        for &(a, b, _) in edges {
            succ[a as usize].push(b);
            pred[b as usize].push(a);
        }
        for v in succ.iter_mut().chain(pred.iter_mut()) {
            v.sort();
            v.dedup();
        }
        DynGraph { succ, pred }
    }

    /// `context g v`: (predecessors without self loop, successors with it)
    fn context(&self, v: Node) -> (Vec<Node>, Vec<Node>) {
        let p = self.pred[v as usize].iter().copied().filter(|&p| p != v).collect();
        (p, self.succ[v as usize].clone())
    }

    /// `delNode`
    fn del_node(&mut self, v: Node) {
        for p in std::mem::take(&mut self.pred[v as usize]) {
            set_remove(&mut self.succ[p as usize], v);
        }
        for s in std::mem::take(&mut self.succ[v as usize]) {
            set_remove(&mut self.pred[s as usize], v);
        }
    }

    /// `ctx & g` on a graph without `v` (`safeUpdate` deletes it first)
    fn insert(&mut self, v: Node, preds: &[Node], succs: &[Node]) {
        for &p in preds {
            set_insert(&mut self.pred[v as usize], p);
            set_insert(&mut self.succ[p as usize], v);
        }
        for &s in succs {
            set_insert(&mut self.succ[v as usize], s);
            set_insert(&mut self.pred[s as usize], v);
        }
    }

    /// `safeUpdate ctx graph = ctx & (delNode node graph)`
    fn safe_update(&mut self, v: Node, preds: &[Node], succs: &[Node]) {
        self.del_node(v);
        self.insert(v, preds, succs);
    }
}

/// Change all subshell invocations to instead link directly to their contents.
/// This is used for producing dominator trees.
fn inline_subshells(graph: &CfGraph, g: &mut DynGraph) {
    // `ufold find []`: ascending, each with its partial ufold context.
    let mut subshells: Vec<(Node, Node, Node, Vec<Node>, Vec<Node>)> = Vec::new();
    for (node, label) in graph.lab_nodes() {
        if let CFExecuteSubshell(_, start, end) = label {
            let incoming: Vec<Node> = g.pred[node as usize].iter().copied().filter(|&p| p > node).collect();
            let outgoing: Vec<Node> = g.succ[node as usize].iter().copied().filter(|&s| s >= node).collect();
            subshells.push((node, *start, *end, incoming, outgoing));
        }
    }
    // relinkedGraph = foldl' relink graph subshells
    for (node, start, end, incoming, outgoing) in subshells {
        // Link the subshell exit to the subshell's successors
        let (end_incoming, _) = g.context(end);
        g.safe_update(end, &end_incoming, &outgoing);
        // Link CFExecuteSubshell to the CFEntryPoint
        g.safe_update(node, &incoming, &[start]);
    }
}

/// `findTerminalNodes`
fn find_terminal_nodes(graph: &CfGraph) -> Vec<Node> {
    let mut out = Vec::new();
    for (node, label) in graph.lab_nodes() {
        match label {
            CFUnresolvedExit => out.push(node),
            CFApplyEffects(effects) => {
                for e in effects {
                    if let IdTagged(_, CFDefineFunction(_, _, _, end)) = e {
                        out.push(*end);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `findPostDominators mainexit (mkGraph nodes onlyRealEdges)`, returned as
/// an immediate-post-dominator array (`NO_NODE` = no entry) plus the root.
pub(super) fn find_post_dominators(
    mainexit: Node,
    graph: &CfGraph,
    real_edges: &[(Node, Node, CFEdge)],
) -> (Vec<Node>, Node) {
    let n = graph.labels.len();
    let original = DynGraph::from_edges(n, real_edges);
    let mut g = DynGraph { succ: original.succ.clone(), pred: original.pred.clone() };
    inline_subshells(graph, &mut g);
    // Labels are unchanged by inlining.
    let terminals = find_terminal_nodes(graph);
    let (mut incoming, outgoing) = original.context(mainexit);
    incoming.extend(terminals);
    g.safe_update(mainexit, &incoming, &outgoing);
    drop(original);
    // reversed = grev withExitEdges; postDoms = dom reversed mainexit
    // (the successors of v in the reversed graph are g.pred[v]).
    let idom = dominators(&g.pred, &g.succ, mainexit);
    // listArray (0, maxNode) $ repeat []
    let (_, max_node) = graph.node_range();
    let mut out = idom;
    out.truncate(max_node as usize + 1);
    out.resize(max_node as usize + 1, NO_NODE);
    (out, mainexit)
}

/// Immediate dominators of every node reachable from `root` following
/// `succ` (Cooper, Harvey & Kennedy, "A Simple, Fast Dominance Algorithm",
/// the same algorithm fgl's `dom` uses). `pred` must be the reverse of
/// `succ`. Unreachable nodes and the root get `NO_NODE`.
fn dominators(succ: &[Vec<Node>], pred: &[Vec<Node>], root: Node) -> Vec<Node> {
    let n = succ.len();
    const UNSEEN: u32 = u32::MAX;
    // Iterative DFS postorder.
    let mut order = vec![UNSEEN; n];
    let mut postorder: Vec<Node> = Vec::new();
    let mut visited = vec![false; n];
    visited[root as usize] = true;
    let mut stack: Vec<(Node, usize)> = vec![(root, 0)];
    while let Some((v, i)) = stack.last_mut() {
        let v = *v;
        if let Some(&w) = succ[v as usize].get(*i) {
            *i += 1;
            if !visited[w as usize] {
                visited[w as usize] = true;
                stack.push((w, 0));
            }
        } else {
            order[v as usize] = postorder.len() as u32;
            postorder.push(v);
            stack.pop();
        }
    }
    let mut idom = vec![NO_NODE; n];
    idom[root as usize] = root;
    let intersect = |idom: &[Node], mut a: Node, mut b: Node| {
        while a != b {
            while order[a as usize] < order[b as usize] {
                a = idom[a as usize];
            }
            while order[b as usize] < order[a as usize] {
                b = idom[b as usize];
            }
        }
        a
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &v in postorder.iter().rev() {
            if v == root {
                continue;
            }
            let mut new_idom = NO_NODE;
            for &p in &pred[v as usize] {
                if order[p as usize] == UNSEEN || idom[p as usize] == NO_NODE {
                    continue;
                }
                new_idom = if new_idom == NO_NODE { p } else { intersect(&idom, p, new_idom) };
            }
            if new_idom != NO_NODE && idom[v as usize] != new_idom {
                idom[v as usize] = new_idom;
                changed = true;
            }
        }
    }
    idom[root as usize] = NO_NODE;
    idom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominators_diamond() {
        // 0 -> 1 -> 3, 0 -> 2 -> 3, 3 -> 4; 5 unreachable
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)];
        let mut succ = vec![vec![]; 6];
        let mut pred = vec![vec![]; 6];
        for (a, b) in edges {
            succ[a].push(b as Node);
            pred[b].push(a as Node);
        }
        let idom = dominators(&succ, &pred, 0);
        assert_eq!(idom, vec![NO_NODE, 0, 0, 0, 3, NO_NODE]);
    }
}
