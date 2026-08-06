use crate::storage::{self, KeyEntry, ReadOnlyDatabase, SearchMode, ValueEntry};
use gpui::{AppContext, Context, EventEmitter, SharedString, Task};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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
    open_task: Option<Task<()>>,
    page_task: Option<Task<()>>,
    value_task: Option<Task<()>>,
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
            open_task: None,
            page_task: None,
            value_task: None,
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
                        state.selected_column_family = state.column_families.first().cloned();
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
        self.search_term = term.into_bytes();
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
                cx.emit(DbEvent::Updated);
                cx.notify();
            });
        }));
        cx.emit(DbEvent::Updated);
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
            self.next_key = None;
        }
        self.load_state = LoadState::Loading;
        self.error = None;
        cx.emit(DbEvent::Updated);
        cx.notify();

        self.page_task = Some(cx.spawn(async move |handle, cx| {
            let result = cx
                .background_spawn(async move {
                    storage::read_search_page(
                        &database,
                        &column_family,
                        search_mode,
                        &search_term,
                        after.as_deref(),
                    )
                })
                .await;
            let _ = handle.update(cx, move |state, cx| {
                if state.generation != generation {
                    return;
                }
                state.page_task = None;
                match result {
                    Ok((keys, next_key)) => {
                        state.keys.extend(keys);
                        state.next_key = next_key;
                        state.load_state = LoadState::Ready;
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
}

impl Default for DbState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DbState, LoadState};
    use gpui::{AppContext, TestAppContext};
    use std::path::PathBuf;

    #[gpui::test]
    fn starts_closed(cx: &mut TestAppContext) {
        let state = cx.new(|_| DbState::new());
        state.read_with(cx, |state, _| {
            assert_eq!(state.load_state(), LoadState::Closed);
            assert!(state.path().is_none());
            assert!(state.keys().is_empty());
        });
    }

    #[gpui::test]
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
