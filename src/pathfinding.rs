//! Wegfindung über Oberfläche und Gänge; Trampelpfade sind schneller und werden bevorzugt.
//! Die Karte ist klein genug, dass das pro Entscheidung schnell genug ist;
//! für viele Kaninchen kommen später Flow-Fields.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::behavior::TRAIL_SPEED;
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

/// Was ein Schritt auf diesen Knoten kostet: Auf dem Trampelpfad ist man schneller, also ist er billiger.
fn step_cost(grid: &Grid, node: Node) -> u32 {
    if node.layer == Layer::Surface && grid.surface(node.pos).trail {
        // Kehrwert des Tempos, in Hundertsteln.
        (100.0 / TRAIL_SPEED).round() as u32
    } else {
        100
    }
}

/// Wie `find_path`, betritt aber nur Knoten, für die `allowed` zutrifft, z. B. um einem Marder auszuweichen.
/// Gesucht wird der schnellste Weg, nicht der mit den wenigsten Feldern: Trampelpfade zählen weniger.
/// `is_goal` wird für jeden erreichten Knoten genau einmal gefragt, vom nächsten zum fernsten.
pub fn find_path_where(
    grid: &Grid,
    start: Node,
    allowed: impl Fn(Node) -> bool,
    mut is_goal: impl FnMut(Node) -> bool,
) -> Option<Vec<Node>> {
    let mut came_from: HashMap<Node, Node> = HashMap::new();
    let mut best: HashMap<Node, u32> = HashMap::from([(start, 0)]);
    let mut done: HashSet<Node> = HashSet::new();
    // Gleich teure Knoten in der Reihenfolge, in der sie gefunden wurden; die Laufnummer zeigt in `found`.
    let mut found = vec![start];
    let mut queue = BinaryHeap::from([(Reverse(0u32), Reverse(0usize))]);

    while let Some((Reverse(cost), Reverse(index))) = queue.pop() {
        let node = found[index];
        if !done.insert(node) {
            continue;
        }
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
            if done.contains(&next) {
                continue;
            }
            let next_cost = cost + step_cost(grid, next);
            if best.get(&next).is_none_or(|known| next_cost < *known) {
                best.insert(next, next_cost);
                came_from.insert(next, node);
                found.push(next);
                queue.push((Reverse(next_cost), Reverse(found.len() - 1)));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(x: i32, y: i32) -> Node {
        Node::new(Layer::Surface, TilePos::new(x, y))
    }

    #[test]
    fn without_trails_the_shortest_way() {
        let grid = Grid::starting_meadow();
        let path = find_path(&grid, surface(11, 12), |n| n == surface(27, 12)).unwrap();
        assert_eq!(path.len(), 16);
    }

    #[test]
    fn a_long_trail_is_worth_a_detour() {
        let mut grid = Grid::starting_meadow();
        for x in 11..=27 {
            grid.surface_mut(TilePos::new(x, 13)).trail = true;
        }
        let path = find_path(&grid, surface(11, 12), |n| n == surface(27, 12)).unwrap();
        assert!(path.iter().any(|n| grid.surface(n.pos).trail), "der Weg sollte den Pfad nutzen");
    }

    #[test]
    fn a_short_trail_is_not() {
        let mut grid = Grid::starting_meadow();
        for x in 12..=14 {
            grid.surface_mut(TilePos::new(x, 13)).trail = true;
        }
        let path = find_path(&grid, surface(11, 12), |n| n == surface(15, 12)).unwrap();
        assert_eq!(path.len(), 4);
    }
}
