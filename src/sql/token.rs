#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Table,
    From,
    Where,
    Into,
    Values,

    Identifier(String),
    Integer(i64),
    StringLiteral(String),

    LeftParen,
    RightParen,
    Comma,
    Semicolon,

    Equal,
    NotEqual,
    LessThan,
    GreaterThan,

    Asterisk,
    EndOfInput,
}
