mod ast;
mod bytecode;
mod codegen;
mod crypto;
mod db;
mod lexer;
mod optimizer;
mod parser;
mod server;
mod store;
mod token;
mod vm;
mod web;
use crate::optimizer::QueryOptimizer;
use crate::store::{GraphStore, LOCAL_STORAGE};
use clap::{Arg, Command, value_parser};
use codegen::CodeGenerator;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Text};
use lexer::Lexer;
use parser::Parser;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::env::current_dir;
use std::ffi::OsString;
use std::fs::{self, remove_dir_all};
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;
use std::time::Instant;
use vm::VirtualMachine;

// ANSI Formatting for compiler logs
const GREEN: &str = "\x1b[1;32m";
const RESET: &str = "\x1b[0m";

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(Command::new("new").about("Create database with prompts"))
        .subcommand(Command::new("list").about("List all created databases"))
        .subcommand(
            Command::new("serve")
                .about("Lance le moteur Ji en mode démon/serveur sur le port 7789"),
        )
        .subcommand(
            Command::new("export")
                .about("Export a database content to the device")
                .arg(
                    Arg::new("base")
                        .long("base")
                        .short('b')
                        .required(true)
                        .value_parser(value_parser!(String)),
                )
                .arg(
                    Arg::new("destination")
                        .long("destination")
                        .short('d')
                        .required(true)
                        .value_parser(value_parser!(String)),
                ),
        )
        .subcommand(Command::new("rm").about("Remove selected databases"))
        .subcommand(
            Command::new("deploy")
                .about("Deploy the database on servers")
                .arg(Arg::new("db").required(true).help("Name of the database"))
                .arg(
                    Arg::new("env")
                        .required(true)
                        .help("Environnement (prod, tests)"),
                ),
        )
        .subcommand(
            Command::new("forward")
                .about("Forward an environment to its next snapshot state")
                .arg(
                    Arg::new("env")
                        .required(false)
                        .help("The environment to forward (prod, tests)")
                        .value_parser(["prod", "tests"])
                        .default_value("prod"),
                ),
        )
        .subcommand(
            Command::new("show")
                .about("Display all nodes and edges from a database")
                .arg(
                    Arg::new("env")
                        .required(false)
                        .help("The environment to read (prod, tests, seeds)")
                        .value_parser(["prod", "tests", "seeds"])
                        .default_value("prod"),
                ),
        )
        .subcommand(Command::new("see").about("See the data graph"))
        .subcommand(
            Command::new("rollback")
                .about("Rollback an environment to its previous snapshot state")
                .arg(
                    Arg::new("env")
                        .required(false)
                        .help("The environment to rollback (prod, tests)")
                        .value_parser(["prod", "tests"])
                        .default_value("prod"),
                ),
        )
        .subcommand(
            Command::new("run")
                .about("Execute a ji file")
                .arg(
                    Arg::new("db")
                        .required(false)
                        .help("db name")
                        .short('b')
                        .long("base"),
                )
                .arg(
                    Arg::new("env")
                        .help("the env to use")
                        .short('e')
                        .long("env")
                        .required(false)
                        .value_parser(["prod", "tests", "seeds"])
                        .default_value("prod")
                        .default_missing_value("prod"),
                )
                .arg(
                    Arg::new("filename")
                        .short('f')
                        .long("file")
                        .required(true)
                        .help("the filename to execute"),
                ),
        )
}

fn process_matches(matches: &clap::ArgMatches, total_start: &Instant) {
    match matches.subcommand() {
        Some(("serve", _)) => {
            // Lance le serveur asynchrone tokio
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = crate::server::start_background_server().await {
                    eprintln!("Le serveur a crashé : {e}");
                }
            });
        }
        Some(("new", _)) => {
            let dir = current_dir().expect("Impossible de récupérer le dossier courant");
            let default_db = dir
                .file_name()
                .expect("Nom de dossier invalide")
                .to_str()
                .expect("")
                .to_string();

            let db = Text::new("Database name:")
                .with_default(&default_db)
                .prompt()
                .expect("failed to get db name");
            if GraphStore::new().list_databases().expect("").contains(&db) {
                eprintln!("{db} already exists");
                exit(1);
            }
            let environments = ["prod", "tests", "seeds"];

            for e in environments {
                let store = GraphStore::new();
                store
                    .save_to_disk(&db, e)
                    .expect("failed to create database");
                println!("{GREEN} ✓ {RESET}{e} database created successfully");
            }
            println!("{GREEN} ✓ {RESET}{db} database ready to use");
        }
        Some(("export", sub)) => {
            let base = sub.get_one::<String>("base").expect("base is required");
            let to = sub
                .get_one::<String>("destination")
                .expect("destination is required");
            export_database(base, to).expect("failed to export database");
        }
        Some(("see", _)) => {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(web::start_web_server());
        }
        Some(("rm", _)) => {
            remove_databases();
        }
        Some(("list", _)) => {
            let store = GraphStore::new();
            match store.list_databases() {
                Ok(dbs) => {
                    if dbs.is_empty() {
                        println!("no dadabases created");
                    } else {
                        println!(".");
                        let last = dbs.len() - 1;
                        for (i, db) in dbs.iter().enumerate() {
                            if i == last {
                                println!("└── {db}");
                            } else {
                                println!("├── {db}");
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Erreur lors du listage : {e}"),
            }
        }
        Some(("forward", sub)) => {
            let dir = current_dir().expect("failed to get current dir");
            let default_db = dir
                .file_name()
                .expect("failed to get filename")
                .to_str()
                .expect("")
                .to_string();

            let env = sub.get_one::<String>("env").expect("env is required");
            let db = matches.get_one::<String>("db").unwrap_or(&default_db);

            let store = GraphStore::new();
            if let Err(e) = store.forward_to_next(db, env) {
                eprintln!("\x1b[1;31mErreur lors du forward :\x1b[0m {e}");
                std::process::exit(1);
            }
        }
        Some(("show", sub)) => {
            let dir = current_dir().expect("failed to get current dir");
            let default_db = dir
                .file_name()
                .expect("failed to get filename")
                .to_str()
                .expect("")
                .to_string();

            let env = sub.get_one::<String>("env").expect("env is required");
            let db = matches.get_one::<String>("db").unwrap_or(&default_db);

            // 1. On localise le fichier de la base de données
            let dummy_store = GraphStore::new();
            let storage_dir = dummy_store.get_secure_storage_dir(db, env);
            let db_path = storage_dir.join(format!("{env}.ji"));

            if db_path.exists() {
                // 2. Chargement militaire (validation Magic Number + Zstd)
                match GraphStore::load_from_file(&db_path) {
                    Ok(store) => {
                        // 3. Affichage des enregistrements
                        store.display_records(db, env);
                    }
                    Err(e) => {
                        eprintln!("\x1b[1;31mErreur de lecture binaire :\x1b[0m {e}");
                        exit(1);
                    }
                }
            } else {
                eprintln!(
                    "\x1b[1;31mErreur :\x1b[0m Aucune base de données trouvée à l'emplacement : {:?}",
                    db_path
                );
                exit(1);
            }
        }
        Some(("run", sub)) => {
            let dir = current_dir().expect("failed to get current dir");
            let default_db = &dir
                .file_name()
                .expect("failed to get filename")
                .to_str()
                .expect("")
                .to_string();
            let file = sub
                .get_one::<String>("filename")
                .expect("filename is required");
            let env = sub.get_one::<String>("env").expect("env is required");
            let db = sub.get_one::<String>("db").unwrap_or(default_db);
            execute_file(&total_start, file, env, db);
        }
        Some(("rollback", sub)) => {
            let dir = current_dir().expect("failed to get current dir");
            let default_db = dir
                .file_name()
                .expect("failed to get filename")
                .to_str()
                .expect("")
                .to_string();

            let env = sub.get_one::<String>("env").expect("env is required");
            let db = matches.get_one::<String>("db").unwrap_or(&default_db);

            let store = GraphStore::new();
            if let Err(e) = store.rollback_to_previous(db, env) {
                eprintln!("\x1b[1;31mErreur lors du rollback :\x1b[0m {e}");
                std::process::exit(1);
            }
        }
        Some(("deploy", sub)) => {
            let db = sub.get_one::<String>("db").unwrap();
            let env = sub.get_one::<String>("env").unwrap();

            let store = GraphStore::new();
            if let Err(e) = store.deploy_to_remotes(db, env) {
                eprintln!("\x1b[1;31mErreur de déploiement :\x1b[0m {e}");
                std::process::exit(1);
            }
        }
        _ => {
            cli().clone().print_help().expect("failed to print help");
        }
    }
}
fn run_repl() {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║             Ji Interactive Shell Engine              ║");
    println!("║       Tapez 'exit' ou 'quit' pour quitter            ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let mut rl = DefaultEditor::new().expect("Échec d'initialisation de Rustyline");

    // Tente de charger l'historique pour le confort (flèche du haut)
    let _ = rl.load_history(".ji_history");

    loop {
        // Le prompt affiche fièrement "ji> "
        let readline = rl.readline("ji> ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == "exit" || trimmed == "quit" {
                    break;
                }

                // Sauvegarde dans l'historique
                let _ = rl.add_history_entry(trimmed);

                // ASTUCE : On découpe la ligne et on reconstruit les arguments
                // Si l'utilisateur tape "run -f test.ji", on injecte "ji" au début pour Clap
                let mut args: Vec<OsString> = vec![OsString::from("ji")];
                for part in trimmed.split_whitespace() {
                    args.push(OsString::from(part));
                }

                // On relance le parsing Clap sur ces arguments virtuels !
                let cli_app = cli();
                match cli_app.try_get_matches_from(args) {
                    Ok(matches) => {
                        // Ici, tu appelles ta fonction existante qui aiguille tes sous-commandes
                        // Exemple si ton aiguillage est dans main() :
                        process_matches(&matches, &Instant::now());
                    }
                    Err(err) => {
                        // Affiche l'erreur Clap proprement (ex: commande inconnue) sans faire crash le shell
                        err.print().unwrap();
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C
                println!("Interrompu. Tapez exit pour quitter.");
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D
                println!("Au revoir !");
                break;
            }
            Err(err) => {
                println!("Erreur Shell: {:?}", err);
                break;
            }
        }
    }
    // Sauvegarde l'historique avant de partir
    let _ = rl.save_history(".ji_history");
}

pub fn export_database(db_name: &str, target_base_path: &str) -> Result<(), String> {
    // 1. Définition des chemins source et cible
    let home = std::env::var("HOME").expect("not unix");
    let src = LOCAL_STORAGE.replace("%home%", home.as_str());
    let source_dir = PathBuf::from(format!("{src}/databases/{db_name}",));
    let target_dir = PathBuf::from(target_base_path)
        .join("databases")
        .join(db_name);

    if !source_dir.exists() {
        return Err(format!(
            "La base de données source [{}] n'existe pas dans ~/.local/share/ji",
            db_name
        ));
    }

    println!("{} {} Exporting the {} database...", GREEN, RESET, db_name);

    // 2. Collecter tous les fichiers à copier pour connaître le "Total" à l'avance
    let mut files_to_copy = Vec::new();
    for entry in walkdir::WalkDir::new(&source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            files_to_copy.push(entry.into_path());
        }
    }

    let total_files = files_to_copy.len() as u64;
    if total_files == 0 {
        println!("{} ✓{} Data is empty.", GREEN, RESET);
        return Ok(());
    }

    // 3. Initialisation et configuration de la chouette Progress Bar
    // Le template utilise des codes de style de indicatif pour la couleur (ex: {green}, {cyan})
    let pb = ProgressBar::new(total_files);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.white} [{elapsed_precise}] [{bar:40.white}] {pos}/{len} fichiers ({eta}) {msg}"
        )
        .unwrap()
        .progress_chars("#>-") // Style de la barre : rempli avec #, pointeur >, vide avec -
    );

    // Activer un spinner si l'écriture prend un peu de temps sur certains blocs
    pb.enable_steady_tick(Duration::from_millis(80));

    // 4. Boucle de copie et création de dossiers
    for file_path in files_to_copy {
        // Déterminer le chemin relatif par rapport au dossier source
        let relative_path = file_path
            .strip_prefix(&source_dir)
            .map_err(|e| e.to_string())?;

        // Reconstruire le chemin cible exact sur la clé USB
        let destination_path = target_dir.join(relative_path);

        // Mettre à jour le message sous la barre pour voir le fichier en cours de traitement
        if let Some(file_name) = file_path.file_name() {
            pb.set_message(format!("{}", file_name.to_string_lossy()));
        }

        // Créer les sous-dossiers parents (ex: /prod, /tests, /seeds) s'ils n'existent pas sur la cible
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Copie atomique ou standard du fichier binaire vers la clé USB
        fs::copy(&file_path, &destination_path).map_err(|e| e.to_string())?;

        // Avancer la barre de progression de 1
        pb.inc(1);
    }

    // 5. Finalisation propre de la barre
    pb.finish_and_clear(); // Nettoie la ligne de message dynamique

    println!(
        "{} {} The database {} has been exported successfully at : {:?}",
        GREEN, RESET, db_name, target_dir
    );

    Ok(())
}

fn execute_file(total_start: &Instant, filename: &str, env: &str, db: &str) {
    println!(
        "{}{} Starting compilation from the source file",
        GREEN, RESET
    );
    let src = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Failed to read target file: {err}");
            std::process::exit(1);
        }
    };
    if Path::new(filename).exists().eq(&false) {
        eprintln!("the source file not exists");
        exit(1);
    }
    if src.is_empty() {
        eprintln!("missing queries");
        exit(1);
    }
    let mut lexer = Lexer::new(&src);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token() {
        tokens.push(token);
    }
    println!(" {GREEN}✓{RESET} Lexical analysis completed.");
    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(prog) => prog,
        Err(err) => {
            eprintln!("SYNTAX ERROR: {err}");
            std::process::exit(1);
        }
    };
    println!(" {GREEN}✓{RESET} Abstract Syntax Tree generated for multi-line execution.");
    let optimizer = QueryOptimizer::new();
    let (optimized_program, mode) = optimizer.optimize(program.clone());
    let mut codegen = CodeGenerator::new();
    let instructions = codegen.generate(optimized_program, mode);
    println!(
        " {GREEN}✓{RESET} Virtual Machine Bytecode emitted ({} opcodes).",
        instructions.len()
    );
    let dummy_store = GraphStore::new();
    let storage_dir = dummy_store.get_secure_storage_dir(db, env);
    let db_path = storage_dir.join(format!("{env}.ji").as_str());
    let store = if db_path.exists() {
        println!(
            // "{GREEN} {RESET} Chargement de la base de données existante depuis : {db_path:?}"
        );
        GraphStore::load_from_file(db_path.clone()).unwrap_or_else(|_| GraphStore::new())
    } else {
        println!(
            "{GREEN} {RESET} Aucune base existante trouvée à l'emplacement cible. Création d'un nouveau store."
        );
        GraphStore::new()
    };

    // --- EXÉCUTION DU BYTECODE ---
    let vm_start = Instant::now();
    let mut vm = VirtualMachine::new(instructions, store);
    vm.run();
    let vm_duration = vm_start.elapsed();

    let updated_store = vm.into_store();

    // --- SYNCHRONISATION ET SNAPSHOT ---
    println!(
        "  Synchronisation binaire dans : {:?}",
        updated_store.get_secure_storage_dir(db, env)
    );

    if let Err(e) = updated_store.save_to_disk(db, env) {
        eprintln!("\x1b[1;31mErreur de synchronisation :\x1b[0m {e}");
        exit(1);
    }
    println!("{GREEN} ✓{RESET} Script processed successfully.");
    let total_duration = total_start.elapsed();

    // Affichage des métriques style Engine de Production / OpenRC
    println!("\n{RESET}");
    println!(" ╔══════════════════════════════════════════════════════╗");
    println!(" ║  Ji Execution Engine Summary                         ║",);
    println!(" ╠══════════════════════════════════════════════════════╣");
    println!(" ║  • Hardware Compute (VM/GPU) : {vm_duration:>10.4?}            ║",);
    println!(" ║  • Total Pipeline Time       : {total_duration:>10.4?}            ║",);
    println!(" ╚══════════════════════════════════════════════════════╝");
    println!(
        "\n{} {GREEN}✓{RESET} Script processed successfully.\n",
        GREEN
    );
}

fn remove_databases() {
    if let Ok(dbs) = GraphStore::new().list_databases() {
        let mut databases = dbs.clone();
        databases.sort();
        let apply_remove_on = MultiSelect::new("Select database to remove", databases)
            .prompt()
            .expect("failed to get dbs");
        let home = std::env::var("HOME").expect("not unix");
        let storage = LOCAL_STORAGE.replace("%home%", &home);
        for db in &apply_remove_on {
            remove_dir_all(format!("{storage}/databases/{db}").as_str())
                .expect("failed to remove database");
        }
        println!("db removed successfully");
    } else {
        println!("no databases to remove");
    }
}
fn main() {
    // Enclenchement du chronomètre global
    let total_start = Instant::now();
    let app = cli();
    let matches = app.clone().get_matches();
    // Si aucune sous-commande n'est passée (ex: juste 'ji'), on lance le shell !
    if matches.subcommand_name().is_none() {
        run_repl();
    } else {
        process_matches(&matches, &total_start);
    }
}
