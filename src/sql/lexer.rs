use super::Token;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    UnterminatedString {position: usize},
    InvalidInteger {literal: String, position: usize},
    UnexpectedCharacter {character: char, position: usize}
}

impl fmt::Display for LexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnterminatedString { position } => {
                write!(
                    formatter,
                    "unterminated string starting at character {position}"
                )
            }
            LexError::InvalidInteger { literal, position } => {
                write!(
                    formatter,
                    "integer '{literal}' at character {position} is outside the i64 range"
                )
            }
            LexError::UnexpectedCharacter {
                character,
                position,
            } => {
                write!(
                    formatter,
                    "unexpected character '{character}' at character {position}"
                )
            }
        }
    }
}

impl Error for LexError {}

pub type LexerError = LexError; //alias 

#[derive(Debug, Clone)]
pub struct Lexer {
    input: Vec<char>, 
    position: usize
}

/// Convenience wrapper for callers that do not need to keep a `Lexer` value.
pub fn tokenize(input: impl AsRef<str>) -> Result<Vec<Token>, LexError> {
    Lexer::new(input).tokenize()
}

impl Lexer {
    pub fn new(input: impl AsRef<str>) -> Self {
        Self {
            input: input.as_ref().chars().collect(),
            position: 0
        }
    }


    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            let is_end = token == Token::EndOfInput;
            tokens.push(token);

            if is_end {
                return Ok(tokens);
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace(); 

        let Some(character) = self.current_char() else {
            return Ok(Token::EndOfInput);
        };


        match character {
            character if character.is_ascii_alphabetic() || character == '_' => {
                Ok(self.read_word())
            }
            character if character.is_ascii_digit() => self.read_integer(),
            '\'' => self.read_string_literal(),
            '(' => {
                self.advance();
                Ok(Token::LeftParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RightParen)
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            ';' => {
                self.advance();
                Ok(Token::Semicolon)
            }
            '=' => {
                self.advance();
                Ok(Token::Equal)
            }
            '!' if self.peek_char() == Some('=') => {
                self.advance();
                self.advance();
                Ok(Token::NotEqual)
            }
            '<' if self.peek_char() == Some('>') => {
                // SQL commonly accepts both `<>` and `!=` for "not equal".
                self.advance();
                self.advance();
                Ok(Token::NotEqual)
            }
            '<' => {
                self.advance();
                Ok(Token::LessThan)
            }
            '>' => {
                self.advance();
                Ok(Token::GreaterThan)
            }
            '*' => {
                self.advance();
                Ok(Token::Asterisk)
            }
            unexpected => Err(LexError::UnexpectedCharacter {
                character: unexpected,
                position: self.position,
            }),
        }
    }

    fn current_char(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while self.current_char().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }

    fn read_word(&mut self) -> Token {
        let start = self.position; 

        while self.current_char().is_some_and(
            |c| c.is_ascii_alphanumeric() || c == '_'
        )
        {
            self.advance();
        }

        let word: String =self.input[start..self.position].iter().collect(); 

        match word.to_ascii_uppercase().as_str() {
            "SELECT" => Token::Select,
            "INSERT" => Token::Insert,
            "UPDATE" => Token::Update,
            "DELETE" => Token::Delete,
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            _ => Token::Identifier(word),
        }
    }

    fn read_integer(&mut self) -> Result<Token, LexError> {
        let start = self.position; 

        while self.current_char().is_some_and(|c| c.is_ascii_digit()){
            self.advance();
        }

        let literal: String = self.input[start..self.position].iter().collect();
        literal
            .parse::<i64>()
            .map(Token::Integer)
            .map_err(|_| LexError::InvalidInteger {
                literal, 
                position: start,
            })
    }

    fn read_string_literal (&mut self) ->Result<Token, LexError> {
        let startt = self.position; 
        self.advance(); //comsume opening quoute; 

        let mut value = String::new(); 

        while let Some(car) = self.current_char() {
            if car != '\'' {
                value.push(car);
                self.advance();
                continue;
            }

            if self.peek_char() ==Some('\''){
                //SQL uses a apostrophe inside text by doubling it like Bob''s Burgers but store the value Bob's
                value.push('\'');
                self.advance();
                self.advance();
                continue;
            }

            self.advance(); //comsume last quote;

            return Ok(Token::StringLiteral(value));
        }

        Err(LexError::UnterminatedString {position : startt})
    }
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_select_statement() {
        let tokens = Lexer::new("SELECT name FROM users WHERE id = 10;")
            .tokenize()
            .unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Select,
                Token::Identifier("name".to_string()),
                Token::From,
                Token::Identifier("users".to_string()),
                Token::Where,
                Token::Identifier("id".to_string()),
                Token::Equal,
                Token::Integer(10),
                Token::Semicolon,
                Token::EndOfInput,
            ]
        );
    }

    #[test]
    fn lexes_insert_statement_from_completion_criteria() {
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
    fn keywords_are_case_insensitive_but_identifiers_keep_their_case() {
        let tokens = Lexer::new("select User_Name from Users")
            .tokenize()
            .unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Select,
                Token::Identifier("User_Name".to_string()),
                Token::From,
                Token::Identifier("Users".to_string()),
                Token::EndOfInput,
            ]
        );
    }

    #[test]
    fn reports_unterminated_string() {
        let error = Lexer::new("SELECT 'unterminated string")
            .tokenize()
            .unwrap_err();

        assert_eq!(error, LexError::UnterminatedString { position: 7 });
    }

    #[test]
    fn lexes_comparison_operators_and_escaped_quote() {
        let tokens = Lexer::new("a != 'Liam''s' <> b < 2 > 1")
            .tokenize()
            .unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("a".to_string()),
                Token::NotEqual,
                Token::StringLiteral("Liam's".to_string()),
                Token::NotEqual,
                Token::Identifier("b".to_string()),
                Token::LessThan,
                Token::Integer(2),
                Token::GreaterThan,
                Token::Integer(1),
                Token::EndOfInput,
            ]
        );
    }
}
