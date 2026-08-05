use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig, ComfyHealth, DeviceInfo, SystemStats,
};
use crate::infrastructure::comfy::dto::SystemStatsDto;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

const COMFY_HTTP_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ComfyHttpAdapter {
    client: Client,
    config: ComfyConnectionConfig,
}

impl ComfyHttpAdapter {
    pub fn new(config: ComfyConnectionConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .no_proxy()
            .timeout(COMFY_HTTP_TIMEOUT)
            .build()?;

        Ok(Self { client, config })
    }

    pub fn endpoint(&self) -> String {
        self.config.endpoint()
    }

    async fn get_json<T>(&self, route: &str) -> Result<T, ComfyAdapterError>
    where
        T: DeserializeOwned,
    {
        let endpoint = self.endpoint();
        let url = self.config.route_url(route);
        let response = self.client.get(&url).send().await.map_err(|error| {
            if error.is_timeout() {
                ComfyAdapterError::Timeout(format!("GET {url}: {error}"))
            } else if error.is_connect() {
                ComfyAdapterError::Offline(format!("GET {url}: {error}"))
            } else {
                ComfyAdapterError::Offline(format!("GET {url}: {error}"))
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let message = format!("GET {endpoint}/{route} returned HTTP {status}");

            if matches!(
                status,
                StatusCode::BAD_GATEWAY
                    | StatusCode::GATEWAY_TIMEOUT
                    | StatusCode::SERVICE_UNAVAILABLE
            ) {
                return Err(ComfyAdapterError::Offline(message));
            }

            return Err(ComfyAdapterError::Incompatible(message));
        }

        response.json::<T>().await.map_err(|error| {
            ComfyAdapterError::Protocol(format!(
                "GET {endpoint}/{route} returned invalid JSON: {error}"
            ))
        })
    }

    async fn get_system_stats_internal(&self) -> Result<SystemStats, ComfyAdapterError> {
        let dto: SystemStatsDto = self.get_json("system_stats").await?;

        if dto.system.is_none() && dto.devices.is_empty() {
            return Err(ComfyAdapterError::Incompatible(
                "system_stats response has no system or device data".to_owned(),
            ));
        }

        let system = dto.system.unwrap_or_default();
        let devices = dto
            .devices
            .into_iter()
            .map(|device| DeviceInfo {
                name: device.name,
                device_type: device.device_type,
                vram_total: device.vram_total,
                vram_free: device.vram_free,
            })
            .collect();

        Ok(SystemStats {
            comfyui_version: system.comfyui_version,
            python_version: system.python_version,
            os: system.os,
            ram_total: system.ram_total,
            ram_free: system.ram_free,
            devices,
        })
    }
}

#[async_trait]
impl ComfyAdapter for ComfyHttpAdapter {
    async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
        Ok(ComfyHealth {
            system: self.get_system_stats().await?,
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
        self.get_system_stats_internal().await
    }

    async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
        let object_info: Value = self.get_json("object_info").await?;

        if !object_info.is_object() {
            return Err(ComfyAdapterError::Incompatible(
                "object_info response is not a JSON object".to_owned(),
            ));
        }

        Ok(object_info)
    }
}

#[cfg(test)]
mod tests {
    use super::ComfyHttpAdapter;
    use crate::application::ports::{ComfyAdapter, ComfyAdapterError, ComfyConnectionConfig};
    use serde_json::json;
    use std::net::TcpListener;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn config_for(server: &MockServer) -> ComfyConnectionConfig {
        ComfyConnectionConfig::new("http", "127.0.0.1", server.address().port())
    }

    #[tokio::test]
    async fn parses_system_stats_with_device_vram() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/system_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "system": {
                    "comfyui_version": "test-version",
                    "python_version": "3.12.0",
                    "os": "windows"
                },
                "devices": [{
                    "name": "Test GPU",
                    "type": "cuda",
                    "vram_total": 17179869184u64,
                    "vram_free": 8589934592u64
                }]
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let stats = adapter
            .get_system_stats()
            .await
            .expect("stats should parse");

        assert_eq!(stats.comfyui_version.as_deref(), Some("test-version"));
        assert_eq!(stats.devices[0].name.as_deref(), Some("Test GPU"));
        assert_eq!(stats.devices[0].vram_total, Some(17179869184));
        assert_eq!(stats.devices[0].vram_free, Some(8589934592));
    }

    #[tokio::test]
    async fn html_response_is_incompatible() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/system_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>not comfyui</html>"))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let error = adapter
            .get_system_stats()
            .await
            .expect_err("HTML should not parse as ComfyUI stats");

        assert!(matches!(error, ComfyAdapterError::Protocol(_)));
    }

    #[tokio::test]
    async fn connection_refused_or_timeout_is_offline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("temporary port should bind");
        let port = listener
            .local_addr()
            .expect("local address should resolve")
            .port();
        drop(listener);

        let adapter = ComfyHttpAdapter::new(ComfyConnectionConfig::new("http", "127.0.0.1", port))
            .expect("client should build");
        let error = adapter
            .get_system_stats()
            .await
            .expect_err("closed port should be offline");

        assert!(matches!(
            error,
            ComfyAdapterError::Offline(_) | ComfyAdapterError::Timeout(_)
        ));
    }

    #[tokio::test]
    async fn object_info_is_read_as_backend_value() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/object_info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "KSampler": {},
                "CLIPTextEncode": {},
                "SaveImage": {}
            })))
            .mount(&server)
            .await;

        let adapter = ComfyHttpAdapter::new(config_for(&server)).expect("client should build");
        let object_info = adapter
            .get_object_info()
            .await
            .expect("object info should parse");

        assert_eq!(object_info.as_object().expect("object expected").len(), 3);
    }
}
