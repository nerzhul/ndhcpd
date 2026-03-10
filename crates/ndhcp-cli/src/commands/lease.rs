use crate::client::ApiClient;
use anyhow::Result;
use chrono::{DateTime, Utc};
use ndhcpd::models::Lease;

pub async fn list(client: ApiClient) -> Result<()> {
    let leases: Vec<Lease> = client.get("/api/leases").await?;

    if leases.is_empty() {
        println!("No active leases");
    } else {
        println!(
            "{:<5} {:<12} {:<20} {:<18} {:<20} {:<20} {:<20}",
            "ID", "Subnet ID", "MAC Address", "IP Address", "Hostname", "Start", "End"
        );
        println!("{}", "-".repeat(120));

        for lease in leases {
            let start = DateTime::<Utc>::from_timestamp(lease.lease_start, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());

            let end = DateTime::<Utc>::from_timestamp(lease.lease_end, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());

            println!(
                "{:<5} {:<12} {:<20} {:<18} {:<20} {:<20} {:<20}",
                lease.id.unwrap_or(0),
                lease.subnet_id,
                lease.mac_address,
                lease.ip_address,
                lease.hostname.as_deref().unwrap_or("-"),
                start,
                end
            );
        }
    }

    Ok(())
}
