use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestCoverage {
    pub unit: bool,
    pub integration: bool,
    pub e2e: bool,
}
