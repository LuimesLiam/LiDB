use super::ast::{
    Assignment, BinaryOperator, ColumnDefinition, CreateTableStatement, DeleteStatement,
    Expression, InsertStatement, SelectItem, SelectStatement, Statement, UnaryOperator,
    UpdateStatement,
};
use super::{LexError, Lexer, Token};
use crate::sql::SelectItem::Wildcard;
use crate::sql::Token::{Create, Select, Update};
use crate::sql::token;
use crate::{DataType, Value};
use std::error::Error;
use std::fmt;

/// A grammatical problem in an otherwise successfully tokenized SQL query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnexpectedToken {
        expected: String,
        found: Token,
        position: usize,
    },
    UnexpectedEnd {
        expected: String,
        position: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken {
                expected,
                found,
                position,
            } => write!(
                formatter,
                "expected {expected} at token {position}, but found {found:?}"
            ),
            ParseError::UnexpectedEnd { expected, position } => {
                write!(
                    formatter,
                    "expected {expected} at token {position}, but reached the end of input"
                )
            }
        }
    }
}

impl Error for ParseError {}

/// An error from either stage of the SQL front end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlError {
    Lex(LexError),
    Parse(ParseError),
}

impl fmt::Display for SqlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlError::Lex(error) => write!(formatter, "lexer error: {error}"),
            SqlError::Parse(error) => write!(formatter, "parser error: {error}"),
        }
    }
}

impl Error for SqlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SqlError::Lex(error) => Some(error),
            SqlError::Parse(error) => Some(error),
        }
    }
}

impl From<LexError> for SqlError {
    fn from(error: LexError) -> Self {
        Self::Lex(error)
    }
}

impl From<ParseError> for SqlError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}


/// Lexes and parses exactly one SQL statement.
pub fn parse_sql(input: &str) -> Result<Statement, SqlError> {
    let tokens = Lexer::new(input).tokenize()?;
    Ok(Parser::new(tokens).parse()?)
}

/// Lexes and parses multiple statements separated by semicolons.
pub fn parse_sql_statements(input: &str) -> Result<Vec<Statement>, SqlError> {
    let tokens = Lexer::new(input).tokenize()?;
    Ok(Parser::new(tokens).parse_all()?)
}

#[derive(Debug, Clone)]

pub struct Parser {
    tokens: Vec<Token>,
    position: usize, 
}


impl Parser{
    pub fn new(mut tokens: Vec<Token>) -> Self {
        if tokens.last() != Some(&Token::EndOfInput) {
            tokens.push(Token::EndOfInput);
        }

        Self {
            tokens, 
            position: 0
        }


    }

    pub fn parse(mut self) -> Result<Statement, ParseError> {
        let statement = self.parse_statement()?;
        self.consume(&Token::Semicolon);
        self.expect(&Token::EndOfInput, "end of input")?;
        Ok(statement)
    }


    pub fn parse_all(mut self) -> Result<Vec<Statement>, ParseError> {
        let mut statement = Vec::new();

        while !self.check(&Token::EndOfInput){
            statement.push(self.parse_statement()?);
            if !self.consume(&Token::Semicolon) && ! self.check(&Token::EndOfInput){
                return Err(self.error("; between SQL statements"))
            }
            
        }

        Ok(statement)
    }

    pub fn parse_statement(&mut self)-> Result<Statement, ParseError> {
        match self.current() {
            Token::Create => self.parse_create_table(),
            Token::Insert => self.parse_insert(),
            Token::Select => self.parse_select(),
            Token::Update => self.parse_updatE(),
            Token::Delete => self.parse_delete(),
            _ => Err(self.error("CREATE, INSERT, SELECT, UPdate, or DELETE"))
        }
    }

    fn parse_create_table(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Create, "CREATE")?;
        self.expect(&Token::Table, "TABLE")?;
        let table = self.expect_indentifier("table name")?;
        self.expect(&Token::LeftParen, "'('")?;

        let mut columns = Vec::new(); 
        loop {
            let name = self.expect_indentifier("column name")?;
            let data_type = self.parse_data_type()?;
            columns.push(ColumnDefinition {name, data_type}); 
        
            if !self.consume(&Token::Comma){
                break;
            }
        }

        self.expect(&Token::RightParen, "')'")?;
        Ok(Statement::CreateTable(CreateTableStatement { table, columns }))
    }

    fn parse_data_type(&mut self) -> Result<DataType, ParseError>{
        let data_type = match self.current() {
            Token::IntegerType => DataType::Integer, 
            Token::BooleanType => DataType::Boolean,
            Token::TextType => DataType::Text,
            _ => return Err(self.error("INT, BOOL, or TEXT"))
        };
        self.advance();
        Ok(data_type)
    }

    fn parse_insert(&mut self) -> Result<Statement, ParseError>{
        self.expect(&Token::Insert, "INSERT")?;
        self.expect(&Token::Into,"INTO")?;
        let table = self.expect_indentifier("table name")?;
        self.expect(&Token::Values, "VALUE")?;
        self.expect(&Token::LeftParen, "'('")?;
        
        let mut values = Vec::new();

        if !self.check(&Token::RightParen){
            loop {
                values.push(self.parse_expression()?);
                if !self.consume(&Token::Comma){
                    break;
                }
            }
        }

        self.expect(&Token::RightParen, "')'")?;
        Ok(Statement::Insert(InsertStatement { table, values }))
    }

    fn parse_select(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Select, "SELECT")?;
        let mut projections = Vec::new(); 

        loop {
            let item = if self.consume(&Token::Asterisk){
                SelectItem:: Wildcard
            } else {
                SelectItem::Expression(self.parse_expression()?)
            };
            projections.push(item);

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        self.expect(&Token::From, "FROM")?;
        let table = self.expect_indentifier("table name")?;
        let filter = if self.consume(&Token::Where){
            Some(self.parse_expression()?)
        }
        else {
            None
        };

        Ok(Statement::Select(SelectStatement { projections, table, filter }))

    }

    fn parse_updatE(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Update, "UPDATE")?;
        let table = self.expect_indentifier("table name")?;
        self.expect(&Token::Set, "SET")?;
        let mut assignments = Vec::new(); 

        loop {
            let column = self.expect_indentifier("column name")?;
            self.expect(&Token::Equal, "'='")?;
            let value = self.parse_expression()?;
            assignments.push(Assignment{column, value});

            if !self.consume(&Token::Comma){
                break;
            }
        }

        let filter = if self.consume(&Token::Where){
            Some(self.parse_expression()?)
        }else{
            None
        };

        Ok(Statement::Update(UpdateStatement { table, assignments, filter }))


        
    }
        fn parse_delete(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Delete, "DELETE")?;
        self.expect(&Token::From, "FROM")?;
        let table = self.expect_indentifier("table name")?;
        let filter = if self.consume(&Token::Where) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Statement::Delete(DeleteStatement { table, filter }))
    }


    pub fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_and()?;

        while self.consume(&Token::Or) {
            let right = self.parse_and()?;
            expression = Self::binary(expression, BinaryOperator::Or, right);
        }

        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_comparison()?;

        while self.consume(&Token::And) {
            let right = self.parse_comparison()?;
            expression = Self::binary(expression, BinaryOperator::And, right);
        }

        Ok(expression)
    }

    fn parse_comparison(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_additive()?;

        loop {
            let operator = match self.current() {
                Token::Equal => BinaryOperator::Equal,
                Token::NotEqual => BinaryOperator::NotEqual,
                Token::LessThan => BinaryOperator::LessThan,
                Token::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                Token::GreaterThan => BinaryOperator::GreaterThan,
                Token::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            expression = Self::binary(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_multiplicative()?;

        loop {
            let operator = match self.current() {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            expression = Self::binary(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_unary()?;

        loop {
            let operator = match self.current() {
                Token::Asterisk => BinaryOperator::Multiply,
                Token::Slash => BinaryOperator::Divide,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            expression = Self::binary(expression, operator, right);
        }

        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if self.consume(&Token::Not) {
            return Ok(Expression::Unary {
                operator: UnaryOperator::Not,
                expression: Box::new(self.parse_unary()?),
            });
        }

        if self.consume(&Token::Minus) {
            return Ok(Expression::Unary {
                operator: UnaryOperator::Negate,
                expression: Box::new(self.parse_unary()?),
            });
        }

        // Unary plus changes no value, so the AST can omit it.
        if self.consume(&Token::Plus) {
            return self.parse_unary();
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        let expression = match self.current().clone() {
            Token::Integer(value) => Expression::Literal(Value::Integer(value)),
            Token::StringLiteral(value) => Expression::Literal(Value::Text(value)),
            Token::True => Expression::Literal(Value::Boolean(true)),
            Token::False => Expression::Literal(Value::Boolean(false)),
            Token::Null => Expression::Literal(Value::Null),
            Token::Identifier(name) => Expression::Column(name),
            Token::LeftParen => {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect(&Token::RightParen, "')'")?;
                return Ok(inner);
            }
            _ => return Err(self.error("a literal, column name, or parenthesized expression")),
        };

        self.advance();
        Ok(expression)
    }

    fn binary(left: Expression, operator: BinaryOperator, right: Expression) -> Expression {
        // Box gives recursive enum children a fixed pointer size. Without it,
        // Expression would contain itself directly and have no finite size.
        Expression::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }
    }

    fn consume(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        }
        else {
            false
        }
    }

    fn check(&self, expected: &Token) -> bool {
        self.current() == expected
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::EndOfInput)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if token != Token::EndOfInput {
            self.position += 1;
        }

        token
    }

    fn expect (&mut self, expected: &Token, description: &str) -> Result<(), ParseError> {
        if self.consume(expected){
            Ok(())
        }
        else {
            Err(self.error(description))
        }
    }

    fn expect_indentifier(&mut self, description: &str) -> Result<String, ParseError> {
        if let Token::Identifier(name) = self.current().clone() {
            self.advance();
            Ok(name)
        }
        else {
            Err(self.error(description))
        }
    }


    fn error(&self, expected: &str) -> ParseError {
        if self.current() == &Token::EndOfInput {
            ParseError::UnexpectedEnd { expected: expected.to_string(), position: self.position }
        }
        else{
            ParseError::UnexpectedToken { expected: expected.to_string(), found: self.current().clone(), position: self.position }
        }
    }
}






#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Statement {
        parse_sql(input).unwrap()
    }

    #[test]
    fn parses_create_table() {
        assert_eq!(
            parse("CREATE TABLE users (id INTEGER, name TEXT);"),
            Statement::CreateTable(CreateTableStatement {
                table: "users".to_string(),
                columns: vec![
                    ColumnDefinition {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                    },
                    ColumnDefinition {
                        name: "name".to_string(),
                        data_type: DataType::Text,
                    },
                ],
            })
        );
    }

    #[test]
    fn parses_insert() {
        assert_eq!(
            parse("INSERT INTO users VALUES (1, 'Liam');"),
            Statement::Insert(InsertStatement {
                table: "users".to_string(),
                values: vec![
                    Expression::Literal(Value::Integer(1)),
                    Expression::Literal(Value::Text("Liam".to_string())),
                ],
            })
        );
    }

    #[test]
    fn parses_select_and_respects_and_precedence() {
        let statement = parse("SELECT id, name FROM users WHERE id >= 1 AND name != 'Bob';");

        let Statement::Select(select) = statement else {
            panic!("expected SELECT statement");
        };

        assert_eq!(select.table, "users");
        assert_eq!(
            select.projections,
            vec![
                SelectItem::Expression(Expression::Column("id".to_string())),
                SelectItem::Expression(Expression::Column("name".to_string())),
            ]
        );
        assert_eq!(
            select.filter,
            Some(Expression::Binary {
                left: Box::new(Expression::Binary {
                    left: Box::new(Expression::Column("id".to_string())),
                    operator: BinaryOperator::GreaterThanOrEqual,
                    right: Box::new(Expression::Literal(Value::Integer(1))),
                }),
                operator: BinaryOperator::And,
                right: Box::new(Expression::Binary {
                    left: Box::new(Expression::Column("name".to_string())),
                    operator: BinaryOperator::NotEqual,
                    right: Box::new(Expression::Literal(Value::Text("Bob".to_string()))),
                }),
            })
        );
    }

    #[test]
    fn multiplication_has_higher_precedence_than_addition() {
        let statement = parse("SELECT 1 + 2 * 3 FROM numbers;");
        let Statement::Select(select) = statement else {
            panic!("expected SELECT statement");
        };

        assert_eq!(
            select.projections[0],
            SelectItem::Expression(Expression::Binary {
                left: Box::new(Expression::Literal(Value::Integer(1))),
                operator: BinaryOperator::Add,
                right: Box::new(Expression::Binary {
                    left: Box::new(Expression::Literal(Value::Integer(2))),
                    operator: BinaryOperator::Multiply,
                    right: Box::new(Expression::Literal(Value::Integer(3))),
                }),
            })
        );
    }

    #[test]
    fn parentheses_override_precedence() {
        let statement = parse("SELECT (1 + 2) * 3 FROM numbers;");
        let Statement::Select(select) = statement else {
            panic!("expected SELECT statement");
        };

        assert_eq!(
            select.projections[0],
            SelectItem::Expression(Expression::Binary {
                left: Box::new(Expression::Binary {
                    left: Box::new(Expression::Literal(Value::Integer(1))),
                    operator: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Value::Integer(2))),
                }),
                operator: BinaryOperator::Multiply,
                right: Box::new(Expression::Literal(Value::Integer(3))),
            })
        );
    }

    #[test]
    fn parses_update_and_delete() {
        let statements = parse_sql_statements(
            "UPDATE users SET name = 'Ada', active = true WHERE id = 1;
             DELETE FROM users WHERE active = false;",
        )
        .unwrap();

        assert!(matches!(statements[0], Statement::Update(_)));
        assert!(matches!(statements[1], Statement::Delete(_)));
    }

    #[test]
    fn malformed_statement_returns_error() {
        let error = parse_sql("SELECT name users;").unwrap_err();

        assert!(matches!(
            error,
            SqlError::Parse(ParseError::UnexpectedToken {
                found: Token::Identifier(_),
                ..
            })
        ));
    }
}



