use LiDB::sql::{LexError, Lexer, Token};

/// This integration test uses the lexer exactly as another crate would. Unlike
/// unit tests beside the implementation, it can access only TinyRel's public
/// API, which protects the interface that later parser code will depend on.
#[test]
fn insert_completion_criteria_produces_expected_tokens() {
    let tokens = Lexer::new("INSERT INTO users VALUES (1, 'Liam');")
        .tokenize()
        .unwrap();

    assert_eq!(
        tokens,
        vec![
            Token::Insert,
            Token::Into,
            Token::Identifier("users".to_string()),
            Token::Values,
            Token::LeftParen,
            Token::Integer(1),
            Token::Comma,
            Token::StringLiteral("Liam".to_string()),
            Token::RightParen,
            Token::Semicolon,
            Token::EndOfInput,
        ]
    );
}

#[test]
fn malformed_sql_returns_a_useful_error() {
    let result = Lexer::new("SELECT 'unterminated string").tokenize();

    assert_eq!(result, Err(LexError::UnterminatedString { position: 7 }));
}
