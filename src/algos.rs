use crate::store::GraphStore;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

// Structure interne pour la file de priorité de Dijkstra
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: u32, // Le temps de trajet cumulé en minutes
    position: &'static str,
}

// On inverse l'ordre pour que la BinaryHeap de Rust devienne un Min-Heap (le plus petit coût en premier)
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| self.position.cmp(&other.position))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl GraphStore {
    /// Calcule le chemin le plus rapide entre deux gares en utilisant Dijkstra
    pub fn chemin_le_plus_court(&self, start: &str, target: &str) -> Option<(Vec<String>, u32)> {
        let mut distances: HashMap<&str, u32> = HashMap::new();
        let mut predecessors: HashMap<&str, &str> = HashMap::new();
        let mut heap = BinaryHeap::new();

        // Initialisation
        distances.insert(start, 0);
        // On doit ruser avec les lifetimes ici ou utiliser des String possédées
        // Pour l'exemple, on passe par l'ID existant dans le store
        let start_id = self.nodes.get(start)?.id.as_str();

        heap.push(State {
            cost: 0,
            position: start_id,
        });

        while let Some(State { cost, position }) = heap.pop() {
            // Si on a atteint la cible, on s'arrête
            if position == target {
                break;
            }

            // Si on trouve un coût supérieur à ce qu'on connaît déjà, on passe
            if let Some(&best) = distances.get(position) {
                if cost > best {
                    continue;
                }
            }

            // On explore les gares voisines (les arêtes sortantes)
            if let Some(edges) = self.edges.get(position) {
                for edge in edges {
                    // On extrait le poids (temps_min), par défaut 60 mins si absent
                    let weight: u32 = edge
                        .properties
                        .get("temps_min")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(60);

                    let next_cost = cost + weight;
                    let target_node_id = edge.target_id.as_str();

                    // Si on trouve un chemin plus rapide vers le voisin
                    if next_cost < *distances.get(target_node_id).unwrap_or(&u32::MAX) {
                        distances.insert(target_node_id, next_cost);
                        predecessors.insert(target_node_id, position);
                        heap.push(State {
                            cost: next_cost,
                            position: target_node_id,
                        });
                    }
                }
            }
        }

        // Reconstruction du chemin à rebours
        let total_time = *distances.get(target)?;
        let mut path = Vec::new();
        let mut current = target;

        path.push(current.to_string());
        while let Some(&pred) = predecessors.get(current) {
            path.push(pred.to_string());
            current = pred;
        }
        path.reverse();

        Some((path, total_time))
    }
}
