//! Transition policy trait and configuration

use chrono::{Duration, Utc};
use memos_core::{MemoryEntry, MemoryStatus};

/// Policy configuration for lifecycle transitions
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    /// Days without access before transitioning from Active to Cooling
    pub active_to_cooling_days: i64,
    /// Days in Cooling status before transitioning to Cold
    pub cooling_to_cold_days: i64,
    /// Days without access before transitioning from Cold to Zombie
    pub cold_to_zombie_days: i64,
    /// Minimum access count to keep memory Active
    pub min_access_count: u64,
    /// Days to check for access count
    pub access_count_window_days: i64,
    /// Minimum importance score (0-1) to avoid Zombie status
    pub min_importance_score: f32,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            active_to_cooling_days: 7,
            cooling_to_cold_days: 14,
            cold_to_zombie_days: 30,
            min_access_count: 1,
            access_count_window_days: 7,
            min_importance_score: 0.1,
        }
    }
}

/// Transition policy trait - determines when memories should transition
pub trait TransitionPolicy: Send + Sync {
    /// Determine the next status for a memory based on its current state
    fn get_next_status(&self, memory: &MemoryEntry) -> Option<MemoryStatus>;

    /// Check if a memory should transition from its current status
    fn should_transition(&self, memory: &MemoryEntry) -> bool;

    /// Get the policy configuration
    fn config(&self) -> &PolicyConfig;
}

impl PolicyConfig {
    /// Calculate the threshold datetime for Active -> Cooling transition
    pub fn active_threshold(&self) -> chrono::DateTime<Utc> {
        Utc::now() - Duration::days(self.active_to_cooling_days)
    }

    /// Calculate the threshold datetime for Cold -> Zombie transition
    pub fn cold_threshold(&self) -> chrono::DateTime<Utc> {
        Utc::now() - Duration::days(self.cold_to_zombie_days)
    }

    /// Calculate the threshold datetime for access count window
    pub fn access_count_threshold(&self) -> chrono::DateTime<Utc> {
        Utc::now() - Duration::days(self.access_count_window_days)
    }

    /// Check if memory has been accessed recently (within the window)
    pub fn has_recent_access(&self, memory: &MemoryEntry) -> bool {
        let threshold = self.access_count_threshold();
        memory
            .last_accessed
            .map(|last_accessed| last_accessed > threshold)
            .unwrap_or(false)
    }

    /// Check if memory has enough access count
    pub fn has_sufficient_access(&self, memory: &MemoryEntry) -> bool {
        memory.access_count >= self.min_access_count
    }

    /// Check if memory has been in current status long enough
    pub fn has_been_in_status_long_enough(&self, memory: &MemoryEntry, required_days: i64) -> bool {
        let threshold = Utc::now() - Duration::days(required_days);
        memory.updated_at < threshold
    }

    /// Check if memory has sufficient importance to avoid demotion
    pub fn has_sufficient_importance(&self, memory: &MemoryEntry) -> bool {
        memory.metadata.importance >= self.min_importance_score
    }

    /// Determine the next status for a memory based on its current state
    pub fn get_next_status(&self, memory: &MemoryEntry) -> Option<MemoryStatus> {
        match memory.status {
            MemoryStatus::Active => {
                // Active -> Cooling: no recent access
                if !self.has_recent_access(memory) && !self.has_sufficient_access(memory) {
                    Some(MemoryStatus::Cooling)
                } else {
                    None
                }
            }
            MemoryStatus::Cooling => {
                // Cooling -> Cold: enough time has passed
                if self.has_been_in_status_long_enough(memory, self.cooling_to_cold_days) {
                    Some(MemoryStatus::Cold)
                } else {
                    None
                }
            }
            MemoryStatus::Cold => {
                // Cold -> Zombie: no access for cold_to_zombie_days AND low importance
                if !self.has_recent_access(memory)
                    && self.has_been_in_status_long_enough(memory, self.cold_to_zombie_days)
                    && !self.has_sufficient_importance(memory)
                {
                    Some(MemoryStatus::Zombie)
                } else {
                    None
                }
            }
            MemoryStatus::Zombie => {
                // Zombie memories don't transition further
                None
            }
        }
    }

    /// Check if a memory should transition from its current status
    pub fn should_transition(&self, memory: &MemoryEntry) -> bool {
        self.get_next_status(memory).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn create_test_memory(
        status: MemoryStatus,
        days_ago: i64,
        access_count: u64,
        importance: f32,
    ) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            content: memos_core::MemoryContent::Text("test".to_string()),
            embedding: None,
            metadata: memos_core::MemoryMetadata {
                tags: vec![],
                source: memos_core::MemorySource::System {
                    source_type: "test".to_string(),
                },
                custom_fields: Default::default(),
                importance,
            },
            status,
            created_at: now - Duration::days(30),
            updated_at: now - Duration::days(days_ago),
            access_count,
            last_accessed: Some(now - Duration::days(days_ago)),
        }
    }

    #[test]
    fn test_policy_config_defaults() {
        let config = PolicyConfig::default();
        assert_eq!(config.active_to_cooling_days, 7);
        assert_eq!(config.cooling_to_cold_days, 14);
        assert_eq!(config.cold_to_zombie_days, 30);
        assert_eq!(config.min_access_count, 1);
        assert_eq!(config.access_count_window_days, 7);
    }

    #[test]
    fn test_active_threshold() {
        let config = PolicyConfig::default();
        let threshold = config.active_threshold();
        let expected = Utc::now() - Duration::days(7);
        // Allow 1 second tolerance
        assert!((threshold - expected).num_seconds().abs() < 1);
    }

    #[test]
    fn test_should_not_transition_recently_accessed() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Active, 1, 1, 0.5);

        assert!(!config.should_transition(&memory));
    }

    #[test]
    fn test_should_transition_inactive_memory() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Active, 10, 0, 0.5);

        assert!(config.should_transition(&memory));
    }

    #[test]
    fn test_get_next_status_active_to_cooling() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Active, 10, 0, 0.5);

        let next_status = config.get_next_status(&memory);
        assert_eq!(next_status, Some(MemoryStatus::Cooling));
    }

    #[test]
    fn test_get_next_status_cooling_to_cold() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Cooling, 20, 0, 0.5);

        let next_status = config.get_next_status(&memory);
        assert_eq!(next_status, Some(MemoryStatus::Cold));
    }

    #[test]
    fn test_get_next_status_cold_to_zombie() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Cold, 40, 0, 0.05);

        let next_status = config.get_next_status(&memory);
        assert_eq!(next_status, Some(MemoryStatus::Zombie));
    }

    #[test]
    fn test_important_memory_not_zombie() {
        let config = PolicyConfig::default();
        let memory = create_test_memory(MemoryStatus::Cold, 40, 0, 0.5);

        let next_status = config.get_next_status(&memory);
        // Important memory should stay Cold even with no access
        assert_eq!(next_status, None);
    }
}
