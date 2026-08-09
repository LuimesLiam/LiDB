use super::ast::{
    BinaryOperator, ColumnDefinition, CreateTableStatement, Expression, SelectItem, Statement,
    UnaryOperator,
};
use crate::{DataType, Database, Schema, Value};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Binary};
use std::ops::Bound;


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundExpression {
    Literal(Value),
    Column {
        column_index: usize, 
        data_type: DataType
    },
    Binary {
        left: Box<BoundExpression>,
        operator: BinaryOperator, 
        right: Box<BoundExpression>,
        data_type: DataType
    },
    Unary {
        operator: UnaryOperator,
        expression: Box<BoundExpression>,
        data_type: DataType
    }
}



impl BoundExpression {
    /// The expression's result type. A bare `NULL` has no type until it is
    /// placed in a typed context such as an INSERT target column.
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            BoundExpression::Literal(value) => value.data_type(),
            BoundExpression::Column { data_type, .. }
            | BoundExpression::Binary { data_type, .. }
            | BoundExpression::Unary { data_type, .. } => Some(*data_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundStatement {
    CreateTable(CreateTableStatement),
    Insert(BoundInsertStatement),
    Select(BoundSelectStatement),
    Update(BoundUpdateStatement),
    Delete(BoundDeleteStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInsertStatement {
    pub table: String,
    pub values: Vec<BoundExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundSelectStatement {
    pub table: String,
    /// `SELECT *` is expanded during binding, so execution sees only indexed
    /// expressions.
    pub projections: Vec<BoundExpression>,
    pub filter: Option<BoundExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundUpdateStatement {
    pub table: String,
    pub assignments: Vec<BoundAssignment>,
    pub filter: Option<BoundExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundAssignment {
    pub column_index: usize,
    pub data_type: DataType,
    pub value: BoundExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundDeleteStatement {
    pub table: String,
    pub filter: Option<BoundExpression>,
}

/// A grammatical SQL statement that is not valid for the current catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindError {
    TableAlreadyExists {
        table: String,
    },
    TableNotFound {
        table: String,
    },
    ColumnNotFound {
        table: String,
        column: String,
    },
    /// Kept as a first-class semantic error for when TinyRel gains joins.
    AmbiguousColumn {
        column: String,
    },
    DuplicateColumn {
        column: String,
    },
    DuplicateAssignment {
        column: String,
    },
    WrongColumnCount {
        table: String,
        expected: usize,
        actual: usize,
    },
    TypeMismatch {
        context: String,
        expected: DataType,
        actual: DataType,
    },
    NullNotAllowed {
        table: String,
        column: String,
    },
    IncompatibleTypes {
        operator: BinaryOperator,
        left: DataType,
        right: DataType,
    },
    InvalidUnaryOperand {
        operator: UnaryOperator,
        expected: DataType,
        actual: Option<DataType>,
    },
    ExpectedBoolean {
        clause: &'static str,
        actual: Option<DataType>,
    },
}

/// A descriptive alias for callers that call this stage semantic analysis.
pub type SemanticError = BindError;
pub type BinderError = BindError;

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindError::TableAlreadyExists { table } => {
                write!(f, "table '{table}' already exists")
            }
            BindError::TableNotFound { table } => write!(f, "table '{table}' does not exist"),
            BindError::ColumnNotFound { table, column } => {
                write!(f, "column '{column}' does not exist in table '{table}'")
            }
            BindError::AmbiguousColumn { column } => {
                write!(f, "column '{column}' is ambiguous")
            }
            BindError::DuplicateColumn { column } => {
                write!(f, "column '{column}' is defined more than once")
            }
            BindError::DuplicateAssignment { column } => {
                write!(f, "column '{column}' is assigned more than once")
            }
            BindError::WrongColumnCount {
                table,
                expected,
                actual,
            } => write!(
                f,
                "table '{table}' expects {expected} values, but received {actual}"
            ),
            BindError::TypeMismatch {
                context,
                expected,
                actual,
            } => write!(f, "{context} expects {expected}, but received {actual}"),
            BindError::NullNotAllowed { table, column } => {
                write!(
                    f,
                    "column '{column}' in table '{table}' does not allow NULL"
                )
            }
            BindError::IncompatibleTypes {
                operator,
                left,
                right,
            } if operator.is_comparison() => {
                write!(f, "Cannot compare {left} with {right}")
            }
            BindError::IncompatibleTypes {
                operator,
                left,
                right,
            } => write!(
                f,
                "operator {} cannot be applied to {left} and {right}",
                operator.symbol()
            ),
            BindError::InvalidUnaryOperand {
                operator,
                expected,
                actual,
            } => match actual {
                Some(actual) => write!(
                    f,
                    "operator {} expects {expected}, but received {actual}",
                    operator.symbol()
                ),
                None => write!(
                    f,
                    "operator {} cannot be applied to untyped NULL",
                    operator.symbol()
                ),
            },
            BindError::ExpectedBoolean { clause, actual } => match actual {
                Some(actual) => write!(f, "{clause} must be BOOLEAN, but received {actual}"),
                None => write!(f, "{clause} cannot be an untyped NULL"),
            },
        }
    }
}

impl Error for BindError {}

trait OperatorInfo{
    fn symbol(self) -> &'static str;
}


impl BinaryOperator{
    fn is_comparison(self) -> bool {
        matches!{
            self, 
            Self::Equal 
            | Self::NotEqual
            | Self::LessThan
            | Self::LessThanOrEqual
            | Self::GreaterThan
            | Self::GreaterThanOrEqual
        }
    }
}

impl OperatorInfo for BinaryOperator {
    fn symbol(self) -> &'static str {
        match self {
            Self::Or => "OR",
            Self::And => "AND",
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
        }
    }
}

impl OperatorInfo for UnaryOperator {
    fn symbol(self) -> &'static str {
        match self {
            Self::Not => "NOT",
            Self::Negate => "-",
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Binder<'a>{
    database: &'a Database,
}

impl<'a> Binder<'a> {
    pub fn new(database: &'a Database) -> Self{
        Self {database}
    }

    pub fn bind (&self, statement: impl Borrow<Statement>) -> Result<BoundStatement, BindError> {
        match statement.borrow(){
            Statement::CreateTable(statement) => self.bind_create_table(statement),
            Statement::Insert(statement) => {
                let schema = self.schema(&statement.table)?; 
                if statement.values.len() != schema.len() {
                    return Err(BindError::WrongColumnCount { table: statement.table.clone(), expected: schema.len() , actual: statement.values.len() });

                }

                let mut values = Vec::with_capacity(statement.values.len());

                for (expression, column) in statement.values.iter().zip(schema.columns()){
                    let value = self.bind_expression(expression, None, &statement.table)?;
                    self.require_assignable(&statement.table, column.name(), column.data_type(), column.is_nullable(), &value)?;
                    values.push(value);
                }
                Ok(
                BoundStatement::Insert(BoundInsertStatement{
                    table: statement.table.clone(),
                    values,
                })
            )}
            Statement::Select(statement) => {
                let schema = self.schema(&statement.table)?;
                let mut projections = Vec::new();

                for projection in &statement.projections {
                    match projection {
                        SelectItem::Wildcard => {
                            // Expand this now so execution never handles a wildcard.
                            // Bound columns are just row indexes, which makes scans simpler.
                            projections.extend(schema.columns().iter().enumerate().map(
                                |(column_index, column)| BoundExpression::Column { column_index, data_type: column.data_type() })
                            
                            );
                        }
                        SelectItem::Expression(expression) => projections.push(
                            self.bind_expression(&expression, Some(schema), &statement.table)?,
                        )
                    }
                }

                let filter = self.bind_filter(statement.filter.as_ref(), schema, &statement.table)?;
                Ok(
                    BoundStatement::Select(BoundSelectStatement { table: statement.table.clone(), projections, filter })
                )
            }
            Statement::Update(statement) => {
                let schema = self.schema(&statement.table)?;
                let mut assignments = Vec::with_capacity(statement.assignments.len());
                let mut assigned = HashSet::new();

                for assignment in &statement.assignments {
                    if !assigned.insert(assignment.column.as_str()) {
                        return Err(BindError::DuplicateAssignment {
                            column: assignment.column.clone(),
                        });
                    }
                    let column_index =
                        self.column_index(schema, &statement.table, &assignment.column)?;
                    let column = &schema.columns()[column_index];
                    let value =
                        self.bind_expression(&assignment.value, Some(schema), &statement.table)?;
                    self.require_assignable(
                        &statement.table,
                        column.name(),
                        column.data_type(),
                        column.is_nullable(),
                        &value,
                    )?;
                    assignments.push(BoundAssignment {
                        column_index,
                        data_type: column.data_type(),
                        value,
                    });
                }

                let filter =
                    self.bind_filter(statement.filter.as_ref(), schema, &statement.table)?;
                Ok(BoundStatement::Update(BoundUpdateStatement {
                    table: statement.table.clone(),
                    assignments,
                    filter,
                }))
            }
            Statement::Delete(statement) => {
                let schema = self.schema(&statement.table)?;
                let filter =
                    self.bind_filter(statement.filter.as_ref(), schema, &statement.table)?;
                Ok(BoundStatement::Delete(BoundDeleteStatement {
                    table: statement.table.clone(),
                    filter,
                }))
            }
        }

    }
    
    fn bind_create_table(&self, statement: &CreateTableStatement, ) -> Result<BoundStatement, BindError>{
        if self.database.table(&statement.table).is_ok() {
            return Err(BindError::TableAlreadyExists { table: statement.table.clone() });
        }
        let mut names = HashSet::new();
        for ColumnDefinition { name , .. } in &statement.columns {
            if !names.insert(name) {
                return Err(BindError::DuplicateColumn { column: name.clone() });
            }
        }
        Ok(BoundStatement::CreateTable(statement.clone()))
    }

    fn schema(&self, table: &str) -> Result<&Schema, BindError> {
        self.database
            .table(table)
            .map(|table| table.schema())
            .map_err(|_| BindError::TableNotFound { table: table.to_string() })
    }
    
    fn column_index(&self, schema: &Schema, table: &str, column: &str) -> Result<usize, BindError> {
        schema
            .column_index(column)
            .ok_or_else(|| BindError::ColumnNotFound {
                table: table.to_string(),
                column: column.to_string(),
            })
    }

    fn bind_filter(
        &self,
        expression: Option<&Expression>,
        schema: &Schema,
        table: &str,
    ) -> Result<Option<BoundExpression>, BindError> {
        let Some(expression) = expression else {
            return Ok(None);
        };
        let expression = self.bind_expression(expression, Some(schema), table)?;
        // Catch non-boolean WHERE expressions before we scan any rows.
        // The executor can then treat this as a runtime value check only.
        if expression.data_type() != Some(DataType::Boolean) {
            return Err(BindError::ExpectedBoolean {
                clause: "WHERE expression",
                actual: expression.data_type(),
            });
        }
        Ok(Some(expression))
    }

    fn bind_expression(
        &self,
        expression: &Expression,
        schema: Option<&Schema>,
        table: &str,
    ) -> Result<BoundExpression, BindError> {
        match expression {
            Expression::Literal(value) => Ok(BoundExpression::Literal(value.clone())),
            Expression::Column(column) => {
                let Some(schema) = schema else {
                    return Err(BindError::ColumnNotFound {
                        table: table.to_string(),
                        column: column.clone(),
                    });
                };
                let column_index = self.column_index(schema, table, column)?;
                Ok(BoundExpression::Column {
                    column_index,
                    data_type: schema.columns()[column_index].data_type(),
                })
            }
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.bind_expression(left, schema, table)?;
                let right = self.bind_expression(right, schema, table)?;
                // Store the result type now; execution should not need schema lookups.
                let data_type = self.binary_result_type(*operator, &left, &right)?;
                Ok(BoundExpression::Binary {
                    left: Box::new(left),
                    operator: *operator,
                    right: Box::new(right),
                    data_type,
                })
            }
            Expression::Unary {
                operator,
                expression,
            } => {
                let expression = self.bind_expression(expression, schema, table)?;
                let expected = match operator {
                    UnaryOperator::Not => DataType::Boolean,
                    UnaryOperator::Negate => DataType::Integer,
                };
                if expression.data_type() != Some(expected) {
                    return Err(BindError::InvalidUnaryOperand {
                        operator: *operator,
                        expected,
                        actual: expression.data_type(),
                    });
                }
                Ok(BoundExpression::Unary {
                    operator: *operator,
                    expression: Box::new(expression),
                    data_type: expected,
                })
            }
        }
    }

    fn binary_result_type(
        &self,
        operator: BinaryOperator,
        left: &BoundExpression,
        right: &BoundExpression,
    ) -> Result<DataType, BindError> {
        let left_type = left.data_type();
        let right_type = right.data_type();

        if operator.is_comparison() {
            // NULL can compare without a concrete type; runtime gives the final value.
            if let (Some(left), Some(right)) = (left_type, right_type)
                && left != right
            {
                return Err(BindError::IncompatibleTypes {
                    operator,
                    left,
                    right,
                });
            }
            return Ok(DataType::Boolean);
        }

        let expected = match operator {
            BinaryOperator::And | BinaryOperator::Or => DataType::Boolean,
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide => DataType::Integer,
            _ => unreachable!("comparisons returned above"),
        };

        if left_type == Some(expected) && right_type == Some(expected) {
            Ok(expected)
        } else {
            Err(BindError::IncompatibleTypes {
                operator,
                left: left_type.unwrap_or(expected),
                right: right_type.unwrap_or(expected),
            })
        }
    }

    fn require_assignable(
        &self,
        table: &str,
        column: &str,
        expected: DataType,
        nullable: bool,
        expression: &BoundExpression,
    ) -> Result<(), BindError> {
        match expression.data_type() {
            None if !nullable => Err(BindError::NullNotAllowed {
                table: table.to_string(),
                column: column.to_string(),
            }),
            None => Ok(()),
            Some(actual) if actual != expected => Err(BindError::TypeMismatch {
                context: format!("column '{column}' in table '{table}'"),
                expected,
                actual,
            }),
            Some(_) => Ok(()),
        }
    }

}


/// Convenience function for one-shot semantic analysis.
pub fn bind_statement(
    database: &Database,
    statement: impl Borrow<Statement>,
) -> Result<BoundStatement, BindError> {
    Binder::new(database).bind(statement)
}
