use crate::ast::{ExecutionMode, PropertyFilter};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum OpCode {
    SetExecutionMode(ExecutionMode),
    TraverseEdge {
        name: String,
        required: bool,
    },
    CreateNode {
        alias: String,
        label: Option<String>,
        properties: HashMap<String, String>,
    },
    // --- MET À JOUR CE BLOC ---
    LoadNode {
        alias: String,
        label: Option<String>,
        properties_filters: Vec<PropertyFilter>, // <-- AJOUTE CETTE LIGNE
    },
    // --------------------------
    CreateEdge {
        source: String,
        target: String,
        name: String,
        properties: HashMap<String, String>,
    },
    GpuVectorFilter {
        target: String,
        op: String,
        threshold: f32,
    },
    SetSentinelNode,
    StoreResult {
        target_alias: String,
    },
}
