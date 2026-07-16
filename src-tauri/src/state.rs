use crate::db::Database;
use crate::domain::ScanRunId;
use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Inner cancellation token holding the watch channel and a flag for fast checks.
struct CancellationTokenInner {
    cancel_sender: tokio::sync::watch::Sender<()>,
    cancelled: AtomicBool,
}

/// Token that can signal cancellation to a running scan.
pub struct CancellationToken(Arc<CancellationTokenInner>);

impl CancellationToken {
    pub fn new() -> Self {
        let (cancel_sender, _cancel_receiver) = tokio::sync::watch::channel(());
        Self(Arc::new(CancellationTokenInner {
            cancel_sender,
            cancelled: AtomicBool::new(false),
        }))
    }

    /// Check if the scan has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    /// Signal cancellation.
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
        let _ = self.0.cancel_sender.send(());
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CancellationToken {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

/// Registry of cancellation tokens for running scans.
pub struct CancellationRegistry {
    /// Maps run_id to its cancellation token.
    tokens: Mutex<HashMap<String, CancellationToken>>,
}

impl CancellationRegistry {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new cancellation token for a scan run.
    pub async fn register(&self, run_id: &ScanRunId) {
        let mut tokens = self.tokens.lock().await;
        tokens.insert(run_id.0.clone(), CancellationToken::new());
    }

    /// Get a reference to the cancellation token for a scan run.
    pub async fn get(&self, run_id: &ScanRunId) -> Option<CancellationToken> {
        let tokens = self.tokens.lock().await;
        tokens.get(&run_id.0).cloned()
    }

    /// Cancel a running scan by run_id.
    pub async fn cancel(&self, run_id: &ScanRunId) -> bool {
        let tokens = self.tokens.lock().await;
        if let Some(token) = tokens.get(&run_id.0) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Remove a cancellation token after a scan completes.
    pub async fn remove(&self, run_id: &ScanRunId) {
        let mut tokens = self.tokens.lock().await;
        tokens.remove(&run_id.0);
    }

    /// Check if a token exists for the given run_id.
    pub async fn contains(&self, run_id: &ScanRunId) -> bool {
        let tokens = self.tokens.lock().await;
        tokens.contains_key(&run_id.0)
    }
}

impl Default for CancellationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AppState {
    database: std::sync::Mutex<Database>,
    cancellation_registry: Arc<CancellationRegistry>,
}

impl AppState {
    pub fn new(database: Database) -> Self {
        Self {
            database: std::sync::Mutex::new(database),
            cancellation_registry: Arc::new(CancellationRegistry::new()),
        }
    }

    pub fn with_database<T>(
        &self,
        operation: impl FnOnce(&mut Database) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut database = self.database.lock().map_err(|error| {
            AppError::internal("failed to lock application database", error.to_string())
        })?;

        operation(&mut database)
    }

    /// Get the cancellation registry for managing scan cancellations.
    pub fn cancellation_registry(&self) -> &Arc<CancellationRegistry> {
        &self.cancellation_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registers_and_retrieves_token() {
        let registry = CancellationRegistry::new();
        let run_id = ScanRunId::new("test-run-1").unwrap();

        registry.register(&run_id).await;
        assert!(registry.contains(&run_id).await);

        let token = registry.get(&run_id).await;
        assert!(token.is_some());
        assert!(!token.unwrap().is_cancelled());
    }

    #[tokio::test]
    async fn cancels_running_scan() {
        let registry = CancellationRegistry::new();
        let run_id = ScanRunId::new("test-run-2").unwrap();

        registry.register(&run_id).await;
        let token = registry.get(&run_id).await.unwrap();

        assert!(!token.is_cancelled());
        registry.cancel(&run_id).await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn removes_token_after_completion() {
        let registry = CancellationRegistry::new();
        let run_id = ScanRunId::new("test-run-3").unwrap();

        registry.register(&run_id).await;
        assert!(registry.contains(&run_id).await);

        registry.remove(&run_id).await;
        assert!(!registry.contains(&run_id).await);
        assert!(registry.get(&run_id).await.is_none());
    }

    #[tokio::test]
    async fn cancel_on_nonexistent_run_returns_false() {
        let registry = CancellationRegistry::new();
        let run_id = ScanRunId::new("nonexistent").unwrap();

        let cancelled = registry.cancel(&run_id).await;
        assert!(!cancelled);
    }

    #[tokio::test]
    async fn multiple_runs_have_independent_tokens() {
        let registry = CancellationRegistry::new();
        let run_id_1 = ScanRunId::new("run-1").unwrap();
        let run_id_2 = ScanRunId::new("run-2").unwrap();

        registry.register(&run_id_1).await;
        registry.register(&run_id_2).await;

        let token_1 = registry.get(&run_id_1).await.unwrap();
        let token_2 = registry.get(&run_id_2).await.unwrap();

        // Cancel only run 1
        registry.cancel(&run_id_1).await;

        assert!(token_1.is_cancelled());
        assert!(!token_2.is_cancelled());
    }
}
