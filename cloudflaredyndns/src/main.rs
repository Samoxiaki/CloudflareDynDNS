pub mod config;
pub mod cloudflare;

use std::{collections::HashMap, sync::Arc};

use config::Config;
use reqwest::Client;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let config: Config = match Config::from_env() {
		Ok(config) => config,
		Err(e) => {
			println!("Error parsing config: {}", e);
			std::process::exit(1);
		}
	};

	tokio::select! {
		_ = main_loop(&config) => (),
		_ = tokio::signal::ctrl_c() => {
			println!("Received SIGINT, shutting down");
			std::process::exit(0);
		},
		else => println!("Unexpected exit"),
	}

	Ok(())
}

async fn main_loop(config: &Config) {
	let client = reqwest::Client::new();
	let domain_zone_id_cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
	
	loop {
		println!("Updating IP addresses...");
		let (ipv4, ipv6) = match update_ips(&client, config.ipv4_enabled, config.ipv6_enabled).await {
			Ok((i4, i6)) => {
				(i4, i6)
			},
			Err(e) => {
				println!("Error updating IPs: {}", e);
				(None, None)
			}
		};
		
		
		if ipv4.is_some() || ipv6.is_some() {
			println!("Updating domains...");

			let mut futures_list = Vec::new();
			for domain in &config.domains {
				let domain_c = domain.clone();
				let client_c = client.clone();
				let domain_zone_id_cache_c = domain_zone_id_cache.clone();
				let token_c = config.token.clone();
				let proxied_c = config.proxied.clone();
				let (ipv4_c, ipv6_c) = (ipv4.clone(), ipv6.clone()); 
				
				let future = tokio::spawn(
					async move {
						println!("Updating domain '{}'", domain_c);
						match update_domain(&client_c, &token_c, &domain_c, ipv4_c, ipv6_c, proxied_c, &domain_zone_id_cache_c).await {
							Ok(()) => {
								println!("Updated domain '{}'", domain_c);
							},
							Err(e) => {
								println!("Error updating domain '{}': {}", domain_c, e);
							}
						}
					}
				);

				futures_list.push(future);
			}
			let _ = futures::future::join_all(futures_list).await;
			println!("Finished updating domains");

		} else {
			println!("No IP addresses to update");
		}

		println!("Sleeping for {} seconds", config.update_interval);
		tokio::time::sleep(tokio::time::Duration::from_secs(config.update_interval)).await;
	}
	
}

async fn update_ips(client: &Client, ipv4_enabled: bool, ipv6_enabled: bool) -> Result<(Option<String>, Option<String>), Box<dyn std::error::Error>> {
	let ipv4_client = client.clone();
	let ipv6_client = client.clone();

	let ipv4_fut = tokio::spawn(
		async move {
			if ipv4_enabled {
				println!("Getting public IPv4...");
				match cloudflare::get_public_ipv4(&ipv4_client).await {
					Ok(ipv4) => {
						println!("Public IPv4: {}", ipv4);
						Some(ipv4)
					},
					Err(e) => {
						println!("Error getting public IPv4: {}", e);
						None
					}
				}
			} else {
				None
			}
		}
	);

	let ipv6_fut = tokio::spawn(
		async move {
			if ipv6_enabled {
				println!("Getting public IPv6...");
				match cloudflare::get_public_ipv6(&ipv6_client).await {
					Ok(ipv6) => {
						println!("Public IPv6: {}", ipv6);	
						Some(ipv6)
					},
					Err(e) => {
						println!("Error getting public IPv6: {}", e);
						None
					}
				}
			} else {
				None
			}
		}
	);

	match tokio::join!(ipv4_fut, ipv6_fut) {
		(Ok(ipv4), Ok(ipv6)) => Ok((ipv4, ipv6)),
		(Err(e), _) | (_, Err(e)) => Err(e.into()),
	}
	
}

async fn update_domain(client: &Client, token: &str, domain: &str, ipv4: Option<String>, ipv6: Option<String>, proxied: bool, domain_zone_id_cache: &Arc<Mutex<HashMap<String, String>>>) -> Result<(), Box<dyn std::error::Error>> {
	let base_domain = cloudflare::extract_domain_name(domain)?;
	let cached_zone_id = domain_zone_id_cache.lock().await.get(&base_domain).cloned();

	let zone_id = match cached_zone_id {
		Some(zone_id) => zone_id.clone(),
		None => {
			let zone_id = cloudflare::get_zone_id(client, token, &base_domain).await?;
			println!("Cached Zone id for {}: {}", base_domain, zone_id);
			domain_zone_id_cache.lock().await.insert(base_domain.clone(), zone_id.clone());
			zone_id
		}
	};

	let mut futures_list = Vec::new();
	if ipv4.is_some() {
		let ipv4_c = ipv4.unwrap();
		let domain_c = domain.to_owned();
		let zone_id_c = zone_id.clone();
		let client_c = client.clone();
		let token_c = token.to_owned();
		let proxied_c = proxied.clone();

		let future = tokio::spawn(
			async move {
				println!("Updating domain '{}' with IPv4 address '{}'", domain_c, ipv4_c);
				match cloudflare::update_record_ipv4(&client_c, &token_c, &domain_c, &zone_id_c, &ipv4_c, proxied_c).await {
					Ok(result) => {
						match result {
							Some(record) => {
								println!("Record updated for domain '{}': {:#?}", domain_c, record);
							},
							None => {
								println!("Record not found for domain '{}'", domain_c);
							}
							
						}
					},
					Err(e) => {
						println!("Error updating domain '{}' with IPv4 address '{}': {}", domain_c, ipv4_c, e);
					}
				}
			}
		);
		futures_list.push(future);
	}
	
	if ipv6.is_some() {
		let ipv6_c = ipv6.unwrap();
		let domain_c = domain.to_owned();
		let zone_id_c = zone_id.clone();
		let client_c = client.clone();
		let token_c = token.to_owned();
		let proxied_c = proxied.clone();

		let future = tokio::spawn(
			async move {
				println!("Updating domain '{}' with IPv6 address '{}'", domain_c, ipv6_c);
				match cloudflare::update_record_ipv6(&client_c, &token_c, &domain_c, &zone_id_c, &ipv6_c, proxied_c).await {
					Ok(result) => {
						match result {
							Some(record) => {
								println!("Record updated for domain '{}': {:#?}", domain_c, record);
							},
							None => {
								println!("Record not found for domain '{}'", domain_c);
							}
							
						}
					},
					Err(e) => {
						println!("Error updating domain '{}' with IPv6 address '{}': {}", domain_c, ipv6_c, e);
					}
				}
			}
		);
		futures_list.push(future);
	}
	

	futures::future::join_all(futures_list).await;
	Ok(())
}