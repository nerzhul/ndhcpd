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
        StaticCommands::Delete { ip } => delete(client, ip).await,
        StaticCommands::SetHostname { ip, hostname } => set_hostname(client, ip, hostname).await,
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
            "{:<18} {:<20} {:<12} {:<20}",
            "IP Address", "MAC Address", "Subnet ID", "Hostname"
        );
        println!("{}", "-".repeat(74));

        for static_ip in static_ips {
            println!(
                "{:<18} {:<20} {:<12} {:<20}",
                static_ip.ip_address,
                static_ip.mac_address,
                static_ip.subnet_id,
                static_ip.hostname.as_deref().unwrap_or("-"),
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
        subnet_id,
        mac_address: mac,
        ip_address: ip_addr,
        hostname,
    };

    client
        .post::<_, ()>("/api/static-ips", &static_ip)
        .await
        .map_err(|e| match e.downcast::<AlreadyExistsError>() {
            Ok(_) => anyhow::anyhow!(
                "Static IP already exists (MAC {} or IP {} already assigned)",
                static_ip.mac_address,
                static_ip.ip_address
            ),
            Err(e) => e,
        })?;
    println!("Created static IP {}", static_ip.ip_address);

    Ok(())
}

async fn delete(client: ApiClient, ip: String) -> Result<()> {
    client.delete(&format!("/api/static-ips/{}", ip)).await?;
    println!("Deleted static IP {}", ip);
    Ok(())
}

async fn set_hostname(client: ApiClient, ip: String, hostname: Option<String>) -> Result<()> {
    use serde_json::json;
    client
        .patch::<_, ()>(
            &format!("/api/static-ips/{}/hostname", ip),
            &json!({ "hostname": &hostname }),
        )
        .await?;
    match hostname {
        Some(h) => println!("Updated hostname for {} to '{}'", ip, h),
        None => println!("Cleared hostname for {}", ip),
    }
    Ok(())
}

