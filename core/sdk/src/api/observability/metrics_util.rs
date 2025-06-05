use chrono::Utc;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RpcStats {
    pub start_time: i64,
    pub last_stage: i64,
    stage_durations: String,
}

impl Default for RpcStats {
    fn default() -> Self {
        Self {
            start_time: Utc::now().timestamp_millis(),
            last_stage: 0,
            stage_durations: String::new(),
        }
    }
}

impl RpcStats {
    pub fn update_stage(&mut self, stage: &str) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        let mut last_stage = self.last_stage;
        if last_stage == 0 {
            last_stage = self.start_time;
        }
        self.last_stage = current_time;
        let duration = current_time - last_stage;
        if self.stage_durations.is_empty() {
            self.stage_durations = format!("{}={}", stage, duration);
        } else {
            self.stage_durations = format!("{},{}={}", self.stage_durations, stage, duration);
        }
        duration
    }

    pub fn get_total_time(&self) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        current_time - self.start_time
    }

    pub fn get_stage_durations(&self) -> String {
        self.stage_durations.clone()
    }
}

#[derive(Clone, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct ApiStats {
    pub api: String,
    pub stats_ref: Option<String>,
    pub stats: RpcStats,
}

impl ApiStats {
    pub fn new(api: String) -> Self {
        Self {
            api,
            stats_ref: None,
            stats: RpcStats::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_update_stage() {
        let mut rpc_stats = RpcStats::default();
        let duration = rpc_stats.update_stage("stage1");
        assert!(duration >= 0, "Duration should be non-negative");
        assert_eq!(
            rpc_stats.stage_durations,
            "stage1=".to_string() + &duration.to_string(),
            "Stage durations mismatch"
        );

        let duration2 = rpc_stats.update_stage("stage2");
        assert!(duration2 >= 0, "Duration should be non-negative");
        assert_eq!(
            rpc_stats.stage_durations,
            format!("stage1={},stage2={}", duration, duration2),
            "Stage durations mismatch"
        );
    }

    #[rstest]
    fn test_get_total_time() {
        let rpc_stats = RpcStats::default();
        let total_time = rpc_stats.get_total_time();
        assert!(total_time >= 0, "Total time should be non-negative");
    }

    #[rstest]
    fn test_get_stage_durations() {
        let mut rpc_stats = RpcStats::default();
        rpc_stats.update_stage("stage1");
        let stage_durations = rpc_stats.get_stage_durations();
        assert!(
            !stage_durations.is_empty(),
            "Stage durations should not be empty"
        );
    }

    #[rstest]
    fn test_api_stats_new() {
        let api_stats = ApiStats::new("test_api".to_string());
        assert_eq!(api_stats.api, "test_api".to_string(), "API name mismatch");
        assert!(api_stats.stats_ref.is_none(), "Stats ref should be None");
    }
}
