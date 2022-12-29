use crate::prelude::*;
use csv::Writer;
use std::{collections::HashMap, io::Write};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No columns defined for table")]
    NoColumns,

    #[error("CSV Error")]
    CsvError(#[from] csv::Error),
}

pub struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<(usize, String)>>,
}

impl Table {
    pub fn new() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
        }
    }

    pub fn add_row(&mut self, row: HashMap<String, String>) {
        let mut row_as_vec = vec![];
        for (key, value) in row.into_iter() {
            let column = self.columns.iter().enumerate().find(|c| c.1 == &key);
            let column_idx = match column {
                Some((idx, _)) => idx,
                None => {
                    self.columns.push(key);
                    self.columns.len() - 1
                }
            };
            row_as_vec.push((column_idx, value));
        }
        row_as_vec.sort_by_key(|(idx, _)| *idx);
        self.rows.push(row_as_vec);
    }

    pub fn write(&self, out: &mut impl Write) -> StdResult<(), Error> {
        let mut csv = Writer::from_writer(out);
        csv.write_record(&self.columns)?;

        for columns in &self.rows {
            let mut row: Vec<&str> = Vec::with_capacity(self.columns.len());
            row.resize(self.columns.len(), "");

            for (column_idx, value) in columns {
                row[*column_idx] = value;
            }

            csv.write_record(row)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use literal::{map, MapLiteral};
    use std::io::Cursor;

    fn to_csv(table: Table) -> Result<String> {
        let mut cursor = Cursor::new(Vec::new());
        table.write(&mut cursor)?;
        let vec = cursor.into_inner();
        Ok(String::from_utf8(vec)?)
    }

    #[test]
    fn check_table_add_column() -> Result<()> {
        let mut table = Table::new();
        table.add_row(map! {"foo": "bar"});
        table.add_row(map! {"bar": "baz"});

        let expected_csv = "foo,bar\nbar,\n,baz\n";
        assert_eq!(expected_csv, to_csv(table)?);
        Ok(())
    }
}
