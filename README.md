# DHCP Server

A Rust-based DHCPv4 server daemon with REST API and CLI interface.

## Features

- **DHCPv4 Server**: Full DHCP server implementation
- **REST API**: OpenAPI-compatible API for configuration
- **CLI Tool**: Command-line interface for management
- **SQLite Backend**: Persistent storage for configuration and leases
- **YAML Configuration**: Easy-to-edit configuration file
- **Subnet Management**: Configure multiple subnets
- **Dynamic IP Ranges**: Define IP ranges for dynamic allocation
- **Static IP Assignments**: Reserve IPs for specific MAC addresses
- **Lease Tracking**: Monitor active DHCP leases

## Project Structure

```
home-router/
├── crates/
│   ├── dhcp-proto/      # DHCP packet encoding/decoding library
│   ├── dhcp-server/     # Main daemon binary (DHCP server + REST API)
│   └── dhcp-cli/        # CLI management tool
├── scripts/             # Cross-compilation helper scripts
├── .cargo/config.toml   # Cargo build configuration (cross-compilation linkers)
├── config.example.yaml  # Example configuration file
└── README.md
```

## Building

### Native (Linux)

```bash
cargo build --release
# or
make release
```

The binaries will be available at:
- Server: `target/release/dhcp-server`
- CLI: `target/release/dhcp-cli`

### Cross-compilation (FreeBSD)

See [CROSS_COMPILATION.md](CROSS_COMPILATION.md) for the full guide.

Quick start (requires `clang` and `llvm-ar`):

```bash
rustup target add x86_64-unknown-freebsd
make freebsd-sysroot   # download FreeBSD 14 sysroot (once)
make freebsd-release   # build for FreeBSD x86_64
```

Binaries land in `target/x86_64-unknown-freebsd/release/`.

## Configuration

Create a configuration file (default: `/etc/dhcp-server/config.yaml`):

```yaml
listen_interfaces:
  - eth0

database_path: /var/lib/dhcp-server/dhcp.db

api:
  listen_address: 127.0.0.1
  port: 8080

dhcp:
  default_lease_time: 86400
  max_lease_time: 604800
```

## Running the Server

```bash
# Using custom config file
DHCP_CONFIG=config.yaml ./target/release/dhcp-server

# Using default config location
./target/release/dhcp-server
```

The server will:
- Start the DHCP server on configured addresses (default: port 67)
- Start the REST API server (default: http://127.0.0.1:8080)
- Provide Swagger UI at http://127.0.0.1:8080/swagger-ui

## Using the CLI

### Subnet Management

```bash
# List all subnets
dhcp-cli subnet list

# Create a subnet
dhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,8.8.4.4 \
  --domain-name example.local

# Get subnet details
dhcp-cli subnet get 1

# Delete a subnet
dhcp-cli subnet delete 1
```

### Dynamic Range Management

```bash
# List all dynamic ranges
dhcp-cli range list

# List ranges for a specific subnet
dhcp-cli range list --subnet-id 1

# Create a dynamic range
dhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200

# Delete a range
dhcp-cli range delete 1
```

### Static IP Management

```bash
# List all static IPs
dhcp-cli static list

# List static IPs for a specific subnet
dhcp-cli static list --subnet-id 1

# Create a static IP assignment
dhcp-cli static create \
  --subnet-id 1 \
  --mac AA:BB:CC:DD:EE:FF \
  --ip 192.168.1.50 \
  --hostname mydevice

# Delete a static IP
dhcp-cli static delete 1
```

### Lease Management

```bash
# View active leases
dhcp-cli leases
```

## REST API

The REST API is available at `http://localhost:8080/api` by default.

### Endpoints

#### Subnets
- `GET /api/subnets` - List all subnets
- `POST /api/subnets` - Create a subnet
- `GET /api/subnets/:id` - Get subnet details
- `PUT /api/subnets/:id` - Update a subnet
- `DELETE /api/subnets/:id` - Delete a subnet

#### Dynamic Ranges
- `GET /api/ranges` - List all ranges (optional `?subnet_id=X`)
- `POST /api/ranges` - Create a range
- `DELETE /api/ranges/:id` - Delete a range

#### Static IPs
- `GET /api/static-ips` - List all static IPs (optional `?subnet_id=X`)
- `POST /api/static-ips` - Create a static IP
- `DELETE /api/static-ips/:id` - Delete a static IP

#### Leases
- `GET /api/leases` - List active leases

### API Documentation

Interactive API documentation is available via Swagger UI at:
```
http://localhost:8080/swagger-ui
```

## Example Workflow

1. Start the server:
```bash
DHCP_CONFIG=config.yaml ./target/release/dhcp-server
```

2. Create a subnet:
```bash
dhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,1.1.1.1 \
  --domain-name home.local
```

3. Add a dynamic range:
```bash
dhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200
```

4. Add a static IP for a device:
```bash
dhcp-cli static create \
  --subnet-id 1 \
  --mac 00:11:22:33:44:55 \
  --ip 192.168.1.10 \
  --hostname server
```

5. Monitor leases:
```bash
dhcp-cli leases
```

## Dependencies

- **tokio**: Async runtime
- **axum**: Web framework for REST API
- **sqlx**: Database interface with SQLite
- **serde**: Serialization/deserialization
- **clap**: CLI argument parsing
- **utoipa**: OpenAPI documentation
- **tracing**: Logging and diagnostics

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
