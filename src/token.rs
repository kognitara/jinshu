#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Connect,
    Add,
    Link,
    Get,
    Detach,
    Ident(String),
    Str(String),
    Float(f32),
    // Ajustements topologiques et objets
    StartEdge,   // -[  (Début de l'arête)
    Arrow,       // ]-> (Fin de l'arête orientée)
    DoubleColon, // :: (Pour Ji::connect)
    Not,
    Star,
    Question,
    Pipe,
    Dot,
    Colon,
    Comma,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    GreaterThan,
    LessThan,
    Equal,
}
