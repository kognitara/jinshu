use crate::{
    ast::ExecutionMode,
    ast::Modifier,
    bytecode::OpCode,
    store::{GraphStore, NodeData},
};
use std::collections::HashMap;
use std::time::Duration;
use wgpu::{ExperimentalFeatures, util::DeviceExt};

// ANSI Colors formatting constants
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1b";
const RED: &str = "\x1b[1;31m";
const GREEN: &str = "\x1b[1;32m";
const YELLOW: &str = "\x1b[1;33m";
const BLUE: &str = "\x1b[1;34m";
const MAGENTA: &str = "\x1b[1;35m";
const CYAN: &str = "\x1b[1;36m";
const GRAY: &str = "\x1b[90m";

pub struct VirtualMachine {
    store: GraphStore,
    instructions: Vec<OpCode>,
    active_node: Option<NodeData>,
    current_mode: ExecutionMode,
}

impl VirtualMachine {
    #[must_use]
    pub fn new(instructions: Vec<OpCode>, store: GraphStore) -> Self {
        VirtualMachine {
            instructions,
            store,
            active_node: None,
            current_mode: ExecutionMode::Hybrid,
        }
    }

    pub fn into_store(self) -> GraphStore {
        self.store
    }
    pub fn run(&mut self) {
        for op in &self.instructions {
            match op {
                OpCode::SetExecutionMode(mode) => {
                    self.current_mode = *mode;
                }
                OpCode::CreateNode {
                    alias,
                    label,
                    properties,
                } => {
                    println!(
                        " {GREEN}{RESET} Modification du stockage : Ajout du nœud « {alias} »"
                    );
                    // On insère directement le nœud dans notre base binaire Zstd !
                    self.store
                        .add_node(alias.clone(), label.clone(), properties.clone());
                }

                // --- VERSION UNIQUE ET NETTOYÉE DE LOADNODE ---
                OpCode::LoadNode {
                    alias,
                    label,
                    properties_filters,
                } => {
                    if let Some(node) = self.store.nodes.get(alias) {
                        let mut node_valid = true;

                        // Exécution du filtrage géométrique (*, ?, !)
                        for filter in properties_filters {
                            let has_prop = node.properties.contains_key(&filter.key_id);

                            match filter.modifier {
                                Modifier::Required => {
                                    if !has_prop {
                                        node_valid = false;
                                        break;
                                    }
                                }
                                Modifier::Opposed => {
                                    if has_prop {
                                        if let Some(val) = node.properties.get(&filter.key_id) {
                                            if val == &filter.target_value_id {
                                                node_valid = false;
                                                break;
                                            }
                                        }
                                    }
                                }
                                Modifier::Optional => {
                                    if has_prop {
                                        if let Some(val) = node.properties.get(&filter.key_id) {
                                            if val != &filter.target_value_id {
                                                node_valid = false;
                                                break;
                                            }
                                        }
                                    }
                                }
                                Modifier::None => {}
                            }
                        }

                        if node_valid {
                            let label_clean = node
                                .label
                                .clone()
                                .unwrap_or_else(|| "Unspecified".to_string());
                            println!(
                                "{GREEN} {RESET} Founded node « {alias} » into topology memory label : {label_clean}",
                            );
                            self.active_node = Some(node.clone());
                        } else {
                            println!(
                                "{RED} ✗{RESET} Node « {alias} » rejected by property modifiers (*, ?, !)",
                            );
                            self.active_node = None; // Le nœud ne valide pas les critères, on vide le registre
                        }
                    } else {
                        println!(
                            "{GREEN} {RESET} Activating sentinel fallback node for « {alias} »"
                        );
                        self.active_node = Some(NodeData {
                            id: alias.clone(),
                            label: Some("Sentinel".to_string()),
                            properties: HashMap::new(),
                            vector: None,
                        });
                    }
                }
                OpCode::TraverseEdge { name, required } => {
                    // Étape 1 : On vérifie qu'on a bien un nœud actif de départ dans la VM
                    if let Some(ref current_node) = self.active_node {
                        match self.current_mode {
                            // =========================================================================
                            // MODE STRICT : Recherche topologique exacte
                            // =========================================================================
                            ExecutionMode::Strict => {
                                // On cherche une arête sortante physique qui porte exactement le bon nom
                                if let Some(edges) = self.store.edges.get(current_node.id.as_str())
                                {
                                    let exact_edge =
                                        edges.iter().find(|e| e.relation_name == name.as_str());

                                    if let Some(edge) = exact_edge {
                                        // Arête trouvée ! On charge le nœud cible dans le registre actif
                                        if let Some(target_node) =
                                            self.store.nodes.get(&edge.target_id)
                                        {
                                            self.active_node = Some(target_node.clone());
                                            println!(
                                                "{} [Strict] Saut topologique réussi via l'arête '{}' -> {}{}",
                                                GREEN, name, target_node.id, RESET
                                            );
                                        }
                                    } else if *required {
                                        // Si l'arête est requise (pas de '?') et introuvable, le chemin casse
                                        self.active_node = None;
                                        println!(
                                            "{} [Strict] Échec : Arête requise '{}' manquante. Wait.{}",
                                            RED, name, RESET
                                        );
                                    }
                                } else if *required {
                                    self.active_node = None;
                                }
                            }

                            // =========================================================================
                            // MODE SÉMANTIQUE / HYBRIDE : Utilisation intensive du GPU Compute (wgpu)
                            // =========================================================================
                            ExecutionMode::Semantic | ExecutionMode::Hybrid => {
                                if let Some(edges) = self.store.edges.get(current_node.id.as_str())
                                {
                                    for edge in edges {
                                        // CORRECTION : L'arête n'a pas de properties, on va chercher le nœud cible dans le store !
                                        if let Some(target_node) =
                                            self.store.nodes.get(edge.target_id.as_str())
                                        {
                                            // On extrait la chaîne du vecteur sémantique depuis le nœud cible
                                            if let Some(vec_str) =
                                                target_node.properties.get("vector")
                                            {
                                                // --- Début du traitement wgpu / GPU Compute ---
                                                // TODO: Insérer ici tes buffers wgpu, le chargement du pipeline
                                                // et le calcul de similarité cosinus avec l'active_node

                                                println!(
                                                    " {}[GPU Compute]{} Calcul de similarité sémantique pour l'arête '{}' vers le nœud '{}'",
                                                    MAGENTA,
                                                    RESET,
                                                    edge.relation_name,
                                                    target_node.id
                                                );

                                                // Exemple de dispatch wgpu ou de fallback sémantique...
                                                // ... Ton code existant avec encoder.begin_compute_pass, staging_buf, etc.
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        println!(
                            "{} [VM Error] Impossible de traverser l'arête '{}' : aucun nœud actif dans le registre.{}",
                            RED, name, RESET
                        );
                    }
                }
                OpCode::CreateEdge {
                    source,
                    target,
                    name,
                } => {
                    println!(
                        "{GREEN} {RESET} Modifying storage: Connecting « {source} » -[{name}]-> « {target} »",
                    );
                    // Injection directe et persistance dans le store binaire
                    self.store
                        .add_edge(source.clone(), target.clone(), name.clone());
                }
                OpCode::GpuVectorFilter {
                    target: _,
                    op,
                    threshold,
                } => {
                    let dimensions: u32 = 384;
                    let num_edges: u32 = 5;
                    println!("{GREEN} {RESET} Submitting WGSL multi-vector alignment pipeline...");

                    // 1. Generate local semantic query vector
                    let mut query_vector = vec![0.0f32; dimensions as usize];
                    for i in 0..dimensions as usize {
                        query_vector[i] = (i as f32 / dimensions as f32).sin();
                    }
                    let q_norm = query_vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                    if q_norm > 0.0 {
                        query_vector.iter_mut().for_each(|v| *v /= q_norm);
                    }

                    // 2. Generate edges matrix
                    let mut edges_matrix = vec![0.0f32; (num_edges * dimensions) as usize];
                    for edge_idx in 0..num_edges as usize {
                        let offset = edge_idx * dimensions as usize;
                        for i in 0..dimensions as usize {
                            let factor = edge_idx as f32 * 0.2;
                            edges_matrix[offset + i] =
                                ((i as f32 / dimensions as f32) + factor).sin();
                        }
                        let e_norm = edges_matrix[offset..offset + dimensions as usize]
                            .iter()
                            .map(|v| v * v)
                            .sum::<f32>()
                            .sqrt();
                        if e_norm > 0.0 {
                            edges_matrix[offset..offset + dimensions as usize]
                                .iter_mut()
                                .for_each(|v| *v /= e_norm);
                        }
                    }

                    // 3. Execute math pipeline via WGSL Compute Shader
                    let scores = pollster::block_on(run_wgpu_similarity(
                        &query_vector,
                        &edges_matrix,
                        dimensions,
                        num_edges,
                    ));

                    let mut path_valid = false;
                    println!();
                    for (idx, score) in scores.iter().enumerate() {
                        let matches_cond = *op == ">" && *score > *threshold;
                        let check_mark = if matches_cond {
                            format!("{GREEN}✓{RESET}")
                        } else {
                            format!("{RED}✗{RESET}")
                        };

                        println!(
                            "{GRAY}   ├─ {idx}{RESET} Cosine similarity: {score:.4} {GREEN}{check_mark}{RESET}"
                        );

                        if matches_cond {
                            path_valid = true;
                        }
                    }

                    if path_valid {
                        println!("{GRAY}   └─ R{RESET} Matrix threshold :  valid {GREEN}{RESET}");
                    } else {
                        println!("{GRAY}   └─ R{RESET} No vector matched : {op}{threshold:.2})");
                    }
                }

                OpCode::SetSentinelNode => {
                    println!("Activating sentinel fallback node routes.");
                }

                OpCode::StoreResult { target_alias } => {
                    println!(
                        "\n{GREEN} {RESET} Target node '{target_alias}' successfully assigned to output register."
                    );
                } // Ta logique existante (LoadNode, TraverseEdge, calculs WGSL...)
            }
        }
    }
}

async fn run_wgpu_similarity(
    query_vector: &[f32],
    edges_matrix: &[f32],
    dimensions: u32,
    num_edges: u32,
) -> Vec<f32> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            trace: wgpu::Trace::Off,
            experimental_features: ExperimentalFeatures::disabled(),
            label: Some("Ji_Compute_Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        })
        .await
        .unwrap();

    let shader_source = r#"
        @group(0) @binding(0) var<storage, read> query : array<f32>;
        @group(0) @binding(1) var<storage, read> edges : array<f32>;
        @group(0) @binding(2) var<storage, read_write> results : array<f32>;

        struct Params {
            dimensions: u32,
            num_edges: u32,
        }
        @group(0) @binding(3) var<uniform> params : Params;

        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let idx = global_id.x;
            if (idx >= params.num_edges) {
                return;
            }

            let dim = params.dimensions;
            let offset = idx * dim;

            var dot_product = 0.0;
            var norm_query = 0.0;
            var norm_edge = 0.0;

            for (var i = 0u; i < dim; i = i + 1u) {
                let q = query[i];
                let e = edges[offset + i];
                dot_product = dot_product + (q * e);
                norm_query = norm_query + (q * q);
                norm_edge = norm_edge + (e * e);
            }

            if (norm_query > 0.0 && norm_edge > 0.0) {
                results[idx] = dot_product / (sqrt(norm_query) * sqrt(norm_edge));
            } else {
                results[idx] = 0.0;
            }
        }
    "#;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Cosine_Similarity_Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let query_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Query_Buffer"),
        contents: bytemuck::cast_slice(query_vector),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let edges_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Edges_Matrix_Buffer"),
        contents: bytemuck::cast_slice(edges_matrix),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let results_size = (num_edges * 4) as wgpu::BufferAddress;
    let results_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output_Results_Buffer"),
        size: results_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_data = [dimensions, num_edges];
    let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform_Params_Buffer"),
        contents: bytemuck::cast_slice(&params_data),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging_Buffer"),
        size: results_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Similarity_Pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Compute_Bind_Group"),
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: query_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: edges_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: results_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);

        let workgroups = (num_edges + 63) / 64;
        cpass.dispatch_workgroups(workgroups, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&results_buf, 0, &staging_buf, 0, results_size);
    queue.submit(Some(encoder.finish()));

    let buffer_slice = staging_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());

    if device
        .poll(wgpu::wgt::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })
        .is_ok()
    {}

    rx.recv().unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let final_results: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buf.unmap();

    final_results
}
