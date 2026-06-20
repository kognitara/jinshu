use crate::ast::{Command, Modifier, NodeExpr, PropertyFilter};
use crate::ast::{EdgeExpr, VectorFilter};
use crate::token::Token;
pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            position: 0,
        }
    }

    /// Analyse l'intégralité du fichier pour produire un Program contenant toutes les instructions
    pub fn parse_program(&mut self) -> Result<crate::ast::Program, String> {
        let mut commands = Vec::new();

        while self.position < self.tokens.len() {
            match self.peek_token() {
                Some(Token::Get) | Some(Token::Add) => {
                    let command = self.parse_single_command()?;
                    commands.push(command);
                }
                _ => {
                    // Consomme les sauts de ligne ou les blancs entre deux commandes
                    self.position += 1;
                }
            }
        }

        Ok(crate::ast::Program { commands })
    }

    /// Extrait une seule commande complète en consommant son mot-clé d'initiation
    fn parse_single_command(&mut self) -> Result<Command, String> {
        match self.peek_token() {
            Some(Token::Add) => {
                self.consume_token(); // Mange 'ADD'

                // COLLÈGUE : C'est ici qu'on appelle ta vraie fonction !
                self.parse_add_command()
            }
            Some(Token::Get) => {
                self.consume_token(); // Mange 'GET'
                self.parse_get_path()
            }
            _ => Err(format!(
                "Syntax Error: Commande inconnue au token {:?}",
                self.peek_token()
            )),
        }
    }
    fn parse_add_command(&mut self) -> Result<Command, String> {
        // 1. On parse le premier nœud
        let first_node = self.parse_node()?;

        // 2. Est-ce qu'il y a une flèche après ?
        if self.peek_token() == Some(&Token::StartEdge) {
            self.consume_token(); // Consomme le '-['

            // On extrait le nom de la relation, ex: "LGV_SUD_EST"
            let edge_name = match self.consume_token() {
                Some(Token::Ident(name)) => name,
                _ => return Err("Nom de relation manquant dans le ADD".to_string()),
            };

            // COLLÈGUE : On ajoute la détection des propriétés de l'arête ici !
            let mut edge_properties = Vec::new();
            if self.peek_token() == Some(&Token::OpenBrace) {
                self.consume_token(); // Mange le '{'
                edge_properties = self.parse_properties()?; // Réutilise ton super analyseur
            }

            // On ferme l'arête avec ']->'
            if self.consume_token() != Some(Token::Arrow) {
                return Err("Syntaxe invalide : ']->' manquant après la relation".to_string());
            }

            // On parse le nœud cible, ex: (destination)
            let target_node = self.parse_node()?;

            // On retourne l'expression d'arête complète avec ses propriétés
            Ok(Command::Add(crate::ast::AddExpression::Edge {
                source: first_node.alias,
                target: target_node.alias,
                name: edge_name,
                properties: edge_properties, // ◄ Injecté proprement !
            }))
        } else {
            // Pas de flèche ? Création d'un nœud isolé
            Ok(Command::Add(crate::ast::AddExpression::Node(first_node)))
        }
    }
    /// Décode un chemin de requêtes GET à sauts multiples (Chaînage profond)
    fn parse_get_path(&mut self) -> Result<Command, String> {
        // 1. Nœud initial de départ, ex: (source)
        let start = self.parse_node()?;

        let mut path = Vec::new();

        // 2. Boucle de chaînage profond : tant qu'il y a une arête qui commence, on continue
        while self.peek_token() == Some(&Token::StartEdge) {
            self.consume_token(); // Consomme le Token::StartEdge (-[)

            // Extraction des modificateurs (? ou *)
            let mut modifier = None;
            if let Some(Token::Star) = self.peek_token() {
                self.consume_token();
                modifier = Some('*');
            } else if let Some(Token::Question) = self.peek_token() {
                self.consume_token();
                modifier = Some('?');
            }

            // Nom de la relation, ex: "amis"
            let name = match self.consume_token() {
                Some(Token::Ident(n)) => n,
                _ => return Err("Nom de relation manquant dans l'arête".to_string()),
            };

            let mut vec_filter = None;

            // Y a-t-il un filtre vectoriel sémantique ? ( | similarite > 0.85 )
            if self.peek_token() == Some(&Token::Pipe) {
                self.consume_token(); // Mange le '|'

                let target = match self.consume_token() {
                    Some(Token::Ident(t)) => t,
                    _ => return Err("Cible du filtre sémantique manquante".to_string()),
                };

                let operator = match self.consume_token() {
                    Some(Token::GreaterThan) => ">".to_string(),
                    Some(Token::LessThan) => "<".to_string(),
                    Some(Token::Equal) => "=".to_string(),
                    _ => return Err("Opérateur de comparaison sémantique invalide".to_string()),
                };

                let value = match self.consume_token() {
                    Some(Token::Float(v)) => v,
                    _ => {
                        return Err(
                            "Valeur flottante attendue pour le filtre vectoriel".to_string()
                        );
                    }
                };

                vec_filter = Some(VectorFilter {
                    target,
                    operator,
                    value,
                });
            }

            // Fin de l'arête fermée par la flèche (]->)
            let next_t = self.consume_token();
            if next_t != Some(Token::Arrow) {
                return Err(format!(
                    "Syntaxe invalide : Fin d'arête ' ]-> ' manquante (Reçu à la place : {:?})",
                    next_t
                ));
            }

            // Nœud d'arrivée de ce segment, ex: (cible)
            let next_node = self.parse_node()?;

            // On pousse le segment (Arête, Nœud) dans notre vecteur de chemin
            path.push((
                EdgeExpr {
                    name,
                    modifier,
                    vec_filter,
                },
                next_node,
            ));
        }

        // Sécurité : Une requête GET doit au moins avoir traversé une arête
        if path.is_empty() {
            return Err(
                "Une requête GET doit spécifier au moins une arête à traverser.".to_string(),
            );
        }

        Ok(Command::Get { start, path })
    }

    // Analyseur de Nœud : (alias:Label {props})
    fn parse_node(&mut self) -> Result<NodeExpr, String> {
        if self.consume_token() != Some(Token::OpenParen) {
            return Err("Syntaxe invalide : Un nœud doit commencer par '('".to_string());
        }

        let alias = match self.consume_token() {
            Some(Token::Ident(name)) => name,
            _ => return Err("Syntaxe invalide : Alias du nœud manquant".to_string()),
        };

        let mut label = None;
        let mut properties_filters: Vec<PropertyFilter> = Vec::new(); // Nouvelle liste vide propre

        if self.peek_token() == Some(&Token::Colon) {
            self.consume_token(); // Mange le ':'
            label = match self.consume_token() {
                Some(Token::Ident(lbl)) => Some(lbl),
                _ => return Err("Syntaxe invalide : Label manquant après ':'".to_string()),
            };
        }

        if self.peek_token() == Some(&Token::OpenBrace) {
            self.consume_token(); // Mange le '{'
            properties_filters = self.parse_properties()?; // Récupère le vecteur de filtres
        }

        if self.consume_token() != Some(Token::CloseParen) {
            return Err("Syntaxe invalide : Parenthèse fermante ')' manquante".to_string());
        }

        Ok(NodeExpr {
            alias,
            label,
            properties_filters, // Type parfait (Vec<PropertyFilter>) !
        })
    }

    // Analyseur de propriétés mis à jour avec la détection des modificateurs (*, ?, !)
    fn parse_properties(&mut self) -> Result<Vec<PropertyFilter>, String> {
        let mut filters = Vec::new();

        while self.peek_token() != Some(&Token::CloseBrace) {
            // 1. Détection de l'opposition immédiate sur la clé (ex: !ville: "Paris")
            let mut modifier = Modifier::None;
            if self.peek_token() == Some(&Token::Not) {
                self.consume_token();
                modifier = Modifier::Opposed;
            }

            // 2. Lecture du nom de la propriété
            let key_id = match self.consume_token() {
                Some(Token::Ident(k)) => k,
                _ => return Err("Nom de propriété invalide".to_string()),
            };

            // 3. Détection des modificateurs de fin sur la clé (ex: hub* ou wifi?)
            if modifier == Modifier::None {
                if self.peek_token() == Some(&Token::Star) {
                    self.consume_token();
                    modifier = Modifier::Required;
                } else if self.peek_token() == Some(&Token::Question) {
                    self.consume_token();
                    modifier = Modifier::Optional;
                }
            }

            // 4. S'il y a un symbole ':', on va chercher la valeur associée
            let mut target_value_id = String::new();
            if self.peek_token() == Some(&Token::Colon) {
                self.consume_token(); // Mange le ':'
                target_value_id = match self.consume_token() {
                    Some(Token::Str(v)) => v,
                    Some(Token::Ident(v)) => v,
                    _ => return Err("Valeur de propriété invalide".to_string()),
                };
            }

            // Ajout du filtre complet construit
            filters.push(PropertyFilter {
                key_id,
                target_value_id,
                modifier,
            });

            // Gestion de la virgule de séparation
            if self.peek_token() == Some(&Token::Comma) {
                self.consume_token();
            }
        }

        self.consume_token(); // Mange le '}'
        Ok(filters)
    }

    // --- Utilitaires ---
    fn peek_token(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn consume_token(&mut self) -> Option<Token> {
        if self.position < self.tokens.len() {
            let t = self.tokens[self.position].clone();
            self.position += 1;
            Some(t)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{AddExpression, Command};
    use crate::token::Token; // Adapte selon ton architecture

    #[test]
    fn test_parse_isolated_node_with_label_and_props() {
        // Simule les tokens pour : (G_PARIS:Gare {ville: "Paris"})
        let tokens = vec![
            Token::Add,
            Token::OpenParen,
            Token::Ident("G_PARIS".to_string()),
            Token::Colon,
            Token::Ident("Gare".to_string()),
            Token::OpenBrace,
            Token::Ident("ville".to_string()),
            Token::Colon,
            Token::Str("Paris".to_string()),
            Token::CloseBrace,
            Token::CloseParen,
        ];

        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();

        assert_eq!(program.commands.len(), 1);
        if let Command::Add(AddExpression::Node(node)) = &program.commands[0] {
            assert_eq!(node.alias, "G_PARIS");
            assert_eq!(node.label, Some("Gare".to_string()));
            assert_eq!(node.properties_filters[0].key_id, "ville");
            assert_eq!(node.properties_filters[0].target_value_id, "Paris");
        } else {
            panic!("La commande aurait dû être un Add Expression d'un Nœud");
        }
    }

    #[test]
    fn test_parse_syntax_error_missing_paren() {
        // Simule un oubli de parenthèse : ADD (G_PARIS:Gare
        let tokens = vec![
            Token::Add,
            Token::OpenParen,
            Token::Ident("G_PARIS".to_string()),
            Token::Colon,
            Token::Ident("Gare".to_string()),
        ];

        let mut parser = Parser::new(tokens);
        let result = parser.parse_program();
        assert!(
            result.is_err(),
            "Le parseur aurait dû lever une erreur de syntaxe"
        );
    }
}
