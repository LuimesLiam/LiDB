/// One meaningful piece of SQL source text.
///
/// Keywords have their own variants because the parser needs to
/// distinguish `SELECT` from a user-chosen name such as `users`.
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
    Set,
    And,
    Or,
    Not,
    True,
    False,
    Null,
    IntegerType,
    BooleanType,
    TextType,

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
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    Asterisk,
    Plus,
    Minus,
    Slash,
    EndOfInput,
}
