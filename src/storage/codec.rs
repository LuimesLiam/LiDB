use crate::storage::value;

use super::error::DbError;
use super::page::PageId;
use super::row::Row;
use super::schema::{Column, Schema};
use super::table::Table;
use super::value::{DataType, Value};

const NO_PAGE: u64 = u64::MAX;


pub (crate) fn encode_row(row: &Row) -> Result<Vec<u8>, DbError> { 
    let mut out = Vec::new(); 
    put_u16(&mut out, checked_u16(row.len(), "columns in a row")?);

    for value  in row.values() {
        match value {
            Value::Null => out.push(0),
            Value::Integer(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            Value::Boolean(value) => {
                out.push(2);
                out.push(u8::from(*value));
            }
            Value::Text(value) => {
                out.push(3);
                put_u32(&mut out, checked_u32(value.len(), "bytes in text")?);
                out.extend_from_slice(value.as_bytes());
            }
        }
    }
    return Ok(out);
}

pub(crate) fn decode_row(bytes: &[u8]) -> Result<Row, DbError> {
    let mut input = Input::new(bytes);
    let count = input.u16()? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(match input.byte()? {
            0 => Value::Null,
            1 => Value::Integer(input.i64()?),
            2 => match input.byte()? {
                0 => Value::Boolean(false),
                1 => Value::Boolean(true),
                _ => return Err(DbError::Corrupt("invalid Boolean encoding".into())),
            },
            3 => {
                let length = input.u32()? as usize;
                Value::Text(
                    String::from_utf8(input.take(length)?.to_vec())
                        .map_err(|_| DbError::Corrupt("text is not UTF-8".into()))?,
                )
            }
            marker => return Err(DbError::Corrupt(format!("unknown value marker {marker}"))),
        });
    }
    input.finish()?;
    Ok(Row::new(values))
}


pub(crate) fn encode_table(table: &Table) -> Result<Vec<u8>, DbError> {
    let mut out = Vec::new();
    put_string(&mut out, table.name())?;
    put_u16(
        &mut out,
        checked_u16(table.schema().len(), "columns in a schema")?,
    );
    for column in table.schema().columns() {
        put_string(&mut out, column.name())?;
        out.push(match column.data_type() {
            DataType::Integer => 1,
            DataType::Boolean => 2,
            DataType::Text => 3,
        });
        out.push(u8::from(column.is_nullable()));
    }
    out.extend_from_slice(&table.first_page().map_or(NO_PAGE, |id| id.0).to_le_bytes());
    out.extend_from_slice(&(table.row_count() as u64).to_le_bytes());
    Ok(out)
}
pub(crate) fn decode_table(bytes: &[u8]) -> Result<Table, DbError> {
    let mut input = Input::new(bytes);
    let name = input.string()?;
    let count = input.u16()? as usize;
    let mut columns = Vec::with_capacity(count);
    for _ in 0..count {
        let column_name = input.string()?;
        let data_type = match input.byte()? {
            1 => DataType::Integer,
            2 => DataType::Boolean,
            3 => DataType::Text,
            marker => return Err(DbError::Corrupt(format!("unknown type marker {marker}"))),
        };
        let nullable = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(DbError::Corrupt("invalid nullable flag".into())),
        };
        columns.push(Column::new(column_name, data_type, nullable));
    }
    let first = input.u64()?;
    let row_count = input.u64()?;
    input.finish()?;
    let schema = Schema::new(columns)?;
    let mut table = Table::new(name, schema);
    table.set_storage((first != NO_PAGE).then_some(PageId(first)), row_count);
    Ok(table)
}



fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), DbError> {
    put_u16(out, checked_u16(value.len(), "bytes in a name")?);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn checked_u16(value: usize, what: &str) -> Result<u16, DbError> {
    u16::try_from(value).map_err(|_| DbError::Encoding(format!("too many {what}")))
}
fn checked_u32(value: usize, what: &str) -> Result<u32, DbError> {
    u32::try_from(value).map_err(|_| DbError::Encoding(format!("too many {what}")))
}



struct Input<'a> {
    bytes: &'a [u8],
    at: usize
}

impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {bytes, at: 0}
    }

    fn take (&mut self, count: usize) -> Result<&'a [u8], DbError> {
        let end = self.at.checked_add(count).ok_or_else(|| DbError::Corrupt("length overflow".into()))?;
        let value = self.bytes.get(self.at..end).ok_or_else(|| DbError::Corrupt("Truncated record".into()))?;
        self.at = end; 
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, DbError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DbError>{ 
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, DbError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DbError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, DbError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String,DbError> {
        let length = self.u16()? as usize; 
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| DbError::Corrupt("text is not UTF 8".into()))
    }

    fn finish(self )-> Result<(), DbError>{
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(DbError::Corrupt("Record has trailing bytes".into()))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_round_trip_handles_every_value_kind() {
        let row = Row::new(vec![
            Value::Null,
            Value::Integer(-7),
            Value::Boolean(true),
            Value::text("héllo"),
        ]);
        assert_eq!(decode_row(&encode_row(&row).unwrap()).unwrap(), row);
    }
}
