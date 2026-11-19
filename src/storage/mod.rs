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

const MAX_VALUE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct KeyEntry {
    pub key: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct KeyValueEntry {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

fn truncate_value(value: Vec<u8>) -> (Vec<u8>, bool) {
    if value.len() > MAX_VALUE_BYTES {
        (value[..MAX_VALUE_BYTES].to_vec(), true)
    } else {
        (value, false)
    }
}

#[derive(Clone, Debug)]
pub struct ValueEntry {
    pub value: Vec<u8>,
    pub truncated: bool,
}

fn bytes_equal_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn bytes_starts_with_ignore_ascii_case(haystack: &[u8], prefix: &[u8]) -> bool {
    haystack.len() >= prefix.len()
        && bytes_equal_ignore_ascii_case(&haystack[..prefix.len()], prefix)
}

fn bytes_contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| bytes_equal_ignore_ascii_case(window, needle))
}

fn starts_with_ignore_case(haystack: &[u8], needle: &[u8], needle_lower: Option<&str>) -> bool {
    if needle.is_empty() {
        return true;
    }
    match (std::str::from_utf8(haystack), needle_lower) {
        (Ok(haystack), Some(needle_lower)) => haystack.to_lowercase().starts_with(needle_lower),
        _ => bytes_starts_with_ignore_ascii_case(haystack, needle),
    }
}

fn contains_ignore_case(haystack: &[u8], needle: &[u8], needle_lower: Option<&str>) -> bool {
    if needle.is_empty() {
        return true;
    }
    match (std::str::from_utf8(haystack), needle_lower) {
        (Ok(haystack), Some(needle_lower)) => haystack.to_lowercase().contains(needle_lower),
        _ => bytes_contains_ignore_ascii_case(haystack, needle),
    }
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
    let query_lower = std::str::from_utf8(query)
        .ok()
        .filter(|query| !query.is_empty())
        .map(|query| query.to_lowercase());

    while let Some(item) = iterator.next() {
        let (key, value) = item.map_err(|error| format!("Unable to iterate keys: {error}"))?;
        if after.is_some_and(|previous| key.as_ref() == previous) {
            continue;
        }
        let matches = match mode {
            SearchMode::Prefix => {
                starts_with_ignore_case(key.as_ref(), query, query_lower.as_deref())
            }
            SearchMode::KeyContains => {
                contains_ignore_case(key.as_ref(), query, query_lower.as_deref())
            }
            SearchMode::ValueContains => {
                contains_ignore_case(value.as_ref(), query, query_lower.as_deref())
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

pub fn read_search_page_with_values(
    database: &ReadOnlyDatabase,
    column_family: &str,
    mode: SearchMode,
    query: &[u8],
    after: Option<&[u8]>,
) -> Result<(Vec<KeyValueEntry>, Option<Vec<u8>>), String> {
    let handle = database
        .db
        .cf_handle(column_family)
        .ok_or_else(|| format!("Column family not found: {column_family}"))?;
    let iterator_mode = after
        .map(|key| IteratorMode::From(key, Direction::Forward))
        .unwrap_or(IteratorMode::Start);
    let mut iterator = database.db.iterator_cf(handle, iterator_mode);
    let mut rows = Vec::with_capacity(PAGE_SIZE);
    let mut last_key = None;
    let query_lower = std::str::from_utf8(query)
        .ok()
        .filter(|query| !query.is_empty())
        .map(|query| query.to_lowercase());

    while let Some(item) = iterator.next() {
        let (key, value) = item.map_err(|error| format!("Unable to iterate keys: {error}"))?;
        if after.is_some_and(|previous| key.as_ref() == previous) {
            continue;
        }
        let matches = match mode {
            SearchMode::Prefix => {
                starts_with_ignore_case(key.as_ref(), query, query_lower.as_deref())
            }
            SearchMode::KeyContains => {
                contains_ignore_case(key.as_ref(), query, query_lower.as_deref())
            }
            SearchMode::ValueContains => {
                contains_ignore_case(value.as_ref(), query, query_lower.as_deref())
            }
        };
        if !matches {
            continue;
        }
        last_key = Some(key.to_vec());
        rows.push(KeyValueEntry {
            key: key.to_vec(),
            // Keep the complete value until it has been parsed. Truncating the
            // bytes here can turn a valid JSON object into invalid JSON and
            // incorrectly move the whole value into the fallback `value` column.
            value: value.to_vec(),
        });
        if rows.len() == PAGE_SIZE {
            break;
        }
    }

    let next = if rows.len() == PAGE_SIZE {
        last_key
    } else {
        None
    };
    Ok((rows, next))
}

pub fn for_each_entry(
    database: &ReadOnlyDatabase,
    column_family: &str,
    mut callback: impl FnMut(&[u8], &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let handle = database
        .db
        .cf_handle(column_family)
        .ok_or_else(|| format!("Column family not found: {column_family}"))?;
    let mut iterator = database.db.iterator_cf(handle, IteratorMode::Start);
    while let Some(item) = iterator.next() {
        let (key, value) = item.map_err(|error| format!("Unable to iterate keys: {error}"))?;
        callback(key.as_ref(), value.as_ref())?;
    }
    Ok(())
}

pub fn read_value(
    database: &ReadOnlyDatabase,
    column_family: &str,
    key: &[u8],
) -> Result<ValueEntry, String> {
    let handle = database
        .db
        .cf_handle(column_family)
        .ok_or_else(|| format!("Column family not found: {column_family}"))?;
    let value = database
        .db
        .get_cf(handle, key)
        .map_err(|error| format!("Unable to read value: {error}"))?
        .ok_or_else(|| "Key no longer exists".to_string())?;
    let (value, truncated) = truncate_value(value);
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
    fn search_is_case_insensitive() {
        let directory = std::env::temp_dir().join(format!(
            "rocksdb-gui-case-search-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, &directory).expect("create db");
        db.put(b"Records/1", b"Value").expect("write key");
        db.put(b"other", b"JSON_VALUE").expect("write value");
        drop(db);

        let database = open_read_only(&directory).expect("open read-only");

        let (keys, _) = read_search_page(
            &database,
            "default",
            SearchMode::Prefix,
            b"records/",
            None,
        )
        .expect("prefix search");
        assert_eq!(keys[0].key, b"Records/1");

        let (keys, _) = read_search_page(
            &database,
            "default",
            SearchMode::KeyContains,
            b"RECORDS",
            None,
        )
        .expect("key contains search");
        assert_eq!(keys[0].key, b"Records/1");

        let (keys, _) = read_search_page(
            &database,
            "default",
            SearchMode::ValueContains,
            b"json_value",
            None,
        )
        .expect("value contains search");
        assert_eq!(keys[0].key, b"other");

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
