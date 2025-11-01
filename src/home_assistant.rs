use serde_derive::{Deserialize, Serialize};
use std::env;

pub struct HomeAssistantAPI {
    endpoint_url: String,
    auth_token: String,
    client: reqwest::Client,
}

impl Default for HomeAssistantAPI {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeAssistantAPI {
    pub fn new() -> Self {
        Self {
            endpoint_url: env::var("HA_URL").unwrap_or_default(),
            auth_token: env::var("HA_TOKEN").unwrap_or_default(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn read_sensor_value(
        &mut self,
        sensor_path: &str,
    ) -> Result<HASensor, anyhow::Error> {
        if self.endpoint_url.is_empty() {
            anyhow::bail!("No HA connection");
        }
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

#[cfg(test)]
mod test_ha_wrapper {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_home_assistant_api() {
        // Set up the mock server
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/api/states/sensor.temperature")
            .match_header("Authorization", "Bearer test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"
                {
                    "entity_id": "sensor.temperature",
                    "state": "22.5",
                    "last_changed": "2023-01-01T12:00:00Z",
                    "last_reported": "2023-01-01T12:00:00Z",
                    "last_updated": "2023-01-01T12:00:00Z"
                }
            "#,
            )
            .create();

        // Set environment variables for the test
        env::set_var("HA_URL", server.url());
        env::set_var("HA_TOKEN", "test_token");

        // Create API instance and perform request

        let mut api = HomeAssistantAPI::new();
        let result = api.read_sensor_value("sensor.temperature").await.unwrap();

        // Verify result
        assert_eq!(result.entity_id, "sensor.temperature");
        assert_eq!(result.state, "22.5");
        assert_eq!(result.last_changed, "2023-01-01T12:00:00Z");
        assert_eq!(result.last_reported, "2023-01-01T12:00:00Z");
        assert_eq!(result.last_updated, "2023-01-01T12:00:00Z");

        // Verify that the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_home_assistant_api_no_connection() {
        // Clear environment variables
        env::remove_var("HA_URL");

        let mut api = HomeAssistantAPI::new();
        let result = api.read_sensor_value("sensor.temperature").await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No HA connection");
    }
}
