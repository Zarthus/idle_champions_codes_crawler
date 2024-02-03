use crate::config::ClientConfig;
use licc::{api_key::ApiKey, client::CodesClient};

impl ClientConfig {
    pub fn api_key(&self) -> Option<ApiKey> {
        match self.api_key.is_empty() {
            true => None,
            false => Some(ApiKey::new(self.api_key.clone())),
        }
    }

    pub fn remote_host(&self) -> Option<String> {
        self.remote_host.clone()
    }

    pub fn client(&self) -> CodesClient {
        CodesClient::new_full(self.api_key(), self.remote_host(), None)
    }
}
