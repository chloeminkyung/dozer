use super::errors::FromArrowError;
use super::errors::FromArrowError::DateConversionError;
use super::errors::FromArrowError::DateTimeConversionError;
use super::errors::FromArrowError::DurationConversionError;
use super::errors::FromArrowError::FieldTypeNotSupported;
use super::errors::FromArrowError::TimeConversionError;
use super::to_arrow;
use crate::arrow_types::to_arrow::DOZER_SCHEMA_KEY;
use crate::json_types::JsonValue;
use crate::types::Record;
use crate::types::{Field as DozerField, FieldType, Schema as DozerSchema};
use arrow::array;
use arrow::array::{Array, ArrayRef};
use arrow::datatypes::{DataType, TimeUnit};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use arrow::row::SortField;

use crate::arrow_types::errors::FromArrowError::SchemaDeserializationError;
use std::collections::HashMap;
use std::str::FromStr;

macro_rules! make_from {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            let s: DozerField = if r.is_null($row.clone()) {
                DozerField::Null
            } else {
                DozerField::from(r.value($row.clone()))
            };

            Ok(s)
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_binary {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            let s: DozerField = if r.is_null($row.clone()) {
                DozerField::Null
            } else {
                DozerField::Binary(r.value($row.clone()).to_vec())
            };

            Ok(s)
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_timestamp {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            if r.is_null($row.clone()) {
                Ok(DozerField::Null)
            } else {
                r.value_as_datetime($row.clone())
                    .map(DozerField::from)
                    .map_or_else(|| Err(DateTimeConversionError), |v| Ok(DozerField::from(v)))
            }
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_date {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            if r.is_null($row.clone()) {
                Ok(DozerField::Null)
            } else {
                r.value_as_date($row.clone())
                    .map_or_else(|| Err(DateConversionError), |v| Ok(DozerField::from(v)))
            }
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_time {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            if r.is_null($row.clone()) {
                Ok(DozerField::Null)
            } else {
                r.value_as_time($row.clone())
                    .map_or_else(|| Err(TimeConversionError), |v| Ok(DozerField::from(v)))
            }
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_duration {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            if r.is_null($row.clone()) {
                Ok(DozerField::Null)
            } else {
                r.value_as_duration($row.clone()).map_or_else(
                    || Err(DurationConversionError),
                    |v| Ok(DozerField::from(v.num_nanoseconds().unwrap())),
                )
            }
        } else {
            Ok(DozerField::Null)
        }
    }};
}

macro_rules! make_text {
    ($array_type:ty, $column: ident, $row: ident) => {{
        let array = $column.as_any().downcast_ref::<$array_type>();

        if let Some(r) = array {
            let s: DozerField = if r.is_null($row.clone()) {
                DozerField::Null
            } else {
                DozerField::Text(r.value($row.clone()).to_string())
            };

            Ok(s)
        } else {
            Ok(DozerField::Null)
        }
    }};
}

pub fn map_schema_to_dozer(
    schema: &arrow::datatypes::Schema,
) -> Result<DozerSchema, FromArrowError> {
    let schema_val = match schema.metadata.get(DOZER_SCHEMA_KEY) {
        Some(s) => s,
        None => return Err(SchemaDeserializationError(format!("{:?}", schema.metadata))),
    };
    let schema: DozerSchema = serde_json::from_str(schema_val.as_str())
        .map_err(|e| SchemaDeserializationError(e.to_string()))?;

    Ok(schema)
}

pub fn map_value_to_dozer_field(
    column: &ArrayRef,
    row: &usize,
    column_name: &str,
    metadata: &HashMap<String, String>,
) -> Result<DozerField, FromArrowError> {
    match column.data_type() {
        DataType::Null => Ok(DozerField::Null),
        DataType::Boolean => make_from!(array::BooleanArray, column, row),
        DataType::Int8 => make_from!(array::Int8Array, column, row),
        DataType::Int16 => make_from!(array::Int16Array, column, row),
        DataType::Int32 => make_from!(array::Int32Array, column, row),
        DataType::Int64 => make_from!(array::Int64Array, column, row),
        DataType::UInt8 => make_from!(array::UInt8Array, column, row),
        DataType::UInt16 => make_from!(array::UInt16Array, column, row),
        DataType::UInt32 => make_from!(array::UInt32Array, column, row),
        DataType::UInt64 => make_from!(array::UInt64Array, column, row),
        DataType::Float16 => make_from!(array::Float32Array, column, row),
        DataType::Float32 => make_from!(array::Float32Array, column, row),
        DataType::Float64 => make_from!(array::Float64Array, column, row),
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            make_timestamp!(array::TimestampMicrosecondArray, column, row)
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            make_timestamp!(array::TimestampMillisecondArray, column, row)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            make_timestamp!(array::TimestampNanosecondArray, column, row)
        }
        DataType::Timestamp(TimeUnit::Second, _) => {
            make_timestamp!(array::TimestampSecondArray, column, row)
        }
        DataType::Date32 => make_date!(array::Date32Array, column, row),
        DataType::Date64 => make_date!(array::Date64Array, column, row),
        DataType::Time32(TimeUnit::Millisecond) => {
            make_time!(array::Time32MillisecondArray, column, row)
        }
        DataType::Time32(TimeUnit::Second) => make_time!(array::Time32SecondArray, column, row),
        DataType::Time64(TimeUnit::Microsecond) => {
            make_time!(array::Time64MicrosecondArray, column, row)
        }
        DataType::Time64(TimeUnit::Nanosecond) => {
            make_time!(array::Time64NanosecondArray, column, row)
        }
        DataType::Duration(TimeUnit::Microsecond) => {
            make_duration!(array::DurationMicrosecondArray, column, row)
        }
        DataType::Duration(TimeUnit::Millisecond) => {
            make_duration!(array::DurationMillisecondArray, column, row)
        }
        DataType::Duration(TimeUnit::Nanosecond) => {
            make_duration!(array::DurationNanosecondArray, column, row)
        }
        DataType::Duration(TimeUnit::Second) => {
            make_duration!(array::DurationSecondArray, column, row)
        }
        DataType::Binary => make_binary!(array::BinaryArray, column, row),
        DataType::FixedSizeBinary(_) => make_binary!(array::FixedSizeBinaryArray, column, row),
        DataType::LargeBinary => make_binary!(array::LargeBinaryArray, column, row),
        DataType::Utf8 => {
            let schema_val = match metadata.get(DOZER_SCHEMA_KEY) {
                Some(s) => s,
                None => return make_from!(array::StringArray, column, row),
            };
            let schema: DozerSchema = serde_json::from_str(schema_val.as_str())
                .map_err(|e| SchemaDeserializationError(e.to_string()))?;
            for fd in schema.fields.into_iter() {
                if fd.name == *column_name && fd.typ == FieldType::Json {
                    let array = column.as_any().downcast_ref::<array::StringArray>();

                    return if let Some(r) = array {
                        let s: DozerField = if r.is_null(*row) {
                            DozerField::Null
                        } else {
                            match JsonValue::from_str(r.value(*row)) {
                                Ok(j) => DozerField::Json(j),
                                Err(_) => DozerField::from(r.value(*row)),
                            }
                        };
                        Ok(s)
                    } else {
                        Ok(DozerField::Null)
                    };
                }
            }
            make_from!(array::StringArray, column, row)
        }
        DataType::LargeUtf8 => make_text!(array::LargeStringArray, column, row),
        // DataType::Interval(TimeUnit::) => make_from!(array::BooleanArray, x, x0),
        // DataType::List(_) => {}
        // DataType::FixedSizeList(_, _) => {}
        // DataType::LargeList(_) => {}
        // DataType::Struct(_) => {}
        // DataType::Union(_, _, _) => {}
        // DataType::Dictionary(_, _) => {}
        // DataType::Decimal128(_, _) => {}
        // DataType::Decimal256(_, _) => {}
        // DataType::Map(_, _) => {}
        _ => Err(FieldTypeNotSupported(column_name.to_string())),
    }
}

pub fn map_record_batch_to_dozer_records(
    batch: arrow::record_batch::RecordBatch,
    schema: &DozerSchema,
) -> Result<Vec<Record>, FromArrowError> {
    if schema.fields.len() != batch.num_columns() {
        return Err(FromArrowError::SchemaMismatchError(
            schema.fields.len(),
            batch.num_columns(),
        ));
    }
    let mut records = Vec::new();
    let columns = batch.columns();
    let batch_schema = batch.schema();
    let metadata = batch_schema.metadata();
    let mut sort_fields = vec![];
    for x in schema.fields.iter() {
        let dt = to_arrow::map_field_type(x.typ);
        sort_fields.push(SortField::new(dt));
    }
    let num_rows = batch.num_rows();

    for r in 0..num_rows {
        let mut values = vec![];
        for (c, x) in columns.iter().enumerate() {
            let field = schema.fields.get(c).unwrap();
            let value = map_value_to_dozer_field(x, &r, &field.name, metadata)?;
            values.push(value);
        }
        records.push(Record {
            schema_id: schema.identifier,
            values,
            lifetime: None,
        });
    }

    Ok(records)
}

pub fn serialize_record_batch(record: &RecordBatch) -> Vec<u8> {
    let buffer: Vec<u8> = Vec::new();
    let mut stream_writer = StreamWriter::try_new(buffer, &record.schema()).unwrap();
    stream_writer.write(record).unwrap();
    stream_writer.finish().unwrap();
    stream_writer.into_inner().unwrap()
}
