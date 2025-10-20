use std::env;

#[derive(Debug)]
pub struct Config {
    pub token: String,
    pub domains: Vec<String>,
	pub ipv4_enabled: bool,
    pub ipv6_enabled: bool,
    pub proxied: bool,
    pub update_interval: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let token = env::var("CF_TOKEN")
            .map_err(|_| "Missing CF_TOKEN")?;

        let domains_raw = env::var("CF_DOMAINS")
            .map_err(|_| "Missing CF_DOMAINS")?;
        let mut domains: Vec<String> = domains_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

		if domains.is_empty() {
			return Err("Missing data in CF_DOMAINS".into());
		}

		domains.sort();
		domains.dedup();

        let ipv4_enabled = env::var("CF_IPV4_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .eq_ignore_ascii_case("true");

		 let ipv6_enabled = env::var("CF_IPV6_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .eq_ignore_ascii_case("true");

        let proxied = env::var("CF_PROXIED")
            .unwrap_or_else(|_| "false".to_string())
            .eq_ignore_ascii_case("true");

        let update_interval = env::var("CF_UPDATE_INTERVAL")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .unwrap_or(300);

        Ok(Self {
            token,
            domains,
			ipv4_enabled,
            ipv6_enabled,
            proxied,
            update_interval,
        })
    }
}
