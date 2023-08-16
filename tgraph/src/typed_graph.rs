// typed graph

use std::collections::{hash_map, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use crate::arena::*;

pub mod library;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub usize);

impl ArenaIndex for NodeIndex {
    fn new(id: usize) -> Self {
        NodeIndex(id)
    }
}

#[derive(Debug, Clone)]
pub struct Graph<NodeT: NodeEnum> {
    nodes: Arena<NodeT, NodeIndex>,
    back_links: HashMap<NodeIndex, HashSet<(NodeIndex, NodeT::SourceEnum)>>,
}

impl<NodeT: NodeEnum> Graph<NodeT> {
    pub fn new(context: &Context) -> Self {
        Graph {
            nodes: Arena::new(Arc::clone(&context.node_dist)),
            back_links: HashMap::new(),
        }
    }

    pub fn get_node(&self, idx: NodeIndex) -> Option<&NodeT> {
        self.nodes.get(idx)
    }

    pub fn iter_nodes(&self) -> Iter<'_, NodeT> {
        self.nodes.iter()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn commit(&mut self, t: Transaction<NodeT>) {
        if t.committed {
            return;
        }

        self.merge_nodes(t.inc_nodes);
        for (i, f) in t.mut_nodes {
            self.modify_node(i, f)
        }
        for (i, f) in t.update_nodes {
            self.update_node(i, f)
        }
        for n in &t.dec_nodes {
            self.remove_node(*n);
        }
    }

    fn merge_nodes(&mut self, nodes: Arena<NodeT, NodeIndex>) {
        for (x, n) in &nodes {
            self.add_back_link(*x, n);
        }
        self.nodes.merge(nodes);
    }

    fn remove_node(&mut self, idx: NodeIndex) {
        let n = self.nodes.remove(idx).unwrap();
        self.remove_back_link(idx, &n);
        self.back_links.remove(&idx).unwrap();
    }

    fn modify_node<F>(&mut self, i: NodeIndex, f: F)
    where
        F: FnOnce(&mut NodeT),
    {
        for (y, s) in self.nodes.get(i).unwrap().iter_source() {
            self.back_links.get_mut(&y).unwrap().remove(&(i, s));
        }

        f(&mut self.nodes.get_mut(i).unwrap());

        for (y, s) in self.nodes.get(i).unwrap().iter_source() {
            self.back_links.get_mut(&y).unwrap().insert((i, s));
        }
    }

    fn update_node<F>(&mut self, i: NodeIndex, f: F)
    where
        F: FnOnce(NodeT) -> NodeT,
    {
        for (y, s) in self.nodes.get(i).unwrap().iter_source() {
            self.back_links.get_mut(&y).unwrap().remove(&(i, s));
        }

        self.nodes.update_with(i, |x| f(x));

        for (y, s) in self.nodes.get(i).unwrap().iter_source() {
            self.back_links.get_mut(&y).unwrap().insert((i, s));
        }
    }

    fn add_back_link(&mut self, x: NodeIndex, n: &NodeT) {
        self.back_links.entry(x).or_insert(HashSet::new());
        for (y, s) in n.iter_source() {
            self.back_links
                .entry(y)
                .or_insert(HashSet::new())
                .insert((x, s));
        }
    }

    fn remove_back_link(&mut self, x: NodeIndex, n: &NodeT) {
        for (y, s) in n.iter_source() {
            self.back_links.get_mut(&y).unwrap().remove(&(x, s));
        }
        self.back_links.remove(&x).unwrap();
    }
}

pub type Iter<'a, NDataT> = hash_map::Iter<'a, NodeIndex, NDataT>;

pub struct Transaction<'a, NodeT: NodeEnum> {
    committed: bool,
    alloc_nodes: HashSet<NodeIndex>,
    inc_nodes: Arena<NodeT, NodeIndex>,
    dec_nodes: Vec<NodeIndex>,
    mut_nodes: Vec<(NodeIndex, Box<dyn FnOnce(&mut NodeT) + 'a>)>,
    update_nodes: Vec<(NodeIndex, Box<dyn FnOnce(NodeT) -> NodeT + 'a>)>,
}

impl<'a, NodeT: NodeEnum> Transaction<'a, NodeT> {
    pub fn new(context: &Context) -> Self {
        let node_dist = Arc::clone(&context.node_dist);
        Transaction {
            committed: false,
            alloc_nodes: HashSet::new(),
            inc_nodes: Arena::new(node_dist),
            dec_nodes: Vec::new(),
            mut_nodes: Vec::new(),
            update_nodes: Vec::new(),
        }
    }

    pub fn alloc_node(&mut self) -> NodeIndex {
        let idx = self.inc_nodes.alloc();
        self.alloc_nodes.insert(idx);
        idx
    }

    pub fn fill_back_node(&mut self, idx: NodeIndex, data: NodeT) {
        self.inc_nodes.fill_back(idx, data);
    }

    pub fn new_node(&mut self, data: NodeT) -> NodeIndex {
        self.inc_nodes.insert(data)
    }

    pub fn remove_node(&mut self, node: NodeIndex) {
        if self.inc_nodes.remove(node).is_none() {
            if !self.alloc_nodes.remove(&node) {
                self.dec_nodes.push(node);
            }
        }
    }

    pub fn mut_node<F>(&mut self, node: NodeIndex, func: F)
    where
        F: FnOnce(&mut NodeT) + 'a,
    {
        if self.inc_nodes.contains(node) {
            func(&mut self.inc_nodes.get_mut(node).unwrap());
        } else {
            self.mut_nodes.push((node, Box::new(func)));
        }
    }

    pub fn update_node<F>(&mut self, node: NodeIndex, func: F)
    where
        F: FnOnce(NodeT) -> NodeT + 'a,
    {
        if self.inc_nodes.contains(node) {
            self.inc_nodes.update_with(node, |x| func(x));
        } else {
            self.update_nodes.push((node, Box::new(func)));
        }
    }

    pub fn giveup(&mut self) {
        self.committed = true;
    }
}

#[derive(Debug)]
pub struct Context {
    node_dist: Arc<IdDistributer>,
}
impl Context {
    pub fn new() -> Context {
        Context {
            node_dist: Arc::new(IdDistributer::new()),
        }
    }
}
impl Clone for Context {
    fn clone(&self) -> Self {
        Context {
            node_dist: Arc::clone(&self.node_dist),
        }
    }
}

pub trait SourceIterator<T: TypedNode>: Iterator<Item = (NodeIndex, Self::Source)> {
    type Source: Copy + Clone + Eq + PartialEq + Debug + Hash;
    fn new(node: &T) -> Self;
}
pub trait TypedNode: Sized {
    type Source: Copy + Clone + Eq + PartialEq + Debug + Hash;
    type Iter: SourceIterator<Self, Source = Self::Source>;
    fn iter_source(&self) -> Self::Iter;
    fn modify(&mut self, source: Self::Source, old_idx: NodeIndex, new_idx: NodeIndex);
}

pub trait NodeEnum {
    type SourceEnum: Copy + Clone + Eq + PartialEq + Debug + Hash;
    fn iter_source(&self) -> Box<dyn Iterator<Item = (NodeIndex, Self::SourceEnum)>>;
    fn modify(&mut self, source: Self::SourceEnum, old_idx: NodeIndex, new_idx: NodeIndex);
}
