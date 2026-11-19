use crate::export;
use crate::record_format::{
    format_key_as_time, parse_record_value, ParsedRecordValue,
};
use crate::storage::{self, KeyEntry, KeyValueEntry, ReadOnlyDatabase, SearchMode, ValueEntry};
use gpui_kit::{AppContext, Context, EventEmitter, SharedString, Task};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrowseMode {
    #[default]
    Table,
    List,
}

#[derive(Clone, Debug, Default)]
pub struct TableRow {
    pub key: Vec<u8>,
    pub key_label: String,
    pub json_fields: std::collections::BTreeMap<String, Arc<str>>,
    pub plain_value: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
pub enum DbEvent {
    Updated,
}

impl EventEmitter<DbEvent> for DbState {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadState {
    Closed,
    Opening,
    Ready,
    Loading,
    Failed,
}

pub struct DbState {
    load_state: LoadState,
    database: Option<Arc<ReadOnlyDatabase>>,
    path: Option<PathBuf>,
    column_families: Vec<String>,
    selected_column_family: Option<String>,
    search_mode: SearchMode,
    search_term: Vec<u8>,
    keys: Vec<KeyEntry>,
    next_key: Option<Vec<u8>>,
    selected_key: Option<Vec<u8>>,
    value: Option<ValueEntry>,
    error: Option<SharedString>,
    generation: u64,
    data_revision: u64,
    open_task: Option<Task<()>>,
    page_task: Option<Task<()>>,
    value_task: Option<Task<()>>,
    browse_mode: BrowseMode,
    table_rows: Vec<TableRow>,
    table_columns: Vec<String>,
    table_include_value_column: bool,
    selected_table_cell: Option<(usize, usize)>,
    table_details_visible: bool,
}

impl DbState {
    pub fn new() -> Self {
        Self {
            load_state: LoadState::Closed,
            database: None,
            path: None,
            column_families: Vec::new(),
            selected_column_family: None,
            search_mode: SearchMode::Prefix,
            search_term: Vec::new(),
            keys: Vec::new(),
            next_key: None,
            selected_key: None,
            value: None,
            error: None,
            generation: 0,
            data_revision: 0,
            open_task: None,
            page_task: None,
            value_task: None,
            browse_mode: BrowseMode::Table,
            table_rows: Vec::new(),
            table_columns: Vec::new(),
            table_include_value_column: false,
            selected_table_cell: None,
            table_details_visible: false,
        }
    }

    pub fn load_state(&self) -> LoadState {
        self.load_state.clone()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn column_families(&self) -> &[String] {
        &self.column_families
    }

    pub fn selected_column_family(&self) -> Option<&str> {
        self.selected_column_family.as_deref()
    }

    pub fn keys(&self) -> &[KeyEntry] {
        &self.keys
    }

    pub fn next_key(&self) -> Option<&[u8]> {
        self.next_key.as_deref()
    }

    pub fn selected_key(&self) -> Option<&[u8]> {
        self.selected_key.as_deref()
    }

    pub fn value(&self) -> Option<&ValueEntry> {
        self.value.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn browse_mode(&self) -> BrowseMode {
        self.browse_mode
    }

    pub fn table_rows(&self) -> &[TableRow] {
        &self.table_rows
    }

    pub fn table_columns(&self) -> &[String] {
        &self.table_columns
    }

    pub fn table_include_value_column(&self) -> bool {
        self.table_include_value_column
    }

    pub fn selected_table_cell(&self) -> Option<(usize, usize)> {
        self.selected_table_cell
    }

    pub fn table_details_visible(&self) -> bool {
        self.table_details_visible
    }

    pub fn table_detail_text(&self) -> Option<String> {
        let (row_ix, col_ix) = self.selected_table_cell?;
        let row = self.table_rows.get(row_ix)?;
        if col_ix == 0 {
            return Some(row.key_label.clone());
        }
        let field_ix = col_ix - 1;
        if let Some(column) = self.table_columns.get(field_ix) {
            return row.json_fields.get(column).map(|value| value.to_string());
        }
        if self.table_include_value_column && field_ix == self.table_columns.len() {
            return row.plain_value.as_deref().map(str::to_owned);
        }
        None
    }

    pub fn select_table_cell(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        cx: &mut Context<Self>,
    ) {
        if self.selected_table_cell == Some((row_ix, col_ix)) {
            return;
        }
        self.selected_table_cell = Some((row_ix, col_ix));
        self.bump_data_revision();
        cx.emit(DbEvent::Updated);
        cx.notify();
    }

    pub fn show_table_cell_details(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        cx: &mut Context<Self>,
    ) {
        self.selected_table_cell = Some((row_ix, col_ix));
        self.table_details_visible = true;
        self.bump_data_revision();
        cx.emit(DbEvent::Updated);
        cx.notify();
    }

    pub fn hide_table_cell_details(&mut self, cx: &mut Context<Self>) {
        if !self.table_details_visible {
            return;
        }
        self.table_details_visible = false;
        self.bump_data_revision();
        cx.emit(DbEvent::Updated);
        cx.notify();
    }

    pub fn data_revision(&self) -> u64 {
        self.data_revision
    }

    fn bump_data_revision(&mut self) {
        self.data_revision = self.data_revision.wrapping_add(1);
    }

    fn initial_column_family(names: &[String]) -> Option<String> {
        names
            .iter()
            .find(|name| name.as_str() != "default")
            .or_else(|| names.first())
            .cloned()
    }

    pub fn set_browse_mode(&mut self, mode: BrowseMode, cx: &mut Context<Self>) {
        if self.browse_mode == mode {
            return;
        }
        self.browse_mode = mode;
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.keys.clear();
        self.table_rows.clear();
        self.table_columns.clear();
        self.table_include_value_column = false;
        self.selected_table_cell = None;
        self.table_details_visible = false;
        if self.database.is_some() {
            self.start_page_load(cx);
        } else {
            self.bump_data_revision();
            cx.emit(DbEvent::Updated);
            cx.notify();
        }
    }

    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.load_state = LoadState::Opening;
        self.error = None;
        self.database = None;
        self.path = None;
        self.column_families.clear();
        self.selected_column_family = None;
        self.search_mode = SearchMode::Prefix;
        self.search_term.clear();
        self.keys.clear();
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.table_rows.clear();
        self.table_columns.clear();
        self.table_include_value_column = false;
        self.selected_table_cell = None;
        self.table_details_visible = false;
        cx.emit(DbEvent::Updated);
        cx.notify();

        self.open_task = Some(cx.spawn(async move |handle, cx| {
            let result = cx
                .background_spawn(async move { storage::open_read_only(&path) })
                .await;
            let _ = handle.update(cx, move |state, cx| {
                if state.generation != generation {
                    return;
                }
                state.open_task = None;
                match result {
                    Ok(database) => {
                        let database = Arc::new(database);
                        state.path = Some(database.path.clone());
                        state.column_families = database.column_families.clone();
                        state.selected_column_family =
                            Self::initial_column_family(&state.column_families);
                        state.database = Some(database);
                        state.load_state = LoadState::Ready;
                        state.start_page_load(cx);
                    }
                    Err(error) => {
                        state.load_state = LoadState::Failed;
                        state.error = Some(error.into());
                    }
                }
                cx.emit(DbEvent::Updated);
                cx.notify();
            });
        }));
    }

    pub fn export_column_family(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(database) = self.database.clone() else {
            return;
        };
        if !self.column_families.iter().any(|item| item == &name) {
            return;
        }
        if self.selected_column_family.as_deref() != Some(name.as_str()) {
            return;
        }

        let name_for_path = name.clone();
        cx.spawn(async move |handle, cx| {
            let save_path = cx
                .background_spawn(async move {
                    rfd::FileDialog::new()
                        .set_title("Export column family to CSV")
                        .set_file_name(format!("{name_for_path}.csv"))
                        .add_filter("CSV", &["csv"])
                        .save_file()
                })
                .await;

            let Some(path) = save_path else {
                return;
            };

            let column_family = name.clone();
            let result = cx
                .background_spawn(async move {
                    export::export_column_family_to_csv(&database, &column_family, &path)
                })
                .await;

            let _ = handle.update(cx, move |state, cx| match result {
                Ok(_) => {
                    state.error = None;
                    cx.notify();
                }
                Err(error) => {
                    state.error = Some(error.into());
                    cx.emit(DbEvent::Updated);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn select_column_family(&mut self, name: String, cx: &mut Context<Self>) {
        if self.selected_column_family.as_deref() == Some(name.as_str()) {
            return;
        }
        if !self.column_families.iter().any(|item| item == &name) {
            return;
        }
        self.selected_column_family = Some(name);
        self.search_term.clear();
        self.search_mode = SearchMode::Prefix;
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.start_page_load(cx);
    }

    pub fn set_search_term(&mut self, term: String, cx: &mut Context<Self>) {
        let term = term.into_bytes();
        if self.search_term == term {
            return;
        }
        self.search_term = term;
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.start_page_load(cx);
    }

    pub fn set_search_mode(&mut self, mode: SearchMode, cx: &mut Context<Self>) {
        if self.search_mode == mode {
            return;
        }
        self.search_mode = mode;
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.start_page_load(cx);
    }

    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.page_task.is_some() || self.next_key.is_none() {
            return;
        }
        self.start_page_load(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.database.is_none() || self.page_task.is_some() {
            return;
        }
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.start_page_load(cx);
    }

    pub fn jump_to_first(&mut self, cx: &mut Context<Self>) {
        if self.database.is_none() || self.page_task.is_some() {
            return;
        }
        self.next_key = None;
        self.selected_key = None;
        self.value = None;
        self.start_page_load(cx);
    }

    pub fn select_key(&mut self, key: Vec<u8>, cx: &mut Context<Self>) {
        if self.selected_key.as_deref() == Some(key.as_slice()) {
            return;
        }
        if self.browse_mode == BrowseMode::Table {
            self.selected_key = Some(key);
            self.bump_data_revision();
            cx.notify();
            return;
        }
        let Some(database) = self.database.clone() else {
            return;
        };
        let Some(column_family) = self.selected_column_family.clone() else {
            return;
        };
        self.selected_key = Some(key.clone());
        self.value = None;
        let expected_key = key.clone();
        self.value_task = Some(cx.spawn(async move |handle, cx| {
            let result = cx
                .background_spawn(
                    async move { storage::read_value(&database, &column_family, &key) },
                )
                .await;
            let _ = handle.update(cx, move |state, cx| {
                state.value_task = None;
                if state.selected_key.as_deref() != Some(expected_key.as_slice()) {
                    return;
                }
                if let Ok(value) = result {
                    state.value = Some(value);
                }
                state.bump_data_revision();
                cx.emit(DbEvent::Updated);
                cx.notify();
            });
        }));
        self.bump_data_revision();
        cx.notify();
    }

    fn start_page_load(&mut self, cx: &mut Context<Self>) {
        let Some(database) = self.database.clone() else {
            return;
        };
        let Some(column_family) = self.selected_column_family.clone() else {
            return;
        };
        let search_mode = self.search_mode;
        let search_term = self.search_term.clone();
        let after = self.next_key.clone();
        let append = after.is_some();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        if !append {
            self.keys.clear();
            self.table_rows.clear();
            self.table_columns.clear();
            self.table_include_value_column = false;
            self.selected_table_cell = None;
            self.next_key = None;
        }
        self.load_state = LoadState::Loading;
        self.error = None;
        let browse_mode = self.browse_mode;
        self.bump_data_revision();
        cx.emit(DbEvent::Updated);
        cx.notify();

        self.page_task = Some(cx.spawn(async move |handle, cx| {
            let result = cx
                .background_spawn(async move {
                    match browse_mode {
                        BrowseMode::List => storage::read_search_page(
                            &database,
                            &column_family,
                            search_mode,
                            &search_term,
                            after.as_deref(),
                        )
                        .map(|(keys, next_key)| PageLoadResult::Keys(keys, next_key)),
                        BrowseMode::Table => storage::read_search_page_with_values(
                            &database,
                            &column_family,
                            search_mode,
                            &search_term,
                            after.as_deref(),
                        )
                        .map(|(rows, next_key)| PageLoadResult::Table(rows, next_key)),
                    }
                })
                .await;
            let _ = handle.update(cx, move |state, cx| {
                if state.generation != generation {
                    return;
                }
                state.page_task = None;
                match result {
                    Ok(PageLoadResult::Keys(keys, next_key)) => {
                        state.keys.extend(keys);
                        state.next_key = next_key;
                        state.load_state = LoadState::Ready;
                    }
                    Ok(PageLoadResult::Table(entries, next_key)) => {
                        state.ingest_table_page(&entries);
                        state.next_key = next_key;
                        state.load_state = LoadState::Ready;
                    }
                    Err(error) => {
                        state.load_state = LoadState::Failed;
                        state.error = Some(error.into());
                    }
                }
                state.bump_data_revision();
                cx.emit(DbEvent::Updated);
                cx.notify();
            });
        }));
    }

    fn ingest_table_page(&mut self, entries: &[KeyValueEntry]) {
        let mut column_names = BTreeSet::from_iter(self.table_columns.iter().cloned());
        for entry in entries {
            let key_label = format_key_as_time(&entry.key);
            let row = match parse_record_value(&entry.value) {
                ParsedRecordValue::JsonObject(json_fields) => TableRow {
                    key: entry.key.clone(),
                    key_label,
                    json_fields: json_fields
                        .into_iter()
                        .map(|(key, value)| (key, Arc::<str>::from(value)))
                        .collect(),
                    plain_value: None,
                },
                ParsedRecordValue::Plain(text) => TableRow {
                    key: entry.key.clone(),
                    key_label,
                    json_fields: Default::default(),
                    plain_value: Some(Arc::<str>::from(text)),
                },
            };
            if row.plain_value.is_some() {
                self.table_include_value_column = true;
            }
            column_names.extend(row.json_fields.keys().cloned());
            self.keys.push(KeyEntry {
                key: row.key.clone(),
            });
            self.table_rows.push(row);
        }
        self.table_columns = column_names.into_iter().collect();
    }
}

enum PageLoadResult {
    Keys(Vec<KeyEntry>, Option<Vec<u8>>),
    Table(Vec<KeyValueEntry>, Option<Vec<u8>>),
}

impl Default for DbState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DbState, LoadState};
    use gpui_kit::{AppContext, TestAppContext};
    use std::path::PathBuf;

    #[gpui_kit::test]
    fn starts_closed(cx: &mut TestAppContext) {
        let state = cx.new(|_| DbState::new());
        state.read_with(cx, |state, _| {
            assert_eq!(state.load_state(), LoadState::Closed);
            assert!(state.path().is_none());
            assert!(state.keys().is_empty());
        });
    }

    #[gpui_kit::test]
    fn opening_invalid_path_enters_opening_state(cx: &mut TestAppContext) {
        let state = cx.new(|_| DbState::new());
        state.update(cx, |state, cx| {
            state.open(PathBuf::from("/path/that/does/not/exist"), cx);
        });
        state.read_with(cx, |state, _| {
            assert_eq!(state.load_state(), LoadState::Opening);
        });
    }
}
