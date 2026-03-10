mod client;
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ndhcp-cli")]
#[command(about = "DHCP Server CLI", long_about = None)]
struct Cli {
    /// API server URL (http://...) or Unix socket path (default: /var/run/ndhcpd.sock)
    #[arg(long, short = 'u')]
    api_url: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage subnets
    Subnet {
        #[command(subcommand)]
        action: SubnetCommands,
    },
    /// Manage dynamic ranges
    Range {
        #[command(subcommand)]
        action: RangeCommands,
    },
    /// Manage static IP assignments
    Static {
        #[command(subcommand)]
        action: StaticCommands,
    },
    /// View leases
    Leases,
    /// Check API health
    Health,
}

#[derive(Subcommand)]
enum SubnetCommands {
    /// List all subnets
    List,
    /// Create a new subnet
    Create {
        /// Network address (e.g., 192.168.1.0)
        #[arg(long)]
        network: String,
        /// Netmask (e.g., 24)
        #[arg(long)]
        netmask: u8,
        /// Gateway address
        #[arg(long)]
        gateway: String,
        /// DNS servers (comma-separated)
        #[arg(long)]
        dns_servers: String,
        /// Domain name (optional)
        #[arg(long)]
        domain_name: Option<String>,
    },
    /// Get subnet details
    Get {
        /// Subnet ID
        id: i64,
    },
    /// Delete a subnet
    Delete {
        /// Subnet ID
        id: i64,
    },
}

#[derive(Subcommand)]
enum RangeCommands {
    /// List all dynamic ranges
    List {
        /// Filter by subnet ID (optional)
        #[arg(long)]
        subnet_id: Option<i64>,
    },
    /// Create a new dynamic range
    Create {
        /// Subnet ID
        #[arg(long)]
        subnet_id: i64,
        /// Range start (e.g., 192.168.1.100)
        #[arg(long)]
        start: String,
        /// Range end (e.g., 192.168.1.200)
        #[arg(long)]
        end: String,
    },
    /// Delete a range
    Delete {
        /// Range ID
        id: i64,
    },
}

#[derive(Subcommand)]
enum StaticCommands {
    /// List all static IP assignments
    List {
        /// Filter by subnet ID (optional)
        #[arg(long)]
        subnet_id: Option<i64>,
    },
    /// Create a new static IP assignment
    Create {
        /// Subnet ID
        #[arg(long)]
        subnet_id: i64,
        /// MAC address (e.g., AA:BB:CC:DD:EE:FF)
        #[arg(long)]
        mac: String,
        /// IP address
        #[arg(long)]
        ip: String,
        /// Hostname (optional)
        #[arg(long)]
        hostname: Option<String>,
    },
    /// Delete a static IP assignment
    Delete {
        /// Static IP ID
        id: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine client type based on api_url
    let client = match &cli.api_url {
        Some(url) if url.starts_with("http://") || url.starts_with("https://") => {
            client::ApiClient::new_http(url)
        }
        Some(path) => client::ApiClient::new_unix(path),
        None => {
            // Default to Unix socket
            client::ApiClient::new_unix("/var/run/ndhcpd.sock")
        }
    };

    match cli.command {
        Commands::Subnet { action } => {
            commands::subnet::handle(client, action).await?;
        }
        Commands::Range { action } => {
            commands::range::handle(client, action).await?;
        }
        Commands::Static { action } => {
            commands::static_ip::handle(client, action).await?;
        }
        Commands::Leases => {
            commands::lease::list(client).await?;
        }
        Commands::Health => {
            let result = client.health().await?;
            println!("{}", result);
        }
    }

    Ok(())
}
