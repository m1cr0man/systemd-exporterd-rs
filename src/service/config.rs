#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub include_filters: Option<Vec<String>>,
    pub exclude_filters: Option<Vec<String>>,
}
