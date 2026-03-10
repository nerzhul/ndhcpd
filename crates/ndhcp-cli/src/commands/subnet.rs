use crate::client::{AlreadyExistsError, ApiClient};
use crate::SubnetCommands;
use anyhow::Result;
use ndhcpd::models::Subnet;
use std::net::Ipv4Addr;

pub async fn handle(client: ApiClient, action: SubnetCommands) -> Result<()> {
    match action {
        SubnetCommands::List => list(client).await,
        SubnetCommands::Create {
            network,
            netmask,
            gateway,
            dns_servers,
            domain_name,
        } => create(client, network, netmask, gateway, dns_servers, domain_name).await,
        SubnetCommands::Get { id } => get(client, id).await,
        SubnetCommands::Delete { id } => delete(client, id).await,
    }
}

async fn list(client: ApiClient) -> Result<()> {
    let subnets: Vec<Subnet> = client.get("/api/subnets").await?;

    if subnets.is_empty() {
        println!("No subnets configured");
    } else {
        println!(
            "{:<5} {:<18} {:<6} {:<15} {:<30} {:<20} {:<8}",
            "ID", "Network", "Mask", "Gateway", "DNS Servers", "Domain", "Enabled"
        );
        println!("{}", "-".repeat(100));

        for subnet in subnets {
            let dns = subnet
                .dns_servers
                .iter()
                .map(|ip| ip.to_string())
                .collect::<Vec<_>>()
                .join(",");

            println!(
                "{:<5} {:<18} /{:<5} {:<15} {:<30} {:<20} {:<8}",
                subnet.id.unwrap_or(0),
                subnet.network,
                subnet.netmask,
                subnet.gateway,
                dns,
                subnet.domain_name.as_deref().unwrap_or("-"),
                subnet.enabled
            );
        }
    }

    Ok(())
}

async fn create(
    client: ApiClient,
    network: String,
    netmask: u8,
    gateway: String,
    dns_servers: String,
    domain_name: Option<String>,
) -> Result<()> {
    let network_ip: Ipv4Addr = network.parse()?;
    let gateway_ip: Ipv4Addr = gateway.parse()?;

    let dns_ips: Vec<Ipv4Addr> = dns_servers
        .split(',')
        .map(|s| s.trim().parse())
        .collect::<Result<Vec<_>, _>>()?;

    let subnet = Subnet {
        id: None,
        network: network_ip,
        netmask,
        gateway: gateway_ip,
        dns_servers: dns_ips,
        domain_name,
        enabled: true,
    };

    let id: i64 = client
        .post("/api/subnets", &subnet)
        .await
        .map_err(|e| match e.downcast::<AlreadyExistsError>() {
            Ok(_) => anyhow::anyhow!("Subnet {}/{} already exists", network_ip, netmask),
            Err(e) => e,
        })?;
    println!("Created subnet with ID: {}", id);

    Ok(())
}

async fn get(client: ApiClient, id: i64) -> Result<()> {
    let subnet: Subnet = client.get(&format!("/api/subnets/{}", id)).await?;

    println!("Subnet ID: {}", subnet.id.unwrap_or(0));
    println!("Network: {}/{}", subnet.network, subnet.netmask);
    println!("Gateway: {}", subnet.gateway);
    println!(
        "DNS Servers: {}",
        subnet
            .dns_servers
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Some(domain) = &subnet.domain_name {
        println!("Domain: {}", domain);
    }
    println!("Enabled: {}", subnet.enabled);

    Ok(())
}

async fn delete(client: ApiClient, id: i64) -> Result<()> {
    client.delete(&format!("/api/subnets/{}", id)).await?;
    println!("Deleted subnet {}", id);
    Ok(())
}
