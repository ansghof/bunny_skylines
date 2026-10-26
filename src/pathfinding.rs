//! Wegfindung per Breitensuche über Oberfläche und Gänge.
//! Die Karte ist klein genug, dass das pro Entscheidung schnell genug ist;
//! für viele Kaninchen kommen später Flow-Fields.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

use crate::map::{Grid, Layer, TilePos};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node {
    pub layer: Layer,
    pub pos: TilePos,
}

impl Node {
    pub const fn new(layer: Layer, pos: TilePos) -> Self {
        Self { layer, pos }
    }
}

/// Erreichbare Nachbarn: vier Richtungen in derselben Ebene, an Eingängen zusätzlich die andere Ebene.
pub fn neighbors(grid: &Grid, node: Node) -> impl Iterator<Item = Node> + '_ {
    let same_layer = node
        .pos
        .neighbors()
        .filter(move |p| grid.walkable(node.layer, *p))
        .map(move |p| Node::new(node.layer, p));
    let through_entrance = (grid.surface(node.pos).entrance && grid.under(node.pos).dug.is_some())
        .then(|| Node::new(node.layer.other(), node.pos));
    same_layer.chain(through_entrance)
}

/// Kürzester Weg zum nächsten Knoten, für den `is_goal` zutrifft.
/// Der Weg enthält den Start nicht, aber das Ziel; ist der Start selbst das Ziel, ist er leer.
pub fn find_path(grid: &Grid, start: Node, is_goal: impl FnMut(Node) -> bool) -> Option<Vec<Node>> {
    find_path_where(grid, start, |_| true, is_goal)
}

/// Wie `find_path`, betritt aber nur Knoten, für die `allowed` zutrifft, z. B. um einem Marder auszuweichen.
pub fn find_path_where(
    grid: &Grid,
    start: Node,
    allowed: impl Fn(Node) -> bool,
    mut is_goal: impl FnMut(Node) -> bool,
) -> Option<Vec<Node>> {
    let mut came_from: HashMap<Node, Node> = HashMap::new();
    let mut queue = VecDeque::from([start]);
    came_from.insert(start, start);

    while let Some(node) = queue.pop_front() {
        if is_goal(node) {
            let mut path = Vec::new();
            let mut current = node;
            while current != start {
                path.push(current);
                current = came_from[&current];
            }
            path.reverse();
            return Some(path);
        }
        for next in neighbors(grid, node).filter(|next| allowed(*next)) {
            if let Entry::Vacant(entry) = came_from.entry(next) {
                entry.insert(node);
                queue.push_back(next);
            }
        }
    }
    None
}
