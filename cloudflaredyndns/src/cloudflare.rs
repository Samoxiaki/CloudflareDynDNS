use std::collections::HashMap;

use reqwest::Client;
use serde_json::Value;

const PROTOCOL: &str = "https";
const CLOUDFLARE_API_HOST: &str = "api.cloudflare.com";

const PUBLIC_IPV4_RESOLVER_HOST: &str = "https://v4.ident.me";
const PUBLIC_IPV6_RESOLVER_HOST: &str = "https://v6.ident.me";

const ZONES_PATH: &str = "/client/v4/zones";

const LIST_RECORDS_PATH: &str = "/client/v4/zones/$zone_id/dns_records"; //zone_id

const CREATE_RECORD_PATH: &str = "/client/v4/zones/$zone_id/dns_records"; //zone_id

const UPDATE_RECORD_PATH: &str = "/client/v4/zones/$zone_id/dns_records/$dns_record_id"; //zone_id, dns_record_id

pub const DNS_RECORD_TYPE_A: &str = "A";
pub const DNS_RECORD_TYPE_AAAA: &str = "AAAA";


#[derive(Debug)]
pub struct DnsRecord {
	pub id: String,
	pub name: String,
	pub record_type: String,
	pub content: String,
	pub proxiable: bool,
	pub proxied: bool,
	pub ttl: u64,
}


fn build_url(protocol: &str, host: &str, path: &str) -> String {
	format!("{}://{}{}", protocol, host, path)
}

pub fn extract_domain_name(domain: &str) -> Result<String, Box<dyn std::error::Error>> {
	let parts: Vec<&str> = domain.split('.').collect();
	if parts.len() < 2 {
		return Err(format!("Invalid domain: {}", domain).into());
	}

	return Ok(
		format!("{}.{}", 
		parts.get(parts.len() - 2).unwrap(), 
		parts.last().unwrap())
	);
	
}

pub async fn get_public_ipv4(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
	let resp = client
        .get(PUBLIC_IPV4_RESOLVER_HOST)
        .send()
        .await?;

	match resp.text().await {
		Ok(ip) => Ok(ip), 
		Err(e) => Err(e.into())
	}
}

pub async fn get_public_ipv6(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
	let resp = client
        .get(PUBLIC_IPV6_RESOLVER_HOST)
        .send()
        .await?;

	match resp.text().await {
		Ok(ip) => Ok(ip), 
		Err(e) => Err(e.into())
	}
}

pub async fn get_zone_id(client: &Client, token: &str, domain: &str) -> Result<String, Box<dyn std::error::Error>> {
	 let url = build_url(PROTOCOL, CLOUDFLARE_API_HOST, ZONES_PATH);
	 let resp_text = client
        .get(&url)
        .bearer_auth(token)
        .query(&[("name", domain), ("status", "active")])
        .send()
    	.await?
		.text()
		.await?;

	let v: Value = serde_json::from_str(&resp_text)?;
    let zone_id = v
        .get("result")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|zone| zone.get("id"))
        .and_then(|id| id.as_str())
        .ok_or(format!("Zone ID not found for {}", domain))?;

    Ok(zone_id.to_string())
}

fn parse_record_data(record: &Value) -> DnsRecord {
	return DnsRecord {
        id: record.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        name: record.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
		record_type: record.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        content: record.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
		proxied: record.get("proxied").and_then(|v| v.as_bool()).unwrap_or(false),
		proxiable: record.get("proxiable").and_then(|v| v.as_bool()).unwrap_or(false),
		ttl: record.get("ttl").and_then(|v| v.as_u64()).unwrap_or(0),
    };
}

fn parse_response_errors(response_json: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let success = response_json.get("success").and_then(|s| s.as_bool()).unwrap_or(false);
    if !success {
		let default_error_list = vec![Value::String("Unknown error".to_string())];
        let errors = response_json
            .get("errors")
            .and_then(|e| e.as_array())
            .unwrap_or(&default_error_list);

        let error_message = errors
            .iter()
            .filter_map(|v| v.get("message"))
			.filter_map(|v| v.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        return Err(error_message.into());
    }

    Ok(())
}

pub async fn record_data(client: &Client, token: &str, record_name: &str, record_type: &str, zone_id: &str) -> Result<Option<DnsRecord>, Box<dyn std::error::Error>> {
	let path = LIST_RECORDS_PATH.replace("$zone_id", zone_id);
	let url = build_url(PROTOCOL, CLOUDFLARE_API_HOST, &path);

    let mut params = HashMap::new();
    params.insert("name", record_name);
    params.insert("type", record_type);

    let resp_text = client
        .get(&url)
        .bearer_auth(token)
        .query(&params)
        .send()
        .await?
        .text()
        .await?;

    let v: Value = serde_json::from_str(&resp_text)?;

    let result_list = v.get("result")
        .and_then(|r| r.as_array())
        .ok_or("Could not find 'result' in response")?;

    if result_list.is_empty() {
        return Ok(None);
    }

    let record = &result_list[0];

    let dns_record = parse_record_data(record);

    Ok(Some(dns_record))
}


#[derive(serde::Serialize, serde::Deserialize)]
pub struct RecordParams{
	name: String,
	#[serde(rename = "type")]
    record_type: String,
    content: String,
	proxied: bool
}
async fn update_record(client: &Client, token: &str, domain: &str, zone_id: &str, ip_addr: &str, proxied: bool, record_type: &str, record_type_id: &str) -> Result<Option<DnsRecord>, Box<dyn std::error::Error>> {
	 let client_request;
	 match record_data(client, token, domain, record_type, zone_id).await? {
		Some(record) => {
			if record.content == ip_addr {
				println!("Record '{}' already has the correct {} address '{}'", domain, record_type_id, ip_addr);
				return Ok(Some(record));

			} else {
				// Update record
				let path = UPDATE_RECORD_PATH.replace("$zone_id", zone_id).replace("$dns_record_id", &record.id);
				let url = build_url(PROTOCOL, CLOUDFLARE_API_HOST, &path);
				client_request = client.patch(&url);

				println!("{}", &url.to_string());
				println!("Updating record '{}' with {} address '{}'", domain, record_type_id, ip_addr);
			}
		},
		None => {
			// Create record
			let path = CREATE_RECORD_PATH.replace("$zone_id", zone_id);
			let url = build_url(PROTOCOL, CLOUDFLARE_API_HOST, &path);
			client_request = client.post(&url);

			println!("Creating record '{}' with {} address '{}'", domain, record_type_id, ip_addr);
		}
	 }
	let params = RecordParams {
		name: domain.to_string(),
		record_type: record_type.to_string(),
		content: ip_addr.to_string(),
		proxied,
	};


	let resp_text = client_request
		.json(&params)
		.bearer_auth(token)
		.send()
		.await?
		.text()
		.await?;
	
	let response_json = serde_json::from_str(&resp_text)?;

	match parse_response_errors(&response_json) {
		Ok(_) => {
			let result_list = response_json.get("result")
				.ok_or("Could not find 'result' in response")?;

			return Ok(Some(parse_record_data(&result_list[0])));
		},
		Err(e) => Err(e)
	}
	
}

pub async fn update_record_ipv4(client: &Client, token: &str, domain: &str, zone_id: &str, ip_addr: &str, proxied: bool) -> Result<Option<DnsRecord>, Box<dyn std::error::Error>> {
	update_record(client, token, domain, zone_id, ip_addr, proxied, DNS_RECORD_TYPE_A, "IPV4").await
}
pub async fn update_record_ipv6(client: &Client, token: &str, domain: &str, zone_id: &str, ip_addr: &str, proxied: bool) -> Result<Option<DnsRecord>, Box<dyn std::error::Error>> {
	update_record(client, token, domain, zone_id, ip_addr, proxied, DNS_RECORD_TYPE_AAAA, "IPV6").await
}