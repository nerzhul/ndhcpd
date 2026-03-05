use anyhow::Result;
use clap::Parser;
use dhcp_server::{
    config::RaConfig, create_database, create_router_with_auth, dhcp::DhcpServer, Config,
};
use std::sync::Arc;
use tower::ServiceExt;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// DHCP Server - A simple DHCP server with API
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "/etc/dhcp-server/config.yaml")]
    config: String,

    /// Data directory for database and other files
    #[arg(short, long)]
    data_dir: Option<String>,

    /// Unix socket path for API communication
    #[arg(short = 's', long)]
    unix_socket: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dhcp_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting DHCP Server");

    // Load configuration - try specified path, then current directory
    let config_path = if std::path::Path::new(&args.config).exists() {
        args.config.clone()
    } else if args.config == "/etc/dhcp-server/config.yaml" {
        // If default path doesn't exist, try current directory
        let current_dir_config = "config.yaml";
        if std::path::Path::new(current_dir_config).exists() {
            info!(
                "Config not found at {}, using {}",
                args.config, current_dir_config
            );
            current_dir_config.to_string()
        } else {
            args.config.clone()
        }
    } else {
        args.config.clone()
    };

    let mut config = match Config::from_file(&config_path) {
        Ok(cfg) => {
            info!("Loaded configuration from {}", config_path);
            cfg
        }
        Err(e) => {
            error!("Failed to load configuration from {}: {}", config_path, e);
            info!("Using default configuration");
            Config::default()
        }
    };

    // Override database path if data_dir is specified
    if let Some(data_dir) = args.data_dir {
        let db_path = std::path::Path::new(&data_dir).join("dhcp.db");
        config.database_path = db_path.to_string_lossy().to_string();
        info!("Using data directory: {}", data_dir);
    }

    // Override unix socket path if specified
    if let Some(socket_path) = args.unix_socket {
        config.api.unix_socket = Some(socket_path);
    }

    let config = Arc::new(config);

    // Create RaConfig from config or use defaults
    let ra_config: Arc<RaConfig> =
        Arc::new(config.ra.clone().unwrap_or_else(|| RaConfig::default()));

    // Initialize database
    let db_url = format!("sqlite:{}", config.database_path);
    let db = match create_database(&db_url).await {
        Ok(database) => {
            info!("Database initialized at {}", config.database_path);
            database
        }
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(e);
        }
    };

    // Start API server
    let api_addr = format!("{}:{}", config.api.listen_address, config.api.port);
    let unix_socket_path = config.api.unix_socket.clone();

    // Start Unix socket listener if configured
    if let Some(socket_path) = unix_socket_path {
        let api_db_unix = Arc::clone(&db);

        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(&socket_path);

        // Unix socket: no authentication required
        let app = create_router_with_auth(api_db_unix, ra_config.clone(), false);

        let listener = tokio::net::UnixListener::bind(&socket_path).map_err(|e| {
            error!("Failed to bind Unix socket at {}: {}", socket_path, e);
            e
        })?;

        info!(
            "API server listening on Unix socket: {} (no auth)",
            socket_path
        );

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let app = app.clone();
                        tokio::spawn(async move {
                            let stream = hyper_util::rt::TokioIo::new(stream);
                            let hyper_service = hyper::service::service_fn(
                                move |request: hyper::Request<hyper::body::Incoming>| {
                                    let method = request.method().clone();
                                    let uri = request.uri().clone();
                                    let app = app.clone();

                                    async move {
                                        let response = app.oneshot(request).await;
                                        match &response {
                                            Ok(resp) => {
                                                let status = resp.status();
                                                info!(
                                                    "Unix socket request: {} {} -> {}",
                                                    method,
                                                    uri,
                                                    status.as_u16()
                                                );
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Unix socket request error: {} {} -> {}",
                                                    method, uri, e
                                                );
                                            }
                                        }
                                        response
                                    }
                                },
                            );

                            if let Err(err) = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            )
                            .serve_connection(stream, hyper_service)
                            .await
                            {
                                error!("Error serving Unix socket connection: {}", err);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting Unix socket connection: {}", e);
                    }
                }
            }
        });
    }

    // Start TCP API server
    let api_db = Arc::clone(&db);
    let require_auth = config.api.require_authentication.unwrap_or(false);
    let app = create_router_with_auth(api_db, ra_config, require_auth);

    let listener = tokio::net::TcpListener::bind(&api_addr)
        .await
        .map_err(|e| {
            error!("Failed to bind API server to {}: {}", api_addr, e);
            e
        })?;

    if require_auth {
        info!(
            "API server listening on {} (authentication enabled)",
            api_addr
        );
    } else {
        info!(
            "API server listening on {} (authentication disabled)",
            api_addr
        );
    }
    info!("Swagger UI available at http://{}/swagger-ui", api_addr);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("API server error: {}", e);
        }
    });

    // Start DHCP server
    let dhcp_server = DhcpServer::new(Arc::clone(&config), Arc::clone(&db));

    info!(
        "DHCP server starting on interfaces: {:?}",
        config.listen_interfaces
    );

    // Run DHCP server (blocks)
    if let Err(e) = dhcp_server.run().await {
        error!("DHCP server error: {}", e);
        return Err(e);
    }

    Ok(())
}
