use crate::record_format::{format_key_as_time, parse_record_value, ParsedRecordValue};
use crate::storage::{self, ReadOnlyDatabase};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub fn export_column_family_to_csv(
    database: &ReadOnlyDatabase,
    column_family: &str,
    path: &Path,
) -> Result<usize, String> {
    let mut column_names = BTreeSet::new();
    let mut has_plain_value = false;
    let mut row_count = 0usize;

    storage::for_each_entry(database, column_family, |_key, value| {
        row_count += 1;
        match parse_record_value(value) {
            ParsedRecordValue::JsonObject(fields) => {
                column_names.extend(fields.keys().cloned());
            }
            ParsedRecordValue::Plain(_) => {
                has_plain_value = true;
            }
        }
        Ok(())
    })?;

    let mut headers = vec!["time".to_string()];
    headers.extend(column_names.iter().cloned());
    if has_plain_value {
        headers.push("value".to_string());
    }

    let file =
        File::create(path).map_err(|error| format!("Unable to create {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", headers.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(","))
        .map_err(|error| format!("Unable to write CSV: {error}"))?;

    storage::for_each_entry(database, column_family, |key, value| {
        let time = format_key_as_time(key);
        let mut cells = vec![csv_cell(&time)];
        match parse_record_value(value) {
            ParsedRecordValue::JsonObject(fields) => {
                for column in &column_names {
                    let cell = fields.get(column).cloned().unwrap_or_default();
                    cells.push(csv_cell(&cell));
                }
                if has_plain_value {
                    cells.push(csv_cell(""));
                }
            }
            ParsedRecordValue::Plain(text) => {
                for _ in &column_names {
                    cells.push(csv_cell(""));
                }
                if has_plain_value {
                    cells.push(csv_cell(&text));
                }
            }
        }
        writeln!(writer, "{}", cells.join(","))
            .map_err(|error| format!("Unable to write CSV: {error}"))
    })?;

    Ok(row_count)
}

fn csv_cell(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let needs_quotes = text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r');
    if !needs_quotes {
        return text.to_string();
    }
    format!("\"{}\"", text.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn exports_json_top_level_fields_to_csv() {
        let directory = std::env::temp_dir().join(format!(
            "rocksdb-gui-export-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        let db = rocksdb::DB::open(&options, &directory).expect("create db");
        db.put(
            &1_700_000_000_000i64.to_le_bytes(),
            br#"{"id":1,"name":"alpha","active":true}"#,
        )
        .expect("write key");
        db.put(b"plain", b"hello, world").expect("write plain key");
        drop(db);

        let database = storage::open_read_only(&directory).expect("open read-only");
        let csv_path = directory.join("export.csv");
        let rows = export_column_family_to_csv(&database, "default", &csv_path).expect("export");
        assert_eq!(rows, 2);
        let text = std::fs::read_to_string(&csv_path).expect("read csv");
        assert!(text.starts_with("time,active,id,name,value\n"));
        assert!(text.contains("2023-11-14T22:13:20.000Z,true,1,alpha,"));
        assert!(text.contains(r#"plain,,,,"hello, world""#));
        drop(database);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}