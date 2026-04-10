//! Default transition policy implementation

use memory_core::{MemoryEntry, MemoryStatus};

use super::transition_policy::{PolicyConfig, TransitionPolicy};

/// Default transition policy based on access patterns and importance
#[derive(Debug, Clone)]
pub struct DefaultTransitionPolicy {
    config: PolicyConfig,
}

impl DefaultTransitionPolicy {
    pub fn new() -> Self {
        Self {
            config: PolicyConfig::default(),
        }
    }

    pub fn with_config(config: PolicyConfig) -> Self {
        Self { config }
    }
}

impl Default for DefaultTransitionPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionPolicy for DefaultTransitionPolicy {
    fn get_next_status(&self, memory: &MemoryEntry) -> Option<MemoryStatus> {
        self.config.get_next_status(memory)
    }

    fn should_transition(&self, memory: &MemoryEntry) -> bool {
        self.config.should_transition(memory)
    }

    fn config(&self) -> &PolicyConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn create_test_memory(
        status: MemoryStatus,
        days_since_update: i64,
        access_count: u64,
        importance: f32,
    ) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            content: memory_core::MemoryContent::Text("test".to_string()),
            embedding: None,
            metadata: memory_core::MemoryMetadata {
                tags: vec![],
                source: memory_core::MemorySource::System {
                    source_type: "test".to_string(),
                },
                custom_fields: Default::default(),
                importance,
            },
            status,
            created_at: now - Duration::days(60),
            updated_at: now - Duration::days(days_since_update),
            access_count,
            last_accessed: Some(now - Duration::days(days_since_update)),
        }
    }

    #[test]
    fn test_active_no_recent_access_transitions() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Active, 10, 0, 0.5);

        assert!(policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), Some(MemoryStatus::Cooling));
    }

    #[test]
    fn test_active_with_recent_access_stays() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Active, 1, 1, 0.5);

        assert!(!policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), None);
    }

    #[test]
    fn test_cooling_transitions_to_cold_after_time() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Cooling, 20, 0, 0.5);

        assert!(policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), Some(MemoryStatus::Cold));
    }

    #[test]
    fn test_cooling_stays_if_too_early() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Cooling, 5, 0, 0.5);

        assert!(!policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), None);
    }

    #[test]
    fn test_cold_low_importance_becomes_zombie() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Cold, 40, 0, 0.05);

        assert!(policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), Some(MemoryStatus::Zombie));
    }

    #[test]
    fn test_cold_high_importance_stays_cold() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Cold, 40, 0, 0.5);

        assert!(!policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), None);
    }

    #[test]
    fn test_cold_recently_accessed_stays() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Cold, 5, 1, 0.05);

        assert!(!policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), None);
    }

    #[test]
    fn test_zombie_never_transitions() {
        let policy = DefaultTransitionPolicy::new();
        let memory = create_test_memory(MemoryStatus::Zombie, 100, 0, 0.0);

        assert!(!policy.should_transition(&memory));
        assert_eq!(policy.get_next_status(&memory), None);
    }

    #[test]
    fn test_custom_config() {
        let config = PolicyConfig {
            active_to_cooling_days: 3,
            cooling_to_cold_days: 7,
            cold_to_zombie_days: 14,
            min_access_count: 1,
            access_count_window_days: 3,
            min_importance_score: 0.2,
        };
        let policy = DefaultTransitionPolicy::with_config(config);

        let memory = create_test_memory(MemoryStatus::Active, 5, 0, 0.5);
        assert!(policy.should_transition(&memory));
    }
}
