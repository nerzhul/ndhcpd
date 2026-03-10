use crate::client::{AlreadyExistsError, ApiClient};
use crate::RangeCommands;
use anyhow::Result;
use ndhcpd::models::DynamicRange;
use std::net::Ipv4Addr;

pub async fn handle(client: ApiClient, action: RangeCommands) -> Result<()> {
    match action {
        RangeCommands::List { subnet_id } => list(client, subnet_id).await,
        RangeCommands::Create {
            subnet_id,
            start,
            end,
        } => create(client, subnet_id, start, end).await,
        RangeCommands::Delete { id } => delete(client, id).await,
    }
}

async fn list(client: ApiClient, subnet_id: Option<i64>) -> Result<()> {
    let path = if let Some(sid) = subnet_id {
        format!("/api/ranges?subnet_id={}", sid)
    } else {
        "/api/ranges".to_string()
    };

    let ranges: Vec<DynamicRange> = client.get(&path).await?;

    if ranges.is_empty() {
        println!("No dynamic ranges configured");
    } else {
        println!(
            "{:<5} {:<12} {:<18} {:<18} {:<8}",
            "ID", "Subnet ID", "Start", "End", "Enabled"
        );
        println!("{}", "-".repeat(70));

        for range in ranges {
            println!(
                "{:<5} {:<12} {:<18} {:<18} {:<8}",
                range.id.unwrap_or(0),
                range.subnet_id,
                range.range_start,
                range.range_end,
                range.enabled
            );
        }
    }

    Ok(())
}

async fn create(client: ApiClient, subnet_id: i64, start: String, end: String) -> Result<()> {
    let start_ip: Ipv4Addr = start.parse()?;
    let end_ip: Ipv4Addr = end.parse()?;

    let range = DynamicRange {
        id: None,
        subnet_id,
        range_start: start_ip,
        range_end: end_ip,
        enabled: true,
    };

    let id: i64 = client
        .post("/api/ranges", &range)
        .await
        .map_err(|e| match e.downcast::<AlreadyExistsError>() {
            Ok(_) => anyhow::anyhow!("Dynamic range {}-{} already exists", start_ip, end_ip),
            Err(e) => e,
        })?;
    println!("Created dynamic range with ID: {}", id);

    Ok(())
}

async fn delete(client: ApiClient, id: i64) -> Result<()> {
    client.delete(&format!("/api/ranges/{}", id)).await?;
    println!("Deleted range {}", id);
    Ok(())
}
