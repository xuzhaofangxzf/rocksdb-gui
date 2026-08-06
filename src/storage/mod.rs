use rocksdb::{DB, Direction, IteratorMode, Options};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub const PAGE_SIZE: usize = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Prefix,
    KeyContains,
    ValueContains,
}

#[derive(Clone)]
pub struct ReadOnlyDatabase {
    pub path: PathBuf,
    pub column_families: Vec<String>,
    db: Arc<DB>,
}

#[derive(Clone, Debug)]
pub struct KeyEntry {
    pub key: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ValueEntry {
    pub value: Vec<u8>,
    pub truncated: bool,
}

pub fn open_read_only(path: &Path) -> Result<ReadOnlyDatabase, String> {
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", path.display()));
    }

    let options = Options::default();
    let mut column_families = DB::list_cf(&options, path)
        .map_err(|error| format!("Unable to inspect database: {error}"))?;
    if column_families.is_empty() {
        column_families.push("default".to_string());
    }
    if !column_families.iter().any(|name| name == "default") {
        return Err("Database does not contain the default column family".to_string());
    }

    let db = DB::open_cf_for_read_only(&options, path, &column_families, false)
        .map_err(|error| format!("Unable to open database read-only: {error}"))?;

    Ok(ReadOnlyDatabase {
        path: path.to_path_buf(),
        column_families,
        db: Arc::new(db),
    })
}

pub fn read_search_page(
    database: &ReadOnlyDatabase,
    column_family: &str,
    mode: SearchMode,
    query: &[u8],
    after: Option<&[u8]>,
) -> Result<(Vec<KeyEntry>, Option<Vec<u8>>), String> {
    let handle = database
        .db
        .cf_handle(column_family)
        .ok_or_else(|| format!("Column family not found: {column_family}"))?;
    let iterator_mode = after
        .map(|key| IteratorMode::From(key, Direction::Forward))
        .unwrap_or(IteratorMode::Start);
    let mut iterator = database.db.iterator_cf(handle, iterator_mode);
    let mut keys = Vec::with_capacity(PAGE_SIZE);
    let mut last_key = None;

    while let Some(item) = iterator.next() {
        let (key, value) = item.map_err(|error| format!("Unable to iterate keys: {error}"))?;
        if after.is_some_and(|previous| key.as_ref() == previous) {
            continue;
        }
        let matches = match mode {
            SearchMode::Prefix => {
                if !query.is_empty() && !key.starts_with(query) {
                    if !keys.is_empty() || key.as_ref() > query {
                        break;
                    }
                    false
                } else {
                    true
                }
            }
            SearchMode::KeyContains => {
                query.is_empty() || key.windows(query.len()).any(|part| part == query)
            }
            SearchMode::ValueContains => {
                query.is_empty() || value.windows(query.len()).any(|part| part == query)
            }
        };
        if !matches {
            continue;
        }
        last_key = Some(key.to_vec());
        keys.push(KeyEntry { key: key.to_vec() });
        if keys.len() == PAGE_SIZE {
            break;
        }
    }

    let next = if keys.len() == PAGE_SIZE {
        last_key
    } else {
        None
    };
    Ok((keys, next))
}

pub fn read_value(
    database: &ReadOnlyDatabase,
    column_family: &str,
    key: &[u8],
) -> Result<ValueEntry, String> {
    const MAX_VALUE_BYTES: usize = 4 * 1024 * 1024;
    let handle = database
        .db
        .cf_handle(column_family)
        .ok_or_else(|| format!("Column family not found: {column_family}"))?;
    let value = database
        .db
        .get_cf(handle, key)
        .map_err(|error| format!("Unable to read value: {error}"))?
        .ok_or_else(|| "Key no longer exists".to_string())?;
    let truncated = value.len() > MAX_VALUE_BYTES;
    let value = if truncated {
        value[..MAX_VALUE_BYTES].to_vec()
    } else {
        value
    };
    Ok(ValueEntry { value, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::WriteBatch;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn opens_and_reads_column_families() {
        let directory = std::env::temp_dir().join(format!(
            "rocksdb-gui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let descriptors = vec![
            rocksdb::ColumnFamilyDescriptor::new("default", Options::default()),
            rocksdb::ColumnFamilyDescriptor::new("users", Options::default()),
        ];
        let db = DB::open_cf_descriptors(&options, &directory, descriptors).expect("create db");
        let default_cf = db.cf_handle("default").expect("default cf");
        let users_cf = db.cf_handle("users").expect("users cf");
        let mut batch = WriteBatch::default();
        batch.put_cf(default_cf, b"records/1", b"value");
        batch.put_cf(users_cf, b"user/1", b"user value");
        db.write(batch).expect("write fixture");
        drop(db);

        let database = open_read_only(&directory).expect("open read-only");
        assert_eq!(database.column_families, vec!["default", "users"]);
        let (keys, next) =
            read_search_page(&database, "default", SearchMode::Prefix, b"records/", None)
                .expect("read page");
        assert_eq!(keys[0].key, b"records/1");
        assert!(next.is_none());
        let (keys, _) = read_search_page(
            &database,
            "default",
            SearchMode::KeyContains,
            b"cords",
            None,
        )
        .expect("search keys");
        assert_eq!(keys[0].key, b"records/1");
        let (keys, _) = read_search_page(
            &database,
            "default",
            SearchMode::ValueContains,
            b"value",
            None,
        )
        .expect("search values");
        assert_eq!(keys[0].key, b"records/1");
        drop(database);
        std::fs::remove_dir_all(directory).expect("remove temp database");
    }

    #[test]
    fn truncates_large_values_for_preview() {
        let directory = std::env::temp_dir().join(format!(
            "rocksdb-gui-large-value-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, &directory).expect("create db");
        let large_value = vec![b'x'; 4 * 1024 * 1024 + 1];
        db.put(b"large", &large_value).expect("write large value");
        drop(db);

        let database = open_read_only(&directory).expect("open read-only");
        let value = read_value(&database, "default", b"large").expect("read value");
        assert_eq!(value.value.len(), 4 * 1024 * 1024);
        assert!(value.truncated);
        drop(database);
        std::fs::remove_dir_all(directory).expect("remove temp database");
    }
}
