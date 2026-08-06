use rocksdb::{ColumnFamilyDescriptor, DB, Options, WriteBatch};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

const DEFAULT_TOTAL_KEYS: usize = 1_000_000;
const BATCH_SIZE: usize = 10_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("test-data/rocksdb-large"));
    let total_keys = env::var("ROCKSDB_TEST_KEYS")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(DEFAULT_TOTAL_KEYS);

    if total_keys == 0 {
        return Err("ROCKSDB_TEST_KEYS must be greater than zero".into());
    }
    if output.exists() {
        return Err(format!(
            "output already exists: {}; remove it manually before regenerating",
            output.display()
        )
        .into());
    }

    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let started = Instant::now();
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options.set_max_background_jobs(4);
    options.set_write_buffer_size(64 * 1024 * 1024);
    options.set_target_file_size_base(64 * 1024 * 1024);

    let column_families = [
        ColumnFamilyDescriptor::new("default", Options::default()),
        ColumnFamilyDescriptor::new("users", Options::default()),
        ColumnFamilyDescriptor::new("binary", Options::default()),
    ];
    let db = DB::open_cf_descriptors(&options, &output, column_families)?;
    let default_cf = db
        .cf_handle("default")
        .ok_or("missing default column family")?;
    let users_cf = db.cf_handle("users").ok_or("missing users column family")?;
    let binary_cf = db
        .cf_handle("binary")
        .ok_or("missing binary column family")?;

    for start in (0..total_keys).step_by(BATCH_SIZE) {
        let end = (start + BATCH_SIZE).min(total_keys);
        let mut batch = WriteBatch::default();

        for index in start..end {
            match index % 10 {
                0..=6 => {
                    let key = format!("records/{:08}", index);
                    let value = format!(
                        "{{\"id\":{},\"name\":\"record-{:08}\",\"active\":{},\"score\":{},\"description\":\"{}\"}}",
                        index,
                        index,
                        index % 3 != 0,
                        (index * 7919) % 100_000,
                        "rocksdb-gui test payload ".repeat(8),
                    );
                    batch.put_cf(default_cf, key.as_bytes(), value.as_bytes());
                }
                7..=8 => {
                    let key = format!("user/{:08}", index);
                    let value = format!(
                        "user_id={index}\nname=user-{index:08}\nemail=user-{index:08}@example.test\nrole={}\n",
                        if index % 5 == 0 { "admin" } else { "viewer" }
                    );
                    batch.put_cf(users_cf, key.as_bytes(), value.as_bytes());
                }
                _ => {
                    let key = format!("blob/{:08}", index);
                    let value = (0..512)
                        .map(|offset| ((index + offset * 31) % 256) as u8)
                        .collect::<Vec<_>>();
                    batch.put_cf(binary_cf, key.as_bytes(), value);
                }
            }
        }

        db.write(batch)?;

        if end % (BATCH_SIZE * 10) == 0 || end == total_keys {
            eprintln!(
                "generated {end}/{total_keys} keys ({:.1}%)",
                end as f64 * 100.0 / total_keys as f64
            );
        }
    }

    db.flush()?;
    let size = directory_size(&output)?;
    println!(
        "generated {} keys in {} ({:.2} MiB) in {:.1}s",
        total_keys,
        output.display(),
        size as f64 / 1024.0 / 1024.0,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn directory_size(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}
