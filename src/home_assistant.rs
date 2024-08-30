use serde_derive::{Deserialize, Serialize};
use std::env;

pub struct HomeAssistantAPI {
    endpoint_url: String,
    auth_token: String,
    client: reqwest::Client,
}

impl HomeAssistantAPI {
    pub fn new() -> Self {
        Self {
            endpoint_url: env::var("HA_URL").expect("Should provide HA url"),
            auth_token: env::var("HA_TOKEN").expect("Should provide HA token"),
            client: reqwest::Client::new(),
        }
    }

    pub async fn read_sensor_value(
        &mut self,
        sensor_path: &str,
    ) -> Result<HASensor, anyhow::Error> {
        let result = self
            .client
            .get(format!("{}/api/states/{}", self.endpoint_url, sensor_path))
            .bearer_auth(&self.auth_token)
            .send()
            .await?
            .json()
            .await?;
        Ok(result)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HASensor {
    #[serde(rename = "entity_id")]
    pub entity_id: String,
    pub state: String,
    #[serde(rename = "last_changed")]
    pub last_changed: String,
    #[serde(rename = "last_reported")]
    pub last_reported: String,
    #[serde(rename = "last_updated")]
    pub last_updated: String,
}
