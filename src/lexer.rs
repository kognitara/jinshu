use crate::token::Token;

pub struct Lexer {
    input: Vec<char>,
    position: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            position: 0,
        }
    }
    pub fn get_tokens(&mut self) -> Vec<crate::token::Token> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens
    }
    // Récupérer le prochain Token
    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace_and_comments();

        if self.position >= self.input.len() {
            return None; // Fin du fichier
        }

        let ch = self.input[self.position];

        let token = match ch {
            // Délimiteurs simples
            '.' => {
                self.position += 1;
                Token::Dot
            }

            ':' => {
                if self.peek_char() == Some(':') {
                    self.position += 2;
                    Token::DoubleColon
                } else {
                    self.position += 1;
                    Token::Colon
                }
            }
            '!' => Token::Not,
            '(' => {
                self.position += 1;
                Token::OpenParen
            }
            ')' => {
                self.position += 1;
                Token::CloseParen
            }
            '[' => {
                self.position += 1;
                Token::OpenBracket
            }
            ']' => {
                if self.peek_char() == Some('-') && self.peek_char_2() == Some('>') {
                    self.position += 3;
                    Token::Arrow
                } else {
                    self.position += 1;
                    Token::CloseBracket
                }
            }
            '{' => {
                self.position += 1;
                Token::OpenBrace
            }
            '}' => {
                self.position += 1;
                Token::CloseBrace
            }
            ',' => {
                self.position += 1;
                Token::Comma
            }
            '*' => {
                self.position += 1;
                Token::Star
            }
            '?' => {
                self.position += 1;
                Token::Question
            }
            '|' => {
                self.position += 1;
                Token::Pipe
            }

            '>' => {
                self.position += 1;
                Token::GreaterThan
            }
            '<' => {
                self.position += 1;
                Token::LessThan
            }
            '=' => {
                self.position += 1;
                Token::Equal
            }
            '-' => {
                if self.peek_char() == Some('[') {
                    self.position += 2;
                    Token::StartEdge
                } else if self.peek_char() == Some('>') {
                    self.position += 2;
                    Token::Arrow
                } else {
                    self.position += 1;
                    Token::Ident("-".to_string()) // Au cas où c'est un signe moins
                }
            }
            // Chaînes de caractères littérales ("...")
            '"' => {
                let s = self.read_string();
                Token::Str(s)
            }

            // Identifiants, Mots-clés ou Nombres
            _ => {
                if ch.is_alphabetic() || ch == '_' {
                    let ident = self.read_identifier();
                    match ident.as_str() {
                        "CONNECT" | "connect" => Token::Connect,
                        "ADD" | "add" => Token::Add,
                        "LINK" | "link" => Token::Link,
                        "GET" | "get" => Token::Get,
                        "DETACH" | "detach" => Token::Detach,
                        _ => Token::Ident(ident),
                    }
                } else if ch.is_numeric() {
                    let num_str = self.read_number();
                    let num = num_str.parse::<f32>().unwrap_or(0.0);
                    Token::Float(num)
                } else {
                    // Caractère inconnu ignoré pour le moment
                    self.position += 1;
                    return self.next_token();
                }
            }
        };

        Some(token)
    }

    // --- Fonctions utilitaires de navigation interne ---

    fn peek_char(&self) -> Option<char> {
        if self.position + 1 >= self.input.len() {
            None
        } else {
            Some(self.input[self.position + 1])
        }
    }

    fn read_identifier(&mut self) -> String {
        let start = self.position;
        while self.position < self.input.len()
            && (self.input[self.position].is_alphanumeric() || self.input[self.position] == '_')
        {
            self.position += 1;
        }
        self.input[start..self.position].iter().collect()
    }

    fn read_number(&mut self) -> String {
        let start = self.position;
        while self.position < self.input.len()
            && (self.input[self.position].is_numeric() || self.input[self.position] == '.')
        {
            self.position += 1;
        }
        self.input[start..self.position].iter().collect()
    }

    fn read_string(&mut self) -> String {
        self.position += 1; // Sauter le " d'ouverture
        let start = self.position;
        while self.position < self.input.len() && self.input[self.position] != '"' {
            self.position += 1;
        }
        let s = self.input[start..self.position].iter().collect();
        self.position += 1; // Sauter le " de fermeture
        s
    }

    fn peek_char_2(&self) -> Option<char> {
        if self.position + 2 >= self.input.len() {
            None
        } else {
            Some(self.input[self.position + 2])
        }
    }
    fn skip_whitespace_and_comments(&mut self) {
        while self.position < self.input.len() {
            let ch = self.input[self.position];
            if ch.is_whitespace() {
                self.position += 1;
            } else if ch == '#' {
                // Commentaire détecté : on saute jusqu'au bout de la ligne
                while self.position < self.input.len() && self.input[self.position] != '\n' {
                    self.position += 1;
                }
            } else {
                break;
            }
        }
    }
}
