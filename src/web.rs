use crate::store::{GraphStore, LOCAL_STORAGE};
use axum::{
    Json, Router,
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
};
use std::fs;
use tower_http::cors::CorsLayer;

const INDEX_HTML: &str = include_str!("../ui/index.html");

use axum::{http::StatusCode, response::Response};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Erreur de sérialisation/désérialisation : {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Erreur d'accès fichier : {0}")]
    Io(#[from] std::io::Error),

    #[error("Le nœud avec l'ID {0} est introuvable")]
    NodeNotFound(String),

    #[error("Accès concurrent impossible (Poison Error)")]
    LockError,
}

// Permet à Axum de transformer automatiquement cette erreur en réponse HTTP
impl IntoResponse for DatabaseError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            DatabaseError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            DatabaseError::Serialization(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Une erreur interne est survenue".to_string(),
            ),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct WebQuery {
    db: String,
    env: String,
}

#[derive(serde::Serialize)]
pub struct AvailableDB {
    db_name: String,
    environments: Vec<String>,
}

/// Route pour servir l'interface graphique embarquée
async fn render_ui() -> impl IntoResponse {
    // On enveloppe la chaîne statique dans une structure Html d'Axum
    // pour qu'il injecte automatiquement le header Content-Type: text/html
    Html(INDEX_HTML)
}

async fn list_databases() -> Json<Vec<AvailableDB>> {
    let home = std::env::var("HOME").expect("not unix");
    let base = LOCAL_STORAGE.replace("%home%", &home);
    let base_dir = std::path::Path::new(&base).join("databases");
    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let db_name = entry.file_name().to_string_lossy().into_owned();
                let mut envs = Vec::new();
                if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                    for sub_entry in sub_entries.flatten() {
                        if sub_entry.path().is_dir() {
                            envs.push(sub_entry.file_name().to_string_lossy().into_owned());
                        }
                    }
                }
                result.push(AvailableDB {
                    db_name,
                    environments: envs,
                });
            }
        }
    }
    Json(result)
}

async fn get_graph_data(Query(params): Query<WebQuery>) -> Json<serde_json::Value> {
    let dummy = GraphStore::new();
    let storage_dir = dummy.get_secure_storage_dir(&params.db, &params.env);
    let db_path = storage_dir.join(format!("{}.ji", params.env));

    if !db_path.exists() {
        return Json(serde_json::json!({ "error": "Base introuvable", "nodes": [], "edges": [] }));
    }

    let store = GraphStore::load_from_file(&db_path).unwrap_or_else(|_| GraphStore::new());

    let mut nodes = Vec::new();
    for (id, node) in &store.nodes {
        // 1. On crée le squelette de base du nœud pour Cytoscape
        let mut node_json = serde_json::json!({
            "data": {
                "id": id,
                "label": node.label.as_deref().unwrap_or("Record"),
                "nom": node.properties.get("nom").cloned().unwrap_or(id.clone())
            }
        });

        // 2. FUSION POLYMORPHE : On injecte dynamiquement toutes les autres propriétés
        if let Some(obj) = node_json["data"].as_object_mut() {
            for (key, val) in &node.properties {
                // Évite d'écraser le champ "nom" si déjà présent
                if key != "nom" {
                    obj.insert(key.clone(), serde_json::Value::String(val.clone()));
                }
            }
        }

        nodes.push(node_json);
    }

    let mut edges = Vec::new();
    let mut edge_id_counter = 0;
    for (source, connections) in &store.edges {
        for edge in connections {
            edge_id_counter += 1;
            edges.push(serde_json::json!({
                "data": {
                    "id": format!("e{}", edge_id_counter),
                    "source": source,
                    "target": edge.target_id,
                    "label": edge.relation_name
                }
            }));
        }
    }

    Json(serde_json::json!({ "nodes": nodes, "edges": edges }))
}

pub async fn start_web_server() {
    let app = Router::new()
        .route("/", get(render_ui))
        .route("/api/graph", get(get_graph_data))
        .route("/api/databases", get(list_databases))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9877")
        .await
        .unwrap();
    println!("\x1b[1;32m✓\x1b[0m Serveur JI Quantum actif sur http://127.0.0.1:9877");
    axum::serve(listener, app).await.unwrap();
}
