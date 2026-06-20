use crate::adapters::persistence::Persistence;
use crate::application::app_error::AppResult;
use crate::application::task_runtime_state::TaskRuntimeState;
use crate::domain::queue::QueueAggregate;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

enum QueueStateUpdate<T> {
    Persist(T),
    Skip(T),
}

pub(crate) struct QueueStateStore {
    state: Arc<Mutex<QueueAggregate>>,
    runtime_states: Arc<Mutex<HashMap<String, TaskRuntimeState>>>,
    persistence_path: PathBuf,
}

impl QueueStateStore {
    pub(crate) fn new(persistence_path: PathBuf) -> Self {
        let state = Persistence::load(&persistence_path).unwrap_or_default();
        Self {
            state: Arc::new(Mutex::new(state)),
            runtime_states: Arc::new(Mutex::new(HashMap::new())),
            persistence_path,
        }
    }

    pub(crate) async fn read<T>(&self, read: impl FnOnce(&QueueAggregate) -> T) -> T {
        let state = self.state.lock().await;
        read(&state)
    }

    pub(crate) async fn read_runtime_states(&self) -> HashMap<String, TaskRuntimeState> {
        self.runtime_states.lock().await.clone()
    }

    pub(crate) async fn update_runtime_state(
        &self,
        id: &str,
        update: impl FnOnce(&mut TaskRuntimeState),
    ) {
        let mut runtime_states = self.runtime_states.lock().await;
        let runtime = runtime_states
            .entry(id.to_string())
            .or_insert_with(TaskRuntimeState::empty);
        update(runtime);
    }

    pub(crate) async fn reset_runtime_state(&self, id: &str) {
        let mut runtime_states = self.runtime_states.lock().await;
        if let Some(runtime) = runtime_states.get_mut(id) {
            runtime.reset_runtime_fields();
        }
    }

    pub(crate) async fn update_and_persist<T>(
        &self,
        update: impl FnOnce(&mut QueueAggregate) -> AppResult<T>,
    ) -> AppResult<T> {
        self.update(|state| update(state).map(QueueStateUpdate::Persist))
            .await
    }

    pub(crate) async fn update_and_persist_when<T>(
        &self,
        update: impl FnOnce(&mut QueueAggregate) -> AppResult<T>,
        should_persist: impl FnOnce(&T) -> bool,
    ) -> AppResult<T> {
        self.update(|state| {
            let value = update(state)?;
            if should_persist(&value) {
                Ok(QueueStateUpdate::Persist(value))
            } else {
                Ok(QueueStateUpdate::Skip(value))
            }
        })
        .await
    }

    async fn update<T>(
        &self,
        update: impl FnOnce(&mut QueueAggregate) -> AppResult<QueueStateUpdate<T>>,
    ) -> AppResult<T> {
        let mut state = self.state.lock().await;
        let mut next_state = state.clone();
        let update = update(&mut next_state)?;

        match update {
            QueueStateUpdate::Persist(value) => {
                self.commit_state(&mut state, next_state)?;
                Ok(value)
            }
            QueueStateUpdate::Skip(value) => Ok(value),
        }
    }

    pub(crate) async fn mutate_memory<T>(
        &self,
        mutate: impl FnOnce(&mut QueueAggregate) -> T,
    ) -> T {
        let mut state = self.state.lock().await;
        mutate(&mut state)
    }

    fn persist(&self, state: &QueueAggregate) -> AppResult<()> {
        Persistence::save(state, &self.persistence_path)
    }

    fn commit_state(
        &self,
        current_state: &mut QueueAggregate,
        next_state: QueueAggregate,
    ) -> AppResult<()> {
        self.persist(&next_state)?;
        *current_state = next_state;
        Ok(())
    }
}
