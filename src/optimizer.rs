use crate::ast::{Command, ExecutionMode, Program};

pub struct QueryOptimizer {
    pub default_mode: ExecutionMode,
}

impl QueryOptimizer {
    pub fn new() -> Self {
        QueryOptimizer {
            default_mode: ExecutionMode::Strict,
        }
    }

    /// Analyse et métamorphose le programme avant la génération de bytecode
    pub fn optimize(&self, program: Program) -> (Program, ExecutionMode) {
        let mut optimized_commands = Vec::new();
        let mut detected_mode = self.default_mode;

        for command in program.commands {
            match &command {
                Command::Get { start, path } => {
                    // Analyse des arêtes pour détecter si un filtre vectoriel est requis
                    let has_vector_filter = path.iter().any(|(edge, _)| edge.vec_filter.is_some());

                    if has_vector_filter {
                        // La requête demande du vecteur -> On bascule dynamiquement le moteur
                        detected_mode = ExecutionMode::Hybrid;
                        println!(
                            "[Optimizer] Détection de filtre vectoriel. Passage en mode HYBRIDE."
                        );
                    }
                    optimized_commands.push(command);
                }
                Command::Configure { mode } => {
                    detected_mode = *mode;
                    optimized_commands.push(command);
                }
                _ => optimized_commands.push(command),
            }
        }

        (
            Program {
                commands: optimized_commands,
            },
            detected_mode,
        )
    }
}
