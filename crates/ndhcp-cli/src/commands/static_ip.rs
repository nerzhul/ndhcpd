use crate::client::{AlreadyExistsError, ApiClient};
use crate::StaticCommands;
use anyhow::Result;
use ndhcpd::models::StaticIP;
use std::net::Ipv4Addr;

pub async fn handle(client: ApiClient, action: StaticCommands) -> Result<()> {
    match action {
        StaticCommands::List { subnet_id } => list(client, subnet_id).await,
        StaticCommands::Create {
            subnet_id,
            mac,
            ip,
            hostname,
        } => create(client, subnet_id, mac, ip, hostname).await,
        StaticCommands::Delete { id } => delete(client, id).await,
    }
}

async fn list(client: ApiClient, subnet_id: Option<i64>) -> Result<()> {
    let path = if let Some(sid) = subnet_id {
        format!("/api/static-ips?subnet_id={}", sid)
    } else {
        "/api/static-ips".to_string()
    };

    let static_ips: Vec<StaticIP> = client.get(&path).await?;

    if static_ips.is_empty() {
        println!("No static IPs configured");
    } else {
        println!(
            "{:<5} {:<12} {:<20} {:<18} {:<20} {:<8}",
            "ID", "Subnet ID", "MAC Address", "IP Address", "Hostname", "Enabled"
        );
        println!("{}", "-".repeat(90));

        for static_ip in static_ips {
            println!(
                "{:<5} {:<12} {:<20} {:<18} {:<20} {:<8}",
                static_ip.id.unwrap_or(0),
                static_ip.subnet_id,
                static_ip.mac_address,
                static_ip.ip_address,
                static_ip.hostname.as_deref().unwrap_or("-"),
                static_ip.enabled
            );
        }
    }

    Ok(())
}

async fn create(
    client: ApiClient,
    subnet_id: i64,
    mac: String,
    ip: String,
    hostname: Option<String>,
) -> Result<()> {
    let ip_addr: Ipv4Addr = ip.parse()?;

    let static_ip = StaticIP {
        id: None,
        subnet_id,
        mac_address: mac,
        ip_address: ip_addr,
        hostname,
        enabled: true,
    };

    let id: i64 = client
        .post("/api/static-ips", &static_ip)
        .await
        .map_err(|e| match e.downcast::<AlreadyExistsError>() {
            Ok(_) => anyhow::anyhow!(
                "Static IP already exists (MAC {} or IP {} already assigned)",
                static_ip.mac_address,
                static_ip.ip_address
            ),
            Err(e) => e,
        })?;
    println!("Created static IP with ID: {}", id);

    Ok(())
}

async fn delete(client: ApiClient, id: i64) -> Result<()> {
    client.delete(&format!("/api/static-ips/{}", id)).await?;
    println!("Deleted static IP {}", id);
    Ok(())
}
