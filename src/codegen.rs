use crate::ast::{Command, ExecutionMode, Program};
use crate::bytecode::OpCode;
use std::collections::HashMap;
pub struct CodeGenerator {
    pub bytecode: Vec<OpCode>,
}

impl CodeGenerator {
    pub fn new() -> Self {
        CodeGenerator {
            bytecode: Vec::new(),
        }
    }
    /// Génère le bytecode complet pour un programme multi-ligne
    pub fn generate(&mut self, program: Program, mode: ExecutionMode) -> Vec<OpCode> {
        self.bytecode.push(OpCode::SetExecutionMode(mode));
        for command in program.commands {
            match command {
                Command::Add(node) => match node {
                    crate::ast::AddExpression::Node(n) => {
                        // Convertit le Vec<PropertyFilter> en HashMap<String, String> pour le stockage brut
                        let mut props_map = HashMap::new();
                        for filter in n.properties_filters {
                            props_map.insert(filter.key_id, filter.target_value_id);
                        }

                        self.bytecode.push(OpCode::CreateNode {
                            alias: n.alias,
                            label: n.label,
                            properties: props_map,
                        });
                    }
                    crate::ast::AddExpression::Edge {
                        source,
                        target,
                        name,
                        properties,
                    } => {
                        let mut edge_props = HashMap::new();
                        for filter in properties {
                            edge_props.insert(filter.key_id, filter.target_value_id);
                        }

                        // 2. On passe la map à l'OpCode
                        self.bytecode.push(OpCode::CreateEdge {
                            source,
                            target,
                            name,
                            properties: edge_props, // Avant, ce champ manquait dans ton OpCode !
                        });
                    }
                },
                Command::Get { start, path } => {
                    // 1. On charge le nœud de départ initial avec ses filtres géométriques !
                    self.bytecode.push(OpCode::LoadNode {
                        alias: start.alias,
                        label: start.label,
                        properties_filters: start.properties_filters, // <-- TRANSMISSION DES FILTRES
                    });

                    // 2. Pour chaque saut dans le chemin profond
                    for (edge, node) in &path {
                        let required = edge.modifier.is_none();

                        // Émet l'instruction de traversée de l'arête
                        self.bytecode.push(OpCode::TraverseEdge {
                            name: edge.name.clone(),
                            required,
                        });

                        // Charge le nœud d'arrivée avec ses propres filtres
                        self.bytecode.push(OpCode::LoadNode {
                            alias: node.alias.clone(),
                            label: node.label.clone(),
                            properties_filters: node.properties_filters.clone(), // <-- TRANSMISSION DES FILTRES
                        });
                    }
                }
                Command::Configure { mode } => {
                    self.bytecode.push(OpCode::SetExecutionMode(mode));
                }
            }
        }
        self.bytecode.clone()
    }
}
