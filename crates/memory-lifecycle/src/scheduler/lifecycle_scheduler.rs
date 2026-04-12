//! Lifecycle scheduler for automatic transitions

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::interval;

use memory_core::{BatchResult, LifecycleApi};

use crate::error::LifecycleError;
use crate::policy::TransitionPolicy;

/// Scheduler state
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerState {
    Stopped,
    Running,
    Paused,
}

/// Lifecycle scheduler that runs periodic transition checks
#[allow(dead_code)]
pub struct LifecycleScheduler {
    storage: Arc<dyn LifecycleApi>,
    policy: Arc<dyn TransitionPolicy>,
    state: Arc<RwLock<SchedulerState>>,
    check_interval: Duration,
    batch_size: usize,
}

impl LifecycleScheduler {
    pub fn new(storage: Arc<dyn LifecycleApi>, policy: Arc<dyn TransitionPolicy>) -> Self {
        Self {
            storage,
            policy,
            state: Arc::new(RwLock::new(SchedulerState::Stopped)),
            check_interval: Duration::from_secs(3600), // Default: 1 hour
            batch_size: 100,
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub async fn state(&self) -> SchedulerState {
        self.state.read().await.clone()
    }

    /// Start the scheduler in the background
    pub async fn start(&self) -> Result<(), LifecycleError> {
        let mut state = self.state.write().await;
        if *state == SchedulerState::Running {
            return Err(LifecycleError::SchedulerError(
                "Scheduler already running".to_string(),
            ));
        }
        *state = SchedulerState::Running;
        drop(state);

        let storage = Arc::clone(&self.storage);
        let check_interval = self.check_interval;
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            let mut ticker = interval(check_interval);

            loop {
                ticker.tick().await;

                let current_state = state.read().await.clone();
                if current_state != SchedulerState::Running {
                    break;
                }

                if let Err(e) = Self::run_cycle(&storage).await {
                    tracing::error!("Lifecycle transition cycle failed: {}", e);
                }
            }

            tracing::info!("Lifecycle scheduler stopped");
        });

        tracing::info!(
            "Lifecycle scheduler started with interval {:?}",
            self.check_interval
        );
        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop(&self) -> Result<(), LifecycleError> {
        let mut state = self.state.write().await;
        if *state == SchedulerState::Stopped {
            return Err(LifecycleError::SchedulerError(
                "Scheduler not running".to_string(),
            ));
        }
        *state = SchedulerState::Stopped;
        Ok(())
    }

    /// Pause the scheduler
    pub async fn pause(&self) -> Result<(), LifecycleError> {
        let mut state = self.state.write().await;
        if *state != SchedulerState::Running {
            return Err(LifecycleError::SchedulerError(
                "Scheduler not running".to_string(),
            ));
        }
        *state = SchedulerState::Paused;
        Ok(())
    }

    /// Resume the scheduler
    pub async fn resume(&self) -> Result<(), LifecycleError> {
        let mut state = self.state.write().await;
        if *state != SchedulerState::Paused {
            return Err(LifecycleError::SchedulerError(
                "Scheduler not paused".to_string(),
            ));
        }
        *state = SchedulerState::Running;
        Ok(())
    }

    /// Run a single transition cycle
    async fn run_cycle(storage: &Arc<dyn LifecycleApi>) -> Result<BatchResult, LifecycleError> {
        tracing::debug!("Running lifecycle transition cycle");

        // Run transitions through the storage
        let result = storage
            .run_transitions()
            .await
            .map_err(|e| LifecycleError::TransitionError(e.to_string()))?;

        tracing::info!(
            "Lifecycle transition cycle complete: {} success, {} failures",
            result.success_count,
            result.failure_count
        );

        Ok(result)
    }

    /// Manually trigger a transition cycle
    pub async fn trigger_cycle(&self) -> Result<BatchResult, LifecycleError> {
        if *self.state.read().await == SchedulerState::Stopped {
            return Err(LifecycleError::SchedulerError(
                "Scheduler not running".to_string(),
            ));
        }

        Self::run_cycle(&self.storage).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use memory_core::{MemoryError, MemoryId, MemoryStatus, WorkspaceId};
    use std::sync::Mutex;

    struct MockLifecycleStorage {
        transitions: Mutex<Vec<(MemoryId, MemoryStatus)>>,
        candidates: Mutex<Vec<MemoryId>>,
    }

    impl MockLifecycleStorage {
        fn new() -> Self {
            Self {
                transitions: Mutex::new(Vec::new()),
                candidates: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl LifecycleApi for MockLifecycleStorage {
        async fn transition(
            &self,
            id: MemoryId,
            new_status: MemoryStatus,
        ) -> Result<(), MemoryError> {
            self.transitions.lock().unwrap().push((id, new_status));
            Ok(())
        }

        async fn get_transition_candidates(
            &self,
            _status: MemoryStatus,
        ) -> Result<Vec<MemoryId>, MemoryError> {
            Ok(self.candidates.lock().unwrap().clone())
        }

        async fn run_transitions(&self) -> Result<BatchResult, MemoryError> {
            Ok(BatchResult::new())
        }

        async fn archive(&self, _workspace_id: WorkspaceId) -> Result<BatchResult, MemoryError> {
            Ok(BatchResult::new())
        }
    }

    #[tokio::test]
    async fn test_scheduler_initial_state() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler = LifecycleScheduler::new(storage, policy);

        assert_eq!(scheduler.state().await, SchedulerState::Stopped);
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler =
            LifecycleScheduler::new(storage, policy).with_interval(Duration::from_secs(1));

        scheduler.start().await.unwrap();
        assert_eq!(scheduler.state().await, SchedulerState::Running);

        scheduler.stop().await.unwrap();
        assert_eq!(scheduler.state().await, SchedulerState::Stopped);
    }

    #[tokio::test]
    async fn test_scheduler_pause_resume() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler =
            LifecycleScheduler::new(storage, policy).with_interval(Duration::from_secs(1));

        scheduler.start().await.unwrap();
        scheduler.pause().await.unwrap();
        assert_eq!(scheduler.state().await, SchedulerState::Paused);

        scheduler.resume().await.unwrap();
        assert_eq!(scheduler.state().await, SchedulerState::Running);

        scheduler.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_scheduler_cannot_start_twice() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler =
            LifecycleScheduler::new(storage, policy).with_interval(Duration::from_secs(1));

        scheduler.start().await.unwrap();
        let result = scheduler.start().await;
        assert!(result.is_err());

        scheduler.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_scheduler_cannot_stop_when_not_running() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler = LifecycleScheduler::new(storage, policy);

        let result = scheduler.stop().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_trigger_cycle() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler = LifecycleScheduler::new(storage, policy);

        // Start the scheduler first
        scheduler.start().await.unwrap();

        // Trigger a cycle manually
        let result = scheduler.trigger_cycle().await;
        assert!(result.is_ok());

        // Stop the scheduler
        scheduler.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_trigger_cycle_when_stopped() {
        let storage = Arc::new(MockLifecycleStorage::new());
        let policy = Arc::new(crate::policy::DefaultTransitionPolicy::new());
        let scheduler = LifecycleScheduler::new(storage, policy);

        // Don't start - try to trigger
        let result = scheduler.trigger_cycle().await;
        assert!(result.is_err());
    }
}
