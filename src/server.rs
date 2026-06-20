use crate::codegen::CodeGenerator;
use crate::lexer::Lexer;
use crate::optimizer::QueryOptimizer;
use crate::parser::Parser;
use crate::store::GraphStore;
use crate::vm::VirtualMachine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
pub async fn start_background_server() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:7789";
    let listener = TcpListener::bind(addr).await?;
    println!("\x1b[1;32m✓\x1b[0m Ji Server générique lancé sur tcp://{addr}");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buffer = [0; 4096];
            match socket.read(&mut buffer).await {
                Ok(n) if n == 0 => return,
                Ok(n) => {
                    let raw_request = String::from_utf8_lossy(&buffer[..n]);

                    // --- NOUVEAU PROTOCOLE ---
                    // On attend un format : db_name:env_name:le_script_ji
                    // Exemple : ji_db:prod:ADD (Paris:Gare)
                    let parts: Vec<&str> = raw_request.splitn(3, ':').collect();
                    if parts.len() < 3 {
                        let _ = socket
                            .write_all(b"ERROR: Format invalide. Attendu 'db:env:command'")
                            .await;
                        return;
                    }

                    let db = parts[0];
                    let env = parts[1];
                    let query_str = parts[2];

                    // Chargement dynamique du bon Store à chaque requête
                    let storage_dir = GraphStore::storage_dir(db, env);
                    let db_path = storage_dir.join(format!("{}.ji", env));

                    let store = if db_path.exists() {
                        GraphStore::load_from_file(db_path).unwrap_or_else(|_| GraphStore::new())
                    } else {
                        GraphStore::new()
                    };

                    // 1. Lexer & Parser de manière classique
                    let mut lexer = Lexer::new(query_str);
                    let tokens = lexer.get_tokens();
                    let mut parser = Parser::new(tokens);

                    if let Ok(program) = parser.parse_program() {
                        // 2. MIDDLEWARE ADAPTATIF (L'Optimizer inspecte l'AST)
                        let optimizer = QueryOptimizer::new();
                        let (optimized_program, mode) = optimizer.optimize(program);

                        // 3. Code Generation configurée avec le mode adapté
                        let mut codegen = CodeGenerator::new();
                        let instructions = codegen.generate(optimized_program, mode);

                        // 4. Exécution par la VM
                        let mut vm = VirtualMachine::new(instructions, store);
                        vm.run();
                    }
                }
                Err(e) => eprint!("Erreur: {:?}", e),
            }
        });
    }
}
