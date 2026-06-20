#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionMode {
    Strict,   // Recherche topologique exacte et rapide (Dijkstra brut, etc.)
    Semantic, // Utilisation intensive du GPU Compute (wgpu) pour la similarité cosinus
    Hybrid,   // Topologie d'abord, puis filtrage sémantique sur le sous-graphe
}

// --- AJOUT : Les modificateurs pour le filtrage géométrique ---
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Modifier {
    Required, // Représente le *
    Optional, // Représente le ?
    Opposed,  // Représente le !
    None,     // Pas de modificateur
}

// --- AJOUT : La structure de filtre de propriété cible ---
#[derive(Debug, Clone)]
pub struct PropertyFilter {
    pub key_id: String,          // La clé (ex: "ville")
    pub target_value_id: String, // La valeur attendue (ex: "Paris")
    pub modifier: Modifier,      // Le modificateur associé
}

#[derive(Debug, Clone)]
pub struct NodeExpr {
    pub alias: String,
    pub label: Option<String>,
    // On remplace l'ancien HashMap simple par notre liste de filtres avancés !
    pub properties_filters: Vec<PropertyFilter>,
}

#[derive(Debug, Clone)]
pub struct EdgeExpr {
    pub name: String,                     // Ex: "amis"
    pub modifier: Option<char>, // Some('*') pour coupe-circuit, Some('?') pour fallback, None pour strict
    pub vec_filter: Option<VectorFilter>, // Filtre sémantique associé
}

#[derive(Debug, Clone)]
pub struct VectorFilter {
    pub target: String,   // Ex: "similarite"
    pub operator: String, // Ex: ">"
    pub value: f32,       // Ex: 0.85
}

#[derive(Debug, Clone)]
pub enum Command {
    Add(AddExpression),
    Get {
        start: NodeExpr,
        path: Vec<(EdgeExpr, NodeExpr)>, // Liste ordonnée de (Arête, Nœud d'arrivée)
    },
    Configure {
        mode: ExecutionMode,
    },
}

/// Nouvelle structure racine représentant un script .ji complet composé de plusieurs lignes
#[derive(Debug, Clone)]
pub struct Program {
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub enum AddExpression {
    // Cas 1 : ADD (alias:Label { props })
    Node(NodeExpr),

    // Cas 2 : ADD (source)-[relation]->(target)
    Edge {
        source: String,
        target: String,
        name: String,
    },
}
