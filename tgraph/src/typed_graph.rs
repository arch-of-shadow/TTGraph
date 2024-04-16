//! Typed graph
//! A container for a graph-like data structure, where nodes have distinct types.
//! An edge in this graph is a data field in the node.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::Arc;

use uuid::Uuid;

use crate::arena::{self, Arena, ArenaIndex, IdDistributer};

pub mod debug;
pub mod display;
// pub mod library;
pub mod macro_traits;
pub use macro_traits::*;

mod transaction;
pub use transaction::Transaction;

pub mod macros;
// pub use macros::*;
pub use tgraph_macros::*;

/// The index of a node, which implements [`Copy`].
/// Note: The index is very independent to the [`Graph`], which does not check if it is realy pointing to a node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeIndex(pub usize);

impl NodeIndex {
  /// Make an empty index
  /// # Example
  /// ```
  /// use tgraph::NodeIndex;
  /// let a = NodeIndex::empty();
  /// assert_eq!(a, NodeIndex::empty());
  /// ````
  pub fn empty() -> NodeIndex {
    NodeIndex(0)
  }

  /// Check if the index is empty
  ///  /// # Example
  /// ```
  /// use tgraph::NodeIndex;
  /// let a = NodeIndex::empty();
  /// assert!(a.is_empty());
  /// ````
  pub fn is_empty(&self) -> bool {
    self.0 == 0
  }
}

impl ArenaIndex for NodeIndex {
  fn new(id: usize) -> Self {
    NodeIndex(id)
  }
}

impl Display for NodeIndex {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if self.is_empty() {
      write!(f, "empty")
    } else {
      write!(f, "{}", self.0)
    }
  }
}

/// A graph with typed nodes
/// The graph can only by modified by commiting a transaction, which avoids mutable borrow of the graph
///
/// # Example:
///
/// ```rust
/// use tgraph::*;
/// # use std::collections::BTreeSet;
///
/// #[derive(TypedNode)]
/// struct NodeA{
///   link: NodeIndex,
///   data: usize,
/// }
///
/// #[derive(TypedNode)]
/// struct NodeB{
///   links: BTreeSet<NodeIndex>,
///   another_data: String,
/// }
///
/// node_enum!{
///   enum Node{
///     A(NodeA),
///     B(NodeB)
///   }
/// }
///
/// let ctx = Context::new();
/// let mut graph = Graph::<Node>::new(&ctx);
/// let mut trans = Transaction::new(&ctx);
/// // Does some operations on the transaction
/// graph.commit(trans);
/// ```
#[derive(Clone)]
pub struct Graph<NodeT: NodeEnum> {
  ctx_id: Uuid,
  nodes: Arena<NodeIndex, NodeT>,
  back_links: BTreeMap<NodeIndex, BTreeSet<(NodeIndex, NodeT::SourceEnum)>>,
}

impl<NodeT: NodeEnum> Graph<NodeT> {
  /// Create an empty graph
  pub fn new(context: &Context) -> Self {
    Graph {
      ctx_id: context.id,
      nodes: Arena::new(Arc::clone(&context.node_dist)),
      back_links: BTreeMap::new(),
    }
  }

  /// Get the reference of a node. For convinience, if the type of the node is previously known, use [`get_node!()`](crate::get_node!) instead.
  /// # Example
  /// ```
  /// use tgraph::*;
  ///
  /// #[derive(TypedNode)]
  /// struct NodeA{
  ///   data: usize,
  /// }
  ///
  /// node_enum!{
  ///   enum Node{
  ///     A(NodeA)
  ///   }
  /// }
  ///
  /// let ctx = Context::new();
  /// let mut graph = Graph::<Node>::new(&ctx);
  /// let mut trans = Transaction::new(&ctx);
  /// let idx = trans.insert(Node::A(NodeA{
  ///   data: 1
  /// }));
  /// graph.commit(trans);
  ///
  /// // node: Option<&Node>
  /// let node = graph.get(idx);
  /// if let Some(Node::A(node)) = node {
  ///   assert_eq!(node.data, 1);
  /// } else {
  ///   panic!();
  /// }
  ///
  /// assert!(graph.get(NodeIndex::empty()).is_none());
  /// ````
  pub fn get(&self, idx: NodeIndex) -> Option<&NodeT> {
    self.nodes.get(idx)
  }

  /// Iterate all nodes in the graph following the order of NodeIndex.
  /// If only a type of node is wanted, use [`iter_nodes!`](`crate::iter_nodes!`) instead.
  /// # Example
  /// ```
  /// use tgraph::*;
  ///
  /// #[derive(TypedNode)]
  /// struct NodeA{
  ///   a: usize
  /// }
  /// #[derive(TypedNode)]
  /// struct NodeB{
  ///   b: usize
  /// }
  ///
  /// node_enum!{
  ///   enum Node{
  ///     A(NodeA),
  ///     B(NodeB),
  ///   }
  /// }
  ///
  /// let ctx = Context::new();
  /// let mut graph = Graph::<Node>::new(&ctx);
  /// let mut trans = Transaction::new(&ctx);
  ///
  /// trans.insert(Node::A(NodeA{ a: 1 }));
  /// trans.insert(Node::A(NodeA{ a: 2 }));
  /// trans.insert(Node::B(NodeB{ b: 0 }));
  /// graph.commit(trans);
  ///
  /// // iterator.next() returns Option<(NodeIndex, &Node)>
  /// let iterator = graph.iter();
  /// for (i, (_, node)) in (1..3).zip(iterator) {
  ///   if let Node::A(a) = node {
  ///     assert_eq!(i, a.a);
  ///   } else {
  ///     panic!();
  ///   }
  /// }
  /// ```
  pub fn iter(&self) -> Iter<'_, NodeT> {
    self.nodes.iter()
  }

  /// Iterate all nodes within the named group
  /// # Example
  /// ```
  /// use tgraph::*;
  /// #[derive(TypedNode, Debug)]
  /// struct NodeA {
  ///   a: usize,
  /// }
  /// #[derive(TypedNode, Debug)]
  /// struct NodeB {
  ///   b: usize,
  /// }
  /// #[derive(TypedNode, Debug)]
  /// struct NodeC {
  ///   c: usize,
  /// }
  /// #[derive(TypedNode, Debug)]
  /// struct NodeD {
  ///   d: usize,
  /// }
  ///
  /// node_enum! {
  ///   #[derive(Debug)]
  ///   enum MultiNodes{
  ///     A(NodeA),
  ///     B(NodeB),
  ///     C(NodeC),
  ///     D(NodeD),
  ///   }
  ///   group!{
  ///     first{A, B},
  ///     second{C, D},
  ///     third{A, D},
  ///     one{B},
  ///     all{A, B, C, D},
  ///   }
  /// }
  ///
  ///  let ctx = Context::new();
  ///  let mut graph = Graph::<MultiNodes>::new(&ctx);
  ///  let mut trans = Transaction::new(&ctx);
  ///  let a = trans.insert(MultiNodes::A(NodeA { a: 1 }));
  ///  let b = trans.insert(MultiNodes::B(NodeB { b: 2 }));
  ///  let c = trans.insert(MultiNodes::C(NodeC { c: 3 }));
  ///  let d = trans.insert(MultiNodes::D(NodeD { d: 4 }));
  ///  graph.commit(trans);
  ///
  ///  assert_eq!(Vec::from_iter(graph.iter_group("first").map(|(x, _)| x)), vec![a, b]);
  ///  assert_eq!(Vec::from_iter(graph.iter_group("second").map(|(x, _)| x)), vec![c, d]);
  ///  assert_eq!(Vec::from_iter(graph.iter_group("third").map(|(x, _)| x)), vec![a, d]);
  ///  assert_eq!(Vec::from_iter(graph.iter_group("one").map(|(x, _)| x)), vec![b]);
  ///  assert_eq!(Vec::from_iter(graph.iter_group("all").map(|(x, _)| x)), vec![a, b, c, d]);
  /// ```
  pub fn iter_group(
    &self, name: &'static str,
  ) -> impl Iterator<Item = (NodeIndex, &NodeT)> {
    self.iter().filter(move |(_, n)| n.in_group(name))
  }

  /// Get the number of nodes in a graph
  /// # Example
  /// ```
  /// use tgraph::*;
  /// #[derive(TypedNode)]
  /// struct NodeA{
  ///   data: usize,
  /// }
  /// node_enum!{
  ///   enum Node{
  ///     A(NodeA)
  ///   }
  /// }
  ///
  /// let ctx = Context::new();
  /// let mut graph = Graph::<Node>::new(&ctx);
  /// assert_eq!(graph.len(), 0);
  /// let mut trans = Transaction::new(&ctx);
  /// trans.insert(Node::A(NodeA{data: 1}));
  /// trans.insert(Node::A(NodeA{data: 1}));
  /// trans.insert(Node::A(NodeA{data: 1}));
  /// graph.commit(trans);
  /// assert_eq!(graph.len(), 3);
  /// ```
  pub fn len(&self) -> usize {
    self.nodes.len()
  }

  /// Check if the graph has no node
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Commit an [`Transaction`] to modify the graph
  /// Operation order:
  /// + Redirect nodes
  /// + Insert new nodes
  /// + Modify nodes
  /// + Update nodes
  /// + Redirect all nodes
  /// + Remove nodes
  /// + Add/Remove links due to bidirectional declaration
  /// # Panics
  /// Panic if the transaction and the graph have different context
  /// # Example
  /// ```
  /// use tgraph::*;
  /// #[derive(TypedNode)]
  /// struct NodeA{
  ///   data: usize,
  /// }
  /// node_enum!{
  ///   enum Node{
  ///     A(NodeA)
  ///   }
  /// }
  ///
  /// let ctx = Context::new();
  /// let mut graph = Graph::<Node>::new(&ctx);
  /// let mut trans = Transaction::new(&ctx);
  /// trans.insert(Node::A(NodeA{data: 1}));
  /// graph.commit(trans);
  /// ```
  pub fn commit(&mut self, t: Transaction<NodeT>) {
    assert!(
      t.ctx_id == self.ctx_id,
      "The transaction and the graph are from different context!"
    );
    assert!(t.alloc_nodes.is_empty(), "There are unfilled allocated nodes");

    let mut bd = BidirectionLinkContainer::default();

    self.redirect_links_vec(t.redirect_links_vec, &mut bd);
    self.merge_nodes(t.inc_nodes, &mut bd);
    for (i, f) in t.mut_nodes {
      self.modify_node(i, f, &mut bd)
    }
    for (i, f) in t.update_nodes {
      self.update_node(i, f, &mut bd)
    }
    self.redirect_links_vec(t.redirect_all_links_vec, &mut bd);
    for n in &t.dec_nodes {
      self.remove_node(*n, &mut bd);
    }

    self.apply_bidirectional_links(bd);
  }

  /// Switch the context and relabel the node ids.
  /// # Usecase:
  /// + Useful when there are a lot of removed [`NodeIndex`], and after context switching the indexes will be more concise.
  /// + Merge two graphs with different context. See [`merge`](Transaction::merge) for example.
  /// # Warning:
  /// + Please ensure there is no uncommitted transactions!
  /// + [`NodeIndex`] pointing to this graph is useless after context switching!
  pub fn switch_context(self, new_ctx: &Context) -> Self {
    let mut new_nodes = Arena::new(Arc::clone(&new_ctx.node_dist));
    let mut id_map = BTreeMap::new();

    for (id, x) in self.nodes {
      id_map.insert(id, new_nodes.insert(x));
    }

    for (id, new_id) in &id_map {
      for (y, s) in &self.back_links[id] {
        new_nodes.get_mut(id_map[&y]).unwrap().modify_link(*s, *id, *new_id);
      }
    }

    let mut result = Graph {
      ctx_id: new_ctx.id,
      nodes: Arena::new(Arc::clone(&new_ctx.node_dist)),
      back_links: BTreeMap::new(),
    };

    let mut bd = BidirectionLinkContainer::default();
    result.merge_nodes(new_nodes, &mut bd);
    result.apply_bidirectional_links(bd);
    result
  }

  /// Check if the backlinks are connected correctly, just for debug
  #[doc(hidden)]
  pub fn check_backlinks(&self) {
    let mut back_links: BTreeMap<NodeIndex, BTreeSet<(NodeIndex, NodeT::SourceEnum)>> =
      BTreeMap::new();
    for (x, n) in &self.nodes {
      back_links.entry(x).or_default();
      for (y, s) in n.iter_sources() {
        back_links.entry(y).or_default().insert((x, s));
        let links = self
          .back_links
          .get(&y)
          .unwrap_or_else(|| panic!("Node {} have no backlink!", x.0));
        assert!(links.contains(&(x, s)));
      }
    }
    assert_eq!(back_links, self.back_links);
  }

  fn merge_nodes(
    &mut self, nodes: Arena<NodeIndex, NodeT>, bd: &mut BidirectionLinkContainer<NodeT>,
  ) {
    for (x, n) in &nodes {
      self.add_back_links(x, n);
    }
    for (id, node) in &nodes {
      for (ys, lms) in node.get_bidiretional_links() {
        bd.add(id, ys, &lms);
      }
    }
    self.nodes.merge(nodes);
  }

  fn remove_node(&mut self, x: NodeIndex, bd: &mut BidirectionLinkContainer<NodeT>) {
    self.remove_bidirectional_link(x, bd);
    let n = self.nodes.remove(x).expect("Remove a non-existing node!");
    self.remove_back_links(x, &n);
    for (y, s) in self.back_links.remove(&x).unwrap() {
      self.nodes.get_mut(y).unwrap().modify_link(s, x, NodeIndex::empty());
    }
  }

  fn modify_node<F>(
    &mut self, x: NodeIndex, f: F, bd: &mut BidirectionLinkContainer<NodeT>,
  ) where
    F: FnOnce(&mut NodeT),
  {
    self.remove_bidirectional_link(x, bd);
    for (y, s) in self.nodes.get(x).unwrap().iter_sources() {
      self.back_links.get_mut(&y).unwrap().remove(&(x, s));
    }

    f(self.nodes.get_mut(x).unwrap());

    self.add_bidirectional_link(x, bd);
    for (y, s) in self.nodes.get(x).unwrap().iter_sources() {
      self.back_links.get_mut(&y).unwrap().insert((x, s));
    }
  }

  fn update_node<F>(
    &mut self, x: NodeIndex, f: F, bd: &mut BidirectionLinkContainer<NodeT>,
  ) where
    F: FnOnce(NodeT) -> NodeT,
  {
    self.remove_bidirectional_link(x, bd);
    for (y, s) in self.nodes.get(x).unwrap().iter_sources() {
      self.back_links.get_mut(&y).unwrap().remove(&(x, s));
    }

    self.nodes.update_with(x, f);

    self.add_bidirectional_link(x, bd);
    for (y, s) in self.nodes.get(x).unwrap().iter_sources() {
      self.back_links.get_mut(&y).unwrap().insert((x, s));
    }
  }

  fn add_bidirectional_link(
    &mut self, x: NodeIndex, bd: &mut BidirectionLinkContainer<NodeT>,
  ) {
    let to_add = self.nodes.get(x).unwrap().get_bidiretional_links();
    for (ys, lms) in to_add {
      bd.add(x, ys, &lms);
    }
  }

  fn remove_bidirectional_link(
    &mut self, x: NodeIndex, bd: &mut BidirectionLinkContainer<NodeT>,
  ) {
    let to_remove = self.nodes.get(x).unwrap().get_bidiretional_links();
    for (ys, lms) in to_remove {
      bd.remove(x, ys, &lms);
    }
  }

  fn redirect_links(
    &mut self, old_node: NodeIndex, new_node: NodeIndex,
    bd: &mut BidirectionLinkContainer<NodeT>,
  ) {
    let old_link = self.back_links.remove(&old_node).unwrap();
    self.back_links.insert(old_node, BTreeSet::new());

    let new_link = self.back_links.entry(new_node).or_default();
    for (y, s) in old_link {
      new_link.insert((y, s));
      let side_effect = self.nodes.get_mut(y).unwrap().modify_link(s, old_node, new_node);
      if !side_effect.add.is_empty() {
        bd.add_one(y, side_effect.add, &side_effect.link_mirrors);
      }
      if !side_effect.remove.is_empty() {
        bd.remove_one(y, side_effect.remove, &side_effect.link_mirrors);
      }
    }
  }

  fn redirect_links_vec(
    &mut self, replacements: Vec<(NodeIndex, NodeIndex)>,
    bd: &mut BidirectionLinkContainer<NodeT>,
  ) {
    let mut fa = BTreeMap::new();

    for (old, new) in &replacements {
      fa.entry(*old).or_insert(*old);
      fa.entry(*new).or_insert(*new);
    }

    for (old, new) in &replacements {
      let mut x = *new;
      while fa[&x] != x {
        x = fa[&x];
      }
      assert!(x != *old, "Loop redirection detected!");
      *fa.get_mut(old).unwrap() = x;
    }

    for (old, new) in &replacements {
      let mut x = *new;
      let mut y = fa[&x];
      while x != y {
        x = y;
        y = fa[&y];
      }

      self.redirect_links(*old, x, bd);

      x = *new;
      while fa[&x] != y {
        let z = fa[&x];
        *fa.get_mut(&x).unwrap() = y;
        x = z;
      }
    }
  }

  fn apply_bidirectional_links(&mut self, bd: BidirectionLinkContainer<NodeT>) {
    for (x, y, l) in bd.to_remove {
      if self.nodes.contains(x)
        && self.nodes.contains(y)
        && self.nodes.get_mut(y).unwrap().remove_link(l, x)
      {
        self.remove_back_link(y, x, NodeT::to_source_enum(&l));
      }
    }
    for (x, y, l) in bd.to_add {
      if self.nodes.contains(x)
        && self.nodes.contains(y)
        && self.nodes.get_mut(y).unwrap().add_link(l, x)
      {
        self.add_back_link(y, x, NodeT::to_source_enum(&l));
      }
    }
  }

  fn add_back_link(&mut self, x: NodeIndex, y: NodeIndex, src: NodeT::SourceEnum) {
    self.back_links.entry(y).or_default().insert((x, src));
  }

  fn add_back_links(&mut self, x: NodeIndex, n: &NodeT) {
    self.back_links.entry(x).or_default();
    for (y, s) in n.iter_sources() {
      self.back_links.entry(y).or_default().insert((x, s));
    }
  }

  fn remove_back_link(&mut self, x: NodeIndex, y: NodeIndex, src: NodeT::SourceEnum) {
    self.back_links.get_mut(&y).unwrap().remove(&(x, src));
  }

  fn remove_back_links(&mut self, x: NodeIndex, n: &NodeT) {
    for (y, s) in n.iter_sources() {
      self.back_links.get_mut(&y).unwrap().remove(&(x, s));
    }
  }
}

// Helper struct for bidirectional links
struct BidirectionLinkContainer<NodeT: NodeEnum> {
  to_add: BTreeSet<(NodeIndex, NodeIndex, NodeT::LinkMirrorEnum)>,
  to_remove: BTreeSet<(NodeIndex, NodeIndex, NodeT::LinkMirrorEnum)>,
}
impl<NodeT: NodeEnum> BidirectionLinkContainer<NodeT> {
  fn add_one(&mut self, x: NodeIndex, y: NodeIndex, lms: &Vec<NodeT::LinkMirrorEnum>) {
    for l in lms {
      if self.to_remove.contains(&(x, y, *l)) {
        self.to_remove.remove(&(x, y, *l));
      } else {
        self.to_add.insert((x, y, *l));
      }
    }
  }

  fn add(&mut self, x: NodeIndex, ys: Vec<NodeIndex>, lms: &Vec<NodeT::LinkMirrorEnum>) {
    for y in ys {
      self.add_one(x, y, lms)
    }
  }

  fn remove_one(&mut self, x: NodeIndex, y: NodeIndex, lms: &Vec<NodeT::LinkMirrorEnum>) {
    for l in lms {
      if self.to_add.contains(&(x, y, *l)) {
        self.to_add.remove(&(x, y, *l));
      } else {
        self.to_remove.insert((x, y, *l));
      }
    }
  }

  fn remove(
    &mut self, x: NodeIndex, ys: Vec<NodeIndex>, lms: &Vec<NodeT::LinkMirrorEnum>,
  ) {
    for y in ys {
      self.remove_one(x, y, lms);
    }
  }
}
impl<NodeT: NodeEnum> Default for BidirectionLinkContainer<NodeT> {
  fn default() -> Self {
    BidirectionLinkContainer {
      to_add: BTreeSet::new(),
      to_remove: BTreeSet::new(),
    }
  }
}

type Iter<'a, NodeT> = arena::Iter<'a, NodeIndex, NodeT>;
type IntoIter<NodeT> = arena::IntoIter<NodeIndex, NodeT>;

impl<T: NodeEnum> IntoIterator for Graph<T> {
  type IntoIter = IntoIter<T>;
  type Item = (NodeIndex, T);

  fn into_iter(self) -> Self::IntoIter {
    self.nodes.into_iter()
  }
}

/// Type alias to be used in [`mutate`](Transaction::mutate), intented to be used in macros
pub type MutFunc<'a, T> = Box<dyn FnOnce(&mut T) + 'a>;
/// Type alias to be used in [`update`](Transaction::update), intented to be used in macros
pub type UpdateFunc<'a, T> = Box<dyn FnOnce(T) -> T + 'a>;

/// Context for typed graph
/// Transactions and graph must have the same context to ensure the correctness of NodeIndex
#[derive(Debug)]
pub struct Context {
  id: Uuid,
  node_dist: Arc<IdDistributer>,
}
impl Context {
  /// Create a new context
  pub fn new() -> Context {
    Context {
      id: Uuid::new_v4(),
      node_dist: Arc::new(IdDistributer::new()),
    }
  }
}
impl Default for Context {
  fn default() -> Self {
    Self::new()
  }
}
impl Clone for Context {
  fn clone(&self) -> Self {
    Context {
      id: self.id,
      node_dist: Arc::clone(&self.node_dist),
    }
  }
}

/// A trait intended to be used in macros
pub trait SourceIterator<T: TypedNode>:
  Iterator<Item = (NodeIndex, Self::Source)>
{
  type Source: Copy + Clone + Eq + PartialEq + Debug + Hash + PartialOrd + Ord;
  fn new(node: &T) -> Self;
}
