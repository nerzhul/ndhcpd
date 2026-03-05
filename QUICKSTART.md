# DHCP Server - Quick Start Guide

## Overview

This is a complete DHCPv4 server implementation in Rust with:
- REST API for configuration management
- CLI tool for easy management
- SQLite persistent storage
- YAML configuration
- OpenAPI/Swagger documentation

## Quick Start

### 1. Build the Project

```bash
cargo build --release
```

Binaries will be in `target/release/`:
- `dhcp-server` - Main daemon
- `dhcp-cli` - CLI tool

### 2. Create Configuration

Copy the example configuration:

```bash
cp config.example.yaml config.yaml
```

Edit as needed. Minimal config:

```yaml
listen_interfaces:
  - eth0

database_path: ./dhcp.db

api:
  listen_address: 127.0.0.1
  port: 8080

dhcp:
  default_lease_time: 86400
  max_lease_time: 604800
```

### 3. Run the Server

```bash
DHCP_CONFIG=config.yaml ./target/release/dhcp-server
```

The server will:
- Start DHCP service on port 67 (UDP)
- Start REST API on http://127.0.0.1:8080
- Provide Swagger UI at http://127.0.0.1:8080/swagger-ui

### 4. Configure via CLI

Create a subnet:

```bash
./target/release/dhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,8.8.4.4
```

Add a dynamic IP range:

```bash
./target/release/dhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200
```

Add a static IP reservation:

```bash
./target/release/dhcp-cli static create \
  --subnet-id 1 \
  --mac AA:BB:CC:DD:EE:FF \
  --ip 192.168.1.10 \
  --hostname myserver
```

View active leases:

```bash
./target/release/dhcp-cli leases
```

## API Examples

### List Subnets

```bash
curl http://localhost:8080/api/subnets
```

### Create Subnet

```bash
curl -X POST http://localhost:8080/api/subnets \
  -H "Content-Type: application/json" \
  -d '{
    "network": "192.168.1.0",
    "netmask": 24,
    "gateway": "192.168.1.1",
    "dns_servers": ["8.8.8.8", "8.8.4.4"],
    "enabled": true
  }'
```

### Interactive API Documentation

Open http://localhost:8080/swagger-ui in your browser to:
- View all API endpoints
- Test API calls interactively
- See request/response schemas

## Architecture

```
home-router/
├── crates/
│   ├── dhcp-core/      # Core library
│   │   ├── config.rs   # YAML configuration
│   │   ├── models.rs   # Data models
│   │   ├── db.rs       # SQLite database
│   │   └── dhcp/       # DHCP protocol & server
│   ├── dhcp-api/       # REST API
│   │   ├── handlers/   # API endpoints
│   │   └── lib.rs      # Router setup
│   ├── dhcp-server/    # Main daemon
│   └── dhcp-cli/       # CLI tool
│       └── commands/   # CLI commands
└── config.example.yaml
```

## Features

✅ DHCPv4 server with DISCOVER/OFFER/REQUEST/ACK
✅ Subnet management
✅ Dynamic IP ranges
✅ Static IP reservations
✅ Lease tracking
✅ REST API with OpenAPI docs
✅ CLI tool
✅ SQLite persistent storage
✅ YAML configuration
✅ Async/await with Tokio
✅ Proper logging with tracing

## Next Steps

- Complete DHCP dynamic range allocation logic
- Add lease renewal handling
- Implement DHCP relay support
- Add metrics and monitoring
- Create systemd service file
- Add configuration validation
- Implement backup/restore

## Troubleshooting

### DHCP server needs root/CAP_NET_BIND_SERVICE

DHCP uses port 67 which requires privileges:

```bash
# Option 1: Run as root
sudo DHCP_CONFIG=config.yaml ./target/release/dhcp-server

# Option 2: Grant capability (Linux)
sudo setcap 'cap_net_bind_service=+ep' ./target/release/dhcp-server
./target/release/dhcp-server
```

### Check logs

The server uses `tracing` for logging. Set log level:

```bash
RUST_LOG=debug DHCP_CONFIG=config.yaml ./target/release/dhcp-server
```

### Database location

By default, database is at `/var/lib/dhcp-server/dhcp.db`.
Ensure the directory exists and is writable, or use a different path in config.

## License

MIT
