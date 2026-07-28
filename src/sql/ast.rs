use crate::{DataType, Value};


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement { 
    CreateTable(CreateTableStatement),
    Insert(InsertStatement),
    Select(SelectStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTableStatement {
    pub table: String, 
    pub columns: Vec<ColumnDefinition>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDefinition {
    pub name: String, 
    pub data_type: DataType
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertStatement {
    pub table: String, 
    pub values: Vec<Expression>
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectStatement {
    pub projections: Vec<SelectItem>,
    pub table: String,
    pub filter: Option<Expression>,
}

/// One item between `SELECT` and `FROM`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectItem {
    /// `*` requests every column.
    Wildcard,
    /// Named columns and future calculated projections share expression syntax.
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatement {
    pub table: String,
    pub assignments: Vec<Assignment>,
    pub filter: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub column: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteStatement {
    pub table: String,
    pub filter: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Value),
    Column(String),
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>
    },
    Unary {
        operator: UnaryOperator,
        expression: Box<Expression>,
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Or,
    And,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
}
