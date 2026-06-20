use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use crate::web::DatabaseError;
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

pub const LOCAL_STORAGE: &str = "%home%/.local/share/ji";
pub const USB_STORAGE: &str = "/run/media/%user%/JI";

/// Signature binaire de niveau militaire pour valider l'authenticité du fichier .ji ("JIDB")
const MAGIC_NUMBER: [u8; 4] = [0x4A, 0x49, 0x44, 0x42];

#[derive(serde::Deserialize)]
struct DeployTarget {
    name: String,
    ip: String,
    user: String,
    remote_dir: String,
}

#[derive(serde::Deserialize)]
struct DeployConfig {
    targets: Vec<DeployTarget>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeData {
    pub id: String,
    pub label: Option<String>,
    pub properties: HashMap<String, String>,
    pub vector: Option<Vec<f32>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EdgeData {
    pub target_id: String,
    pub relation_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphStore {
    pub nodes: HashMap<String, NodeData>,
    pub edges: HashMap<String, Vec<EdgeData>>,
}

impl GraphStore {
    /// Initialise un stockage de graphe vide en mémoire
    pub fn new() -> Self {
        GraphStore {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }
    }
    pub fn atomic_save(&self, path: &Path) -> Result<(), DatabaseError> {
        // 1. Définir le chemin temporaire
        let tmp_path = path.with_extension("ji.tmp");

        // 2. Ouvrir le fichier temporaire et y sérialiser les données actuelles
        let mut tmp_file = File::create(&tmp_path)?;
        let serialized_data = serde_json::to_vec(&self)?; // Ou bincode / ton format binaire
        tmp_file.write_all(&serialized_data)?;

        // 3. Forcer la synchronisation avec le disque (flush physique)
        tmp_file.sync_all()?;

        // 4. Renommer de manière atomique (remplace l'ancien fichier .ji de façon sécurisée)
        fs::rename(tmp_path, path)?;

        Ok(())
    }
    pub fn load_or_new(db: &str, env: &str) -> Self {
        let dummy = GraphStore::new();
        let storage_dir = dummy.get_secure_storage_dir(db, env);
        let db_path = storage_dir.join(format!("{}.ji", env));

        if db_path.exists() {
            // Tente de charger, sinon retourne une nouvelle instance
            match GraphStore::load_from_file(&db_path) {
                Ok(store) => store,
                Err(_) => GraphStore::new(),
            }
        } else {
            GraphStore::new()
        }
    }
    pub fn deploy_to_remotes(&self, db: &str, env: &str) -> Result<(), String> {
        let dummy = GraphStore::new();
        let storage_dir = dummy.get_secure_storage_dir(db, env);
        let local_file = storage_dir.join(format!("{env}.ji"));

        if !local_file.exists() {
            return Err(format!(
                "Le fichier local à déployer n'existe pas : {:?}",
                local_file
            ));
        }

        let config_path = storage_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("deploy.yaml");
        if !config_path.exists() {
            return Err(
                "Fichier de configuration 'deploy.yaml' introuvable à la racine".to_string(),
            );
        }

        let config_str = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        let config: DeployConfig = serde_yaml::from_str(&config_str).map_err(|e| e.to_string())?;

        println!(
            "\x1b[1;36m  Deploying\x1b[0m targets for database \x1b[1;33m{}\x1b[0m [\x1b[1;32m{}\x1b[0m]",
            db, env
        );
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );

        let mut success_count = 0;
        let total_targets = config.targets.len();
        for target in config.targets {
            // 1. On affiche l'état initial sans retour à la ligne
            print!(
                "  \x1b[1;34m•\x1b[0m Transferring to \x1b[1m{}\x1b[0m ({}) ...",
                target.name, target.ip
            );
            std::io::Write::flush(&mut std::io::stdout()).unwrap();

            let remote_destination = format!(
                "{}@{}:{}/{}/{}/",
                target.user, target.ip, target.remote_dir, db, env
            );

            // SSH discret
            let mkdir_status = Command::new("ssh")
                .args([
                    "-q",
                    &format!("{}@{}", target.user, target.ip),
                    &format!("mkdir -p {}/{}/{}", target.remote_dir, db, env),
                ])
                .status();

            if mkdir_status.is_err() || !mkdir_status.unwrap().success() {
                // \r efface virtuellement la ligne en repartant du début, et \x1b[K efface le reste de la ligne
                println!(
                    "\r\x1b[K  \x1b[1;31mFailed\x1b[0m to \x1b[1m{}\x1b[0m (SSH unreachable)",
                    target.name
                );
                continue;
            }

            // SCP discret
            let scp_status = Command::new("scp")
                .args([
                    "-q",
                    local_file.to_str().unwrap(),
                    &format!("{}{}.ji", remote_destination, env),
                ])
                .status();

            // 2. On écrase PROPREMENT la ligne en cours
            match scp_status {
                Ok(status) if status.success() => {
                    success_count += 1;
                    // \r rembobine, \x1b[K nettoie les résidus de caractères de la ligne précédente
                    println!(
                        "\r\x1b[K  \x1b[1;32m✓ Success\x1b[0m to \x1b[1m{}\x1b[0m ({})",
                        target.name, target.ip
                    );
                }
                _ => {
                    println!(
                        "\r\x1b[K  \x1b[1;31mfailed\x1b[0m to \x1b[1m{}\x1b[0m ({})",
                        target.name, target.ip
                    );
                }
            }
        }
        // Pied de page bilan style cargo
        println!(
            "\x1b[90m─────────────────────────────────────────────────────────────────\x1b[0m"
        );
        if success_count == total_targets {
            println!(
                "  \x1b[1;32mFinished\x1b[0m successfully deployed to {}/{} servers.",
                success_count, total_targets
            );
        } else {
            println!(
                "  \x1b[1;33mWarning\x1b[0m deployment partial: {}/{} targets reached.",
                success_count, total_targets
            );
        }
        println!();

        Ok(())
    }

    /// du nom de la base de données et de l'environnement ciblé.
    pub fn storage_dir(db_name: &str, env: &str) -> PathBuf {
        let home = std::env::var("HOME").expect("not unix");
        let path = PathBuf::from(LOCAL_STORAGE.replace("%home%", &home)).join("databases");

        // 2. Construction sémantique : racine / nom_de_la_db / environnement
        let final_dir = path.join(db_name).join(env);

        // 3. Création automatique et récursive de toute l'arborescence s'il n'existe pas
        let _ = std::fs::create_dir_all(&final_dir);

        final_dir
    }
    pub fn get_secure_storage_dir(&self, db_name: &str, env: &str) -> PathBuf {
        let home = std::env::var("HOME").expect("not unix");
        let path = PathBuf::from(LOCAL_STORAGE.replace("%home%", &home)).join("databases");

        // 2. Construction sémantique : racine / nom_de_la_db / environnement
        let final_dir = path.join(db_name).join(env);

        // 3. Création automatique et récursive de toute l'arborescence s'il n'existe pas
        let _ = std::fs::create_dir_all(&final_dir);

        final_dir
    }

    pub fn display_records(&self, db_name: &str, env: &str) {
        println!("\n{GREEN}╔══════════════════════════════════════════════════════╗{RESET}");
        println!(
            "║  JI DATABASE RECORDS LOG : [{:<6}] Environs: [{:<5}] ║",
            db_name, env
        );
        println!("{GREEN}╠══════════════════════════════════════════════════════╣{RESET}");

        println!("Nombre de Nœuds : {}", self.nodes.len());
        if self.nodes.is_empty() {
            println!("    (Aucun nœud enregistré)");
        } else {
            for (id, node) in &self.nodes {
                let label = node.label.as_deref().unwrap_or("Aucun");
                println!("   ├── [Nœud] ID: \x1b[1m{}\x1b[0m", id);
                println!("   │   ├── Label: {}", label);
                if !node.properties.is_empty() {
                    println!("   │   └── Propriétés:");
                    for (k, v) in &node.properties {
                        println!("   │       └── {}: {}", k, v);
                    }
                } else {
                    println!("   │   └── Propriétés: (Vide)");
                }
            }
        }

        println!("{GREEN}╠══════════════════════════════════════════════════════╣{RESET}");

        // Calcul du nombre total d'arêtes
        let total_edges: usize = self.edges.values().map(|v| v.len()).sum();
        println!("Nombre de Relations : {}", total_edges);
        if total_edges == 0 {
            println!("    (Aucune relation enregistrée)");
        } else {
            for (source, edges) in &self.edges {
                for edge in edges {
                    println!(
                        "   └──  \x1b[1m{}\x1b[0m --[\x1b[36m{}\x1b[0m]--> \x1b[1m{}\x1b[0m",
                        source, edge.relation_name, edge.target_id
                    );
                }
            }
        }
        println!("{GREEN}╚══════════════════════════════════════════════════════╝{RESET}\n");
    }
    pub fn get_node(&self, id: &str) -> Option<&NodeData> {
        self.nodes.get(id)
    }

    /// Récupère toutes les arêtes (relations) sortantes d'un nœud spécifique
    pub fn get_connections(&self, source_id: &str) -> Option<&Vec<EdgeData>> {
        self.edges.get(source_id)
    }

    /// Vérifie si un nœud existe dans le graphe
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Affiche proprement le contenu actuel du graphe en mémoire (Utile pour le débogage)
    pub fn debug_dump(&self) {
        println!("\n{GREEN} --- DUMP DE LA BASE DE DONNÉES JI ---{RESET}");
        println!("Nœuds enregistrés : {}", self.nodes.len());
        for (id, node) in &self.nodes {
            let label = node.label.as_deref().unwrap_or("Aucun");
            println!(
                "  • [Nœud] ID: {} | Label: {} | Propriétés: {:?}",
                id, label, node.properties
            );
        }

        println!("\nArêtes (Relations) enregistrées : ");
        for (source, edges) in &self.edges {
            for edge in edges {
                println!(
                    "  • [Arête] {} --({})--> {}",
                    source, edge.relation_name, edge.target_id
                );
            }
        }
    }
    /// Sauvegarde atomique avec Magic Number, compression Zstd, Bincode,
    /// gestion de l'historique dans snapshots/, et empilement (Push) dans ROLLBACK.
    pub fn save_to_disk(&self, db_name: &str, env: &str) -> Result<(), String> {
        let storage_dir = self.get_secure_storage_dir(db_name, env);

        // Le fichier principal porte le nom de son environnement (ex: prod.ji, tests.ji)
        let filename = format!("{env}.ji");
        let final_path = storage_dir.join(&filename);
        let tmp_path = storage_dir.join(format!("{filename}.tmp"));

        // Dossier pour isoler les clichés historiques
        let snaps_dir = storage_dir.join("snapshots");
        fs::create_dir_all(&snaps_dir)
            .map_err(|e| format!("Échec création dossier snapshots: {e}"))?;

        println!(
            "{GREEN} {RESET} Synchronisation binaire dans : {:?}",
            storage_dir
        );

        // 1. Sérialisation bincode + compression Zstd en mémoire vive
        let mut encoder = zstd::Encoder::new(Vec::new(), 3).map_err(|e| e.to_string())?;
        bincode::serialize_into(&mut encoder, &self).map_err(|e| e.to_string())?;
        let compressed_bytes = encoder.finish().map_err(|e| e.to_string())?;

        // 2. Écriture atomique dans le fichier temporaire (.tmp)
        {
            let mut tmp_file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;

            // On injecte le Magic Number d'abord, suivi du payload compressé
            tmp_file
                .write_all(&MAGIC_NUMBER)
                .map_err(|e| e.to_string())?;
            tmp_file
                .write_all(&compressed_bytes)
                .map_err(|e| e.to_string())?;

            // Flush physique des caches de l'OS sur le disque/clé USB
            tmp_file.sync_all().map_err(|e| e.to_string())?;
        }

        // 3. Bascule sécurisée : renommage atomique du .tmp vers le fichier final .ji
        std::fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;
        println!(
            "{GREEN} ✓{RESET} Base de données [{db_name}] synchronisée pour l'environnement [{env}].",
        );

        // 4. Système de Snapshotting automatique, gestion de HEAD et de ROLLBACK
        if env != "seeds" {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let snapshot_filename = format!("{env}_{timestamp}.ji");
            let snapshot_path = snaps_dir.join(&snapshot_filename);

            // Écriture de la copie historique binaire
            let mut snap_file = std::fs::File::create(&snapshot_path).map_err(|e| e.to_string())?;
            snap_file
                .write_all(&compressed_bytes)
                .map_err(|e| e.to_string())?;
            println!("{GREEN} ✓{RESET} Snapshot archivé : {snapshot_filename}");

            let head_path = storage_dir.join("HEAD");
            if head_path.exists() {
                if let Ok(old_head_content) = fs::read_to_string(&head_path) {
                    let old_head_trimmed = old_head_content.trim();

                    // Si un HEAD précédent valide existe, on l'empile à la fin du fichier ROLLBACK
                    if !old_head_trimmed.is_empty() && old_head_trimmed != snapshot_filename {
                        let mut rollback_file = fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(storage_dir.join("ROLLBACK"))
                            .map_err(|e| format!("Échec ouverture pile ROLLBACK: {e}"))?;

                        writeln!(rollback_file, "{}", old_head_trimmed)
                            .map_err(|e| format!("Échec écriture pile ROLLBACK: {e}"))?;

                        println!(
                            "{GREEN} ✓{RESET} Snapshot [{}] poussé dans la pile ROLLBACK.",
                            old_head_trimmed
                        );
                    }
                }
            }

            // Mise à jour du fichier pointeur HEAD vers le tout nouveau snapshot
            let mut head_file = File::create(&head_path)
                .map_err(|e| format!("Échec création fichier HEAD: {e}"))?;
            head_file
                .write_all(snapshot_filename.as_bytes())
                .map_err(|e| format!("Échec écriture fichier HEAD: {e}"))?;
            head_file.sync_data().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Charge le graphe depuis un fichier binaire sécurisé (.ji)
    /// en validant la signature binaire et en appliquant la décompression Zstd.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("Erreur ouverture fichier: {e}"))?;

        // 1. Extraction et vérification immédiate du Magic Number
        let mut header = [0u8; 4];
        file.read_exact(&mut header)
            .map_err(|e| format!("Erreur lecture en-tête (Fichier peut-être tronqué): {e}"))?;

        if header != MAGIC_NUMBER {
            return Err(
                "SÉCURITÉ : Fichier invalide ! Le Magic Number ne correspond pas.".to_string(),
            );
        }

        // 2. Lecture sécurisée du reste du flux d'octets purement Zstd
        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)
            .map_err(|e| format!("Erreur de lecture des données compressées: {e}"))?;

        // 3. Initialisation du décodeur Zstd sur les données brutes extraites
        let mut decoder = zstd::stream::Decoder::new(&compressed_data[..])
            .map_err(|e| format!("Erreur initialisation décodeur Zstd: {e}"))?;

        // 4. Désérialise les octets décodés vers la structure GraphStore
        let store: Self = bincode::deserialize_from(&mut decoder)
            .map_err(|e| format!("Erreur désérialisation bincode: {e}"))?;

        Ok(store)
    }

    /// Dépile (Pop) la dernière ligne du fichier ROLLBACK et met à jour la pile sur le disque (LIFO).
    pub fn pop_rollback_snapshot(
        &self,
        db_name: &str,
        env: &str,
    ) -> Result<Option<String>, String> {
        let storage_dir = self.get_secure_storage_dir(db_name, env);
        let rollback_path = storage_dir.join("ROLLBACK");

        if !rollback_path.exists() {
            return Ok(None);
        }

        // 1. Lecture complète de la pile de texte en RAM
        let content = fs::read_to_string(&rollback_path)
            .map_err(|e| format!("Impossible de lire la pile ROLLBACK: {e}"))?;

        // 2. Découpage par lignes valides
        let mut lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        // 3. Extraction du sommet de la pile (Last In, First Out)
        if let Some(last_snapshot) = lines.pop() {
            let target_snapshot = last_snapshot.to_string();

            // 4. Mise à jour ou nettoyage du fichier sur disque
            if lines.is_empty() {
                let _ = fs::remove_file(&rollback_path);
            } else {
                let mut new_content = lines.join("\n");
                new_content.push('\n');
                fs::write(&rollback_path, new_content)
                    .map_err(|e| format!("Impossible de mettre à jour la pile ROLLBACK: {e}"))?;
            }

            return Ok(Some(target_snapshot));
        }

        Ok(None)
    }

    pub fn rollback_to_previous(&self, db_name: &str, env: &str) -> Result<(), String> {
        let storage_dir = self.get_secure_storage_dir(db_name, env);
        let filename = format!("{env}.ji");
        let final_path = storage_dir.join(&filename);
        let head_path = storage_dir.join("HEAD");

        // 1. On extrait le dernier état de la pile ROLLBACK
        let target_snapshot_name = match self.pop_rollback_snapshot(db_name, env)? {
            Some(name) => name,
            None => {
                return Err(format!(
                    "Aucun état de rollback disponible pour {db_name}/{env}"
                ));
            }
        };

        let snapshot_full_path = storage_dir.join("snapshots").join(&target_snapshot_name);
        if !snapshot_full_path.exists() {
            return Err(format!(
                "Erreur : Le fichier snapshot n'existe pas ({:?})",
                snapshot_full_path
            ));
        }

        // 🎯 NOUVEAU : Avant d'écraser le HEAD actuel, on l'empile dans FORWARD
        if head_path.exists() {
            if let Ok(current_head) = fs::read_to_string(&head_path) {
                let current_head_trimmed = current_head.trim();
                if !current_head_trimmed.is_empty() {
                    let mut forward_file = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(storage_dir.join("FORWARD"))
                        .map_err(|e| format!("Échec ouverture pile FORWARD: {e}"))?;
                    writeln!(forward_file, "{}", current_head_trimmed)
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        println!(
            "{} {} Rétrogradation de la base [{}] ({}) vers : {}",
            GREEN, RESET, db_name, env, target_snapshot_name
        );

        // 2. Remplacement atomique de la base courante
        let tmp_path = storage_dir.join(format!("{filename}.rollback.tmp"));
        fs::copy(&snapshot_full_path, &tmp_path).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;

        // 3. Réalignement du pointeur HEAD
        fs::write(&head_path, &target_snapshot_name).map_err(|e| e.to_string())?;

        println!("{} ✓{} Rollback effectué avec succès !", GREEN, RESET);
        Ok(())
    }

    /// Liste toutes les bases de données détectées à la racine du stockage.
    pub fn list_databases(&self) -> Result<Vec<String>, String> {
        let home = std::env::var("HOME").expect("not unix");
        let path = PathBuf::from(LOCAL_STORAGE.replace("%home%", &home)).join("databases");

        if !path.exists() {
            return Ok(Vec::new());
        }

        let mut databases = Vec::new();

        // On scanne la racine en limitant à un seul niveau de profondeur
        for entry in WalkDir::new(&path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                if let Some(folder_name) = entry.file_name().to_str() {
                    databases.push(folder_name.to_string());
                }
            }
        }

        Ok(databases)
    }

    /// Avance l'environnement actuel vers l'état pointé par le sommet de la pile FORWARD (O(1) RAM)
    pub fn forward_to_next(&self, db_name: &str, env: &str) -> Result<(), String> {
        let storage_dir = self.get_secure_storage_dir(db_name, env);
        let filename = format!("{env}.ji");
        let final_path = storage_dir.join(&filename);
        let head_path = storage_dir.join("HEAD");
        let forward_path = storage_dir.join("FORWARD");

        if !forward_path.exists() {
            return Err(format!(
                "Impossible de forward : Aucun état futur trouvé pour {db_name}/{env}"
            ));
        }

        // 1. On lit et on pop la dernière ligne de la pile FORWARD
        let content = fs::read_to_string(&forward_path).map_err(|e| e.to_string())?;
        let mut lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        let target_snapshot_name = match lines.pop() {
            Some(name) => name.to_string(),
            None => return Err("Pile FORWARD vide.".to_string()),
        };

        let snapshot_full_path = storage_dir.join("snapshots").join(&target_snapshot_name);
        if !snapshot_full_path.exists() {
            return Err(format!(
                "Erreur : Le fichier du snapshot futur n'existe plus ({:?})",
                snapshot_full_path
            ));
        }

        if lines.is_empty() {
            let _ = fs::remove_file(&forward_path);
        } else {
            let mut new_content = lines.join("\n");
            new_content.push('\n');
            fs::write(&forward_path, new_content).map_err(|e| e.to_string())?;
        }

        if head_path.exists() {
            if let Ok(current_head) = fs::read_to_string(&head_path) {
                let current_head_trimmed = current_head.trim();
                if !current_head_trimmed.is_empty() {
                    let mut rollback_file = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(storage_dir.join("ROLLBACK"))
                        .map_err(|e| format!("Échec ouverture pile ROLLBACK: {e}"))?;
                    writeln!(rollback_file, "{}", current_head_trimmed)
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        println!(
            "{} {} Progression vers l'état futur : {}",
            GREEN, RESET, target_snapshot_name
        );

        // 2. Remplacement atomique du fichier principal
        let tmp_path = storage_dir.join(format!("{filename}.forward.tmp"));
        fs::copy(&snapshot_full_path, &tmp_path).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;

        // 3. Mise à jour du pointeur HEAD
        fs::write(&head_path, &target_snapshot_name).map_err(|e| e.to_string())?;

        println!(
            "{} ✓{} Forward effectué avec succès. Base réalignée sur le futur.",
            GREEN, RESET
        );
        Ok(())
    }
    /// Ajoute un nœud dans le dictionnaire en mémoire RAM
    pub fn add_node(
        &mut self,
        id: String,
        label: Option<String>,
        properties: HashMap<String, String>,
    ) {
        let node = NodeData {
            id: id.clone(),
            label,
            properties,
            vector: None,
        };
        self.nodes.insert(id, node);
    }

    /// Connecte deux nœuds existants via une arête relationnelle
    pub fn add_edge(&mut self, source_id: String, target_id: String, relation_name: String) {
        let edge = EdgeData {
            target_id,
            relation_name,
        };
        self.edges
            .entry(source_id)
            .or_insert_with(Vec::new)
            .push(edge);
    }
}
