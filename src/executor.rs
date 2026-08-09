use crate::BoundExpression::Unary;
use crate::sql::{
    BinaryOperator, BindError, Binder, BoundDeleteStatement, BoundExpression, BoundInsertStatement,
    BoundSelectStatement, BoundStatement, BoundUpdateStatement, SqlError, UnaryOperator, parse_sql,
    parse_sql_statements,
};
use crate::{Column, Database, DbError, Row, Schema, Value};
use std::error::Error;
use std::fmt;


pub trait Executor {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseError{
    Sql(SqlError),
    Bind(BindError),
    Storage(DbError),
    DivisionByZero, 
    IntegerOverflow,
    InvalidColumnIndex {index: usize, row_len: usize},
    ExpectedBoolean {actual: Value}
}


impl fmt::Display for DatabaseError{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(error) => write!(f, "{error}"),
            Self::Bind(error) => write!(f, "binding error: {error}"),
            Self::Storage(error) => write!(f, "storage error: {error}"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::IntegerOverflow => write!(f, "integer arithmetic overflow"),
            Self::InvalidColumnIndex { index, row_len } => {
                write!(
                    f,
                    "column index {index} is outside a row of length {row_len}"
                )
            }
            Self::ExpectedBoolean { actual } => {
                write!(f, "predicate evaluated to {actual}, not BOOLEAN")
            }
        }
    }
}

impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            Self::Bind(error) => Some(error),
            Self::Storage(error) => Some(error),
            _ => None
        }
    }
}


impl From<SqlError> for DatabaseError {
    fn from(value: SqlError) -> Self {
        Self::Sql(value)
    }
}
impl From<BindError> for DatabaseError {
    fn from(value: BindError) -> Self {
        Self::Bind(value)
    }
}
impl From<DbError> for DatabaseError {
    fn from(value: DbError) -> Self {
        Self::Storage(value)
    }
}


impl BoundExpression {
    pub fn evaluate(&self, roW: &Row) -> Result<Value,DatabaseError> {
        match self {
            Self::Literal(value) => Ok(value.clone()),
            Self::Column { column_index,.. } => {
                roW.get(*column_index)
                    .cloned()
                    .ok_or(DatabaseError::InvalidColumnIndex { index: *column_index, row_len: roW.len() })
            }
            Self::Unary { operator, expression, .. } =>{
                let value = expression.evaluate(roW)?;
                evaluate_unary(*operator, value)
            },
            Self::Binary { left,
                operator,
                right,
                ..
            } => {
                let left = left.evaluate(roW)?;

                // Avoid evaluating the rhs when the answer is already known.
                // This matters if the rhs would otherwise fail (for example, `false AND 1 / 0`).
                if (*operator == BinaryOperator::And && left == Value::Boolean(false))
                    || (*operator == BinaryOperator::Or && left == Value::Boolean(true)){
                    return Ok(left);
                }
                let right = right.evaluate(roW)?;
                evaluate_binary(left, *operator, right)
                
            }
        }
    }
}



fn evaluate_unary(operator: UnaryOperator, value: Value) -> Result<Value,DatabaseError> {
    match (operator, value) {
        (_, Value::Null) => Ok(Value::Null),
        (UnaryOperator::Not, Value::Boolean(value)) => Ok(Value::Boolean(value)),
        (UnaryOperator::Negate, Value::Integer(value)) => value.checked_neg()
            .map(Value::Integer)
            .ok_or(DatabaseError::IntegerOverflow), 
        (_, actual) => Err(DatabaseError::ExpectedBoolean { actual }),
    }
}


fn evaluate_binary(
    left: Value,
    operator: BinaryOperator,
    right: Value,
) -> Result<Value, DatabaseError> {
    use BinaryOperator as Op;
    use Value as V;

    if left == V::Null || right == V::Null {
        // Keep SQL's three-value logic here, not in each operator arm.
        // `NULL` means unknown, except false AND unknown and true OR unknown.
        return match (operator, left, right) {
            (Op::And, V::Boolean(false), _) | (Op::And, _, V::Boolean(false)) => {
                Ok(V::Boolean(false))
            }
            (Op::Or, V::Boolean(true), _) | (Op::Or, _, V::Boolean(true)) => Ok(V::Boolean(true)),
            _ => Ok(V::Null),
        };
    }

    match (operator, left, right) {
        (Op::And, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(a && b)),
        (Op::Or, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(a || b)),
        (Op::Equal, a, b) => Ok(V::Boolean(a == b)),
        (Op::NotEqual, a, b) => Ok(V::Boolean(a != b)),
        (Op::LessThan, V::Integer(a), V::Integer(b)) => Ok(V::Boolean(a < b)),
        (Op::LessThanOrEqual, V::Integer(a), V::Integer(b)) => Ok(V::Boolean(a <= b)),
        (Op::GreaterThan, V::Integer(a), V::Integer(b)) => Ok(V::Boolean(a > b)),
        (Op::GreaterThanOrEqual, V::Integer(a), V::Integer(b)) => Ok(V::Boolean(a >= b)),
        (Op::LessThan, V::Text(a), V::Text(b)) => Ok(V::Boolean(a < b)),
        (Op::LessThanOrEqual, V::Text(a), V::Text(b)) => Ok(V::Boolean(a <= b)),
        (Op::GreaterThan, V::Text(a), V::Text(b)) => Ok(V::Boolean(a > b)),
        (Op::GreaterThanOrEqual, V::Text(a), V::Text(b)) => Ok(V::Boolean(a >= b)),
        (Op::LessThan, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(!a & b)),
        (Op::LessThanOrEqual, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(a <= b)),
        (Op::GreaterThan, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(a & !b)),
        (Op::GreaterThanOrEqual, V::Boolean(a), V::Boolean(b)) => Ok(V::Boolean(a >= b)),
        (Op::Add, V::Integer(a), V::Integer(b)) => checked(a.checked_add(b)),
        (Op::Subtract, V::Integer(a), V::Integer(b)) => checked(a.checked_sub(b)),
        (Op::Multiply, V::Integer(a), V::Integer(b)) => checked(a.checked_mul(b)),
        (Op::Divide, V::Integer(_), V::Integer(0)) => Err(DatabaseError::DivisionByZero),
        (Op::Divide, V::Integer(a), V::Integer(b)) => checked(a.checked_div(b)),
        (_, actual, _) => Err(DatabaseError::ExpectedBoolean { actual }),
    }
}


fn checked(value: Option<i64>) -> Result<Value, DatabaseError> {
    value
        .map(Value::Integer)
        .ok_or(DatabaseError::IntegerOverflow)
}

fn matches_filter(filter: Option<&BoundExpression>, row: &Row) -> Result<bool, DatabaseError> {
    match filter {
        None => Ok(true),
        Some(filter) => match filter.evaluate(row)? {
            Value::Boolean(value) => Ok(value),
            // WHERE only keeps rows that are definately true.
            Value::Null => Ok(false),
            actual => Err(DatabaseError::ExpectedBoolean { actual }),
        },
    }
}


pub struct TableScanExecutor{
    rows: Vec<Row>, 
    position: usize,
}

impl TableScanExecutor {
    pub fn new (rows: Vec<Row>) -> Self {
        Self {rows, position: 0}
    }

    pub fn from_table(database: &Database, table: &str) -> Result<Self, DatabaseError> {
        Ok(Self::new(database.table(table)?.rows().to_vec()))
    }
}

impl Executor for TableScanExecutor { 
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        let Some(row) = self.rows.get(self.position).cloned() else {
            return Ok(None);
        };
        self.position +=1; 
        Ok(Some(row))
    }
}


pub struct FilterExecutor<'a> {
    child: Box<dyn Executor + 'a>,
    predicate: BoundExpression,
}

impl<'a> FilterExecutor<'a> {
    pub fn new(child: Box<dyn Executor + 'a>, predicate:BoundExpression) -> Self {
        Self {child, predicate}
    }
}

impl Executor for FilterExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>,DatabaseError> {
        loop {
            let Some(row) = self.child.next()? else {
                return Ok(None);
            };
            if matches_filter(Some(&self.predicate), &row)? {
                return Ok(Some(row));
            }
        }
    }
}



pub struct ProjectionExecutor<'a> {
    child: Box<dyn Executor + 'a>,
    projections: Vec<BoundExpression>,
}

impl<'a> ProjectionExecutor<'a> {
    pub fn new(child: Box<dyn Executor + 'a>, projections: Vec<BoundExpression>) -> Self {
        Self { child, projections }
    }
}

impl Executor for ProjectionExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        let Some(row) = self.child.next()? else {
            return Ok(None);
        };
        let values = self
            .projections
            .iter()
            .map(|expression| expression.evaluate(&row))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Row::new(values)))
    }
}



pub struct InsertExecutor<'a> {
    database: &'a mut Database,
    statement: BoundInsertStatement,
    finished: bool,
}

impl<'a> InsertExecutor<'a> {
    pub fn new(database: &'a mut Database, statement: BoundInsertStatement) -> Self {
        Self {
            database,
            statement,
            finished: false,
        }
    }
}

impl Executor for InsertExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        if self.finished {
            return Ok(None);
        }
        self.finished = true;
        let empty = Row::new(Vec::new());
        let values = self
            .statement
            .values
            .iter()
            .map(|value| value.evaluate(&empty))
            .collect::<Result<Vec<_>, _>>()?;
        let row = Row::new(values);
        self.database
            .table_mut(&self.statement.table)?
            .insert(row.clone())?;
        Ok(Some(row))
    }
}

pub struct UpdateExecutor<'a> {
    database: &'a mut Database, 
    statement: BoundUpdateStatement, 
    output:Option<std::vec::IntoIter<Row>>
}


impl <'a> UpdateExecutor<'a> {
    pub fn new(database: &'a mut Database, statement: BoundUpdateStatement) -> Self {
        Self {
            database,
            statement,
            output: None
        }
    }

    fn execute_once(&mut self) -> Result<(), DatabaseError> {
        let table = self.database.table_mut(&self.statement.table)?;
        let mut replacemments = Vec::new(); 
        for (index, original) in table.rows().iter().enumerate() {
            if matches_filter(self.statement.filter.as_ref(), original)? {
                let mut updated = original.clone(); 
                for assignments in &self.statement.assignments {
                    let row_len = updated.len(); 
                    let target = updated.get_mut(assignments.column_index).ok_or(
                        DatabaseError::InvalidColumnIndex { index:assignments.column_index, row_len }
                    )?;
                    // Every assignment sees the old row, like a SQL update.
                    *target = assignments.value.evaluate(original)?;
                }

                table.schema().validate_row(table.name(), &updated)?;
                replacemments.push((index, updated));
            }
        }

        let result = replacemments
            .iter()
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        // The first pass can still fail validation, so don't modify rows until it is done.
        // Validate every candidate before touching the table.
        for (index, row) in replacemments {
            table.rows_mut()[index] = row; 
        }
        self.output = Some(result.into_iter());

        Ok(())
    }

}

impl Executor for UpdateExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        if self.output.is_none() {
            self.execute_once()?;
        }
        Ok(self.output.as_mut().and_then(|output| output.next()))
    }
}

pub struct DeleteExecutor<'a> {
    database: &'a mut Database,
    statement: BoundDeleteStatement,
    output: Option<std::vec::IntoIter<Row>>,
}

impl<'a> DeleteExecutor<'a> {
    pub fn new(database: &'a mut Database, statement: BoundDeleteStatement) -> Self {
        Self {
            database,
            statement,
            output: None,
        }
    }

    fn execute_once(&mut self) -> Result<(), DatabaseError> {
        let table = self.database.table_mut(&self.statement.table)?;
        let mut kept = Vec::new();
        let mut deleted = Vec::new();
        for row in table.rows() {
            if matches_filter(self.statement.filter.as_ref(), row)? {
                deleted.push(row.clone());
            } else {
                kept.push(row.clone());
            }
        }
        // TODO: Move rows into the kept/deleted sets after evaluating all filters, rather than
        // cloning them. Preserve the current behavior where filter errors leave the table intact.
        // `retain` would be shorter, but splitting the rows keeps DELETE output available.
        // Rebuild so we can return the removed rows too.
        table.replace_rows(kept);
        self.output = Some(deleted.into_iter());
        Ok(())
    }
}

impl Executor for DeleteExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        if self.output.is_none() {
            self.execute_once()?;
        }
        Ok(self.output.as_mut().and_then(|output| output.next()))
    }
}



pub struct CreateTableExecutor<'a> {
    database: &'a mut Database,
    statement: crate::CreateTableStatement,
    finished: bool,
}

impl<'a> CreateTableExecutor<'a> {
    pub fn new(database: &'a mut Database, statement: crate::CreateTableStatement) -> Self {
        Self {
            database,
            statement,
            finished: false,
        }
    }
}

impl Executor for CreateTableExecutor<'_> {
    fn next(&mut self) -> Result<Option<Row>, DatabaseError> {
        if self.finished {
            return Ok(None);
        }
        self.finished = true;
        let columns = self
            .statement
            .columns
            .iter()
            .map(|column| Column::required(column.name.clone(), column.data_type))
            .collect();
        self.database
            .create_table(&self.statement.table, Schema::new(columns)?)?;
        Ok(None)
    }
}



/// Builds the iterator tree represented by a bound statement.
pub fn build_executor<'a>(
    database: &'a mut Database,
    statement: BoundStatement,
) -> Result<Box<dyn Executor + 'a>, DatabaseError> {
    Ok(match statement {
        BoundStatement::CreateTable(statement) => {
            Box::new(CreateTableExecutor::new(database, statement))
        }
        BoundStatement::Insert(statement) => Box::new(InsertExecutor::new(database, statement)),
        BoundStatement::Update(statement) => Box::new(UpdateExecutor::new(database, statement)),
        BoundStatement::Delete(statement) => Box::new(DeleteExecutor::new(database, statement)),
        BoundStatement::Select(BoundSelectStatement {
            table,
            projections,
            filter,
        }) => {
            let scan: Box<dyn Executor> =
                Box::new(TableScanExecutor::from_table(database, &table)?);
            let child = match filter {
                Some(predicate) => {
                    Box::new(FilterExecutor::new(scan, predicate)) as Box<dyn Executor>
                }
                None => scan,
            };
            Box::new(ProjectionExecutor::new(child, projections))
        }
    })
}

/// Drains one bound statement's executor into a result set.
pub fn execute_bound(
    database: &mut Database,
    statement: BoundStatement,
) -> Result<Vec<Row>, DatabaseError> {
    let mut executor = build_executor(database, statement)?;
    let mut rows = Vec::new();
    while let Some(row) = executor.next()? {
        rows.push(row);
    }
    Ok(rows)
}

pub fn execute_sql(database: &mut Database, sql: &str) -> Result<Vec<Row>, DatabaseError> {
    let statement = parse_sql(sql)?;
    let bound = Binder::new(database).bind(statement)?;
    execute_bound(database, bound)
}

pub fn execute_sql_batch(
    database: &mut Database,
    sql: &str,
) -> Result<Vec<Vec<Row>>, DatabaseError> {
    let statements = parse_sql_statements(sql)?;
    let mut results = Vec::with_capacity(statements.len());
    // Bind one at a time since earlier statments may change the catalog.
    for statement in statements {
        let bound = Binder::new(database).bind(statement)?;
        results.push(execute_bound(database, bound)?);
    }
    Ok(results)
}
