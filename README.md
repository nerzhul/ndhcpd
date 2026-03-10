# ndhcpd / ndhcp-cli

A Rust-based DHCPv4 server daemon with REST API, CLI interface, and IPv6 Router Advertisement prefix management.

## Features

- **DHCPv4 Server**: Full DHCP server implementation (`ndhcpd`)
- **IPv6 RA Prefix Management**: Manage IPv6 prefixes for Router Advertisements (RA) via API
- **REST API**: OpenAPI-compatible API for configuration and management
- **CLI Tool**: Command-line interface for management (`ndhcp-cli`)
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
│   ├── ndhcpd/          # Main daemon binary (DHCP server + REST API)
│   └── ndhcp-cli/       # CLI management tool
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
- Server: `target/release/ndhcpd`
- CLI: `target/release/ndhcp-cli`

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

Create a configuration file (default: `/etc/ndhcpd/config.yaml`):

```yaml
listen_interfaces:
  - eth0

database_path: /var/lib/ndhcpd/dhcp.db

api:
  listen_address: 127.0.0.1
  port: 8080
  unix_socket: /var/run/ndhcpd.sock

dhcp:
  default_lease_time: 86400
  max_lease_time: 604800

# Optional: default lifetimes for IPv6 RA prefixes
ra:
  default_preferred_lifetime: 86400   # 24h
  default_valid_lifetime: 2592000     # 30 days
  default_dns_lifetime: 86400         # 24h
```

## Running the Server

```bash
# Using a custom config file
DHCP_CONFIG=config.yaml ./target/release/ndhcpd

# Using the default config location (/etc/ndhcpd/config.yaml)
./target/release/ndhcpd
```

The daemon starts:
- The DHCP server on configured interfaces (UDP port 67)
- The REST API (default: `http://127.0.0.1:8080`)
- A Unix socket listener at `/var/run/ndhcpd.sock` (no authentication required)
- Swagger UI at `http://127.0.0.1:8080/swagger-ui`

## Using the CLI

### Subnet Management

```bash
# List all subnets
ndhcp-cli subnet list

# Create a subnet
ndhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,8.8.4.4 \
  --domain-name example.local

# Get subnet details
ndhcp-cli subnet get 1

# Delete a subnet
ndhcp-cli subnet delete 1
```

### Dynamic Range Management

```bash
# List all dynamic ranges
ndhcp-cli range list

# List ranges for a specific subnet
ndhcp-cli range list --subnet-id 1

# Create a dynamic range
ndhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200

# Delete a range
ndhcp-cli range delete 1
```

### Static IP Management

```bash
# List all static IPs
ndhcp-cli static list

# List static IPs for a specific subnet
ndhcp-cli static list --subnet-id 1

# Create a static IP assignment
ndhcp-cli static create \
  --subnet-id 1 \
  --mac AA:BB:CC:DD:EE:FF \
  --ip 192.168.1.50 \
  --hostname mydevice

# Delete a static IP
ndhcp-cli static delete 1
```

### Lease Management

```bash
# View active leases
ndhcp-cli leases
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

#### IPv6 IA Prefixes
- `GET /api/ia-prefixes` - List all IPv6 prefixes (optional `?interface=eth0`)
- `POST /api/ia-prefixes` - Create a prefix
- `GET /api/ia-prefixes/:id` - Get prefix details
- `PUT /api/ia-prefixes/:id` - Update a prefix
- `DELETE /api/ia-prefixes/:id` - Delete a prefix

### API Documentation

Interactive API documentation is available via Swagger UI at:
```
http://localhost:8080/swagger-ui
```

## IPv6 / Router Advertisement

`ndhcpd` does not send Router Advertisements itself. Instead, it exposes an API to manage IPv6 prefixes that an external `rtadvd` (or compatible daemon) can consume to build its RA configuration.

Each IA prefix entry holds:

| Field | Description |
|---|---|
| `interface` | Network interface to advertise the prefix on |
| `prefix` | IPv6 network address (e.g. `2001:db8::`) |
| `prefix_len` | Prefix length (e.g. `64`) |
| `preferred_lifetime` | Preferred lifetime in seconds (0 = use default from `ra` config) |
| `valid_lifetime` | Valid lifetime in seconds (0 = use default) |
| `dns_servers` | Comma-separated IPv6 DNS servers for the RDNSS option |
| `dns_lifetime` | DNS lifetime in seconds for the RDNSS option (0 = use default) |
| `enabled` | Whether the prefix is active |

### Example: create an IPv6 prefix

```bash
curl -s --unix-socket /var/run/ndhcpd.sock \
  -X POST http://localhost/api/ia-prefixes \
  -H 'Content-Type: application/json' \
  -d '{
    "interface": "em0",
    "prefix": "2001:db8::",
    "prefix_len": 64,
    "preferred_lifetime": 0,
    "valid_lifetime": 0,
    "dns_servers": "2001:db8::1",
    "dns_lifetime": 0,
    "enabled": true
  }'
```

Default lifetime values (when `0` is passed) are taken from the `ra` section of the configuration file.

## Example Workflow

1. Start the daemon:
```bash
DHCP_CONFIG=config.yaml ./target/release/ndhcpd
```

2. Create a subnet:
```bash
ndhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,1.1.1.1 \
  --domain-name home.local
```

3. Add a dynamic range:
```bash
ndhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200
```

4. Add a static IP for a device:
```bash
ndhcp-cli static create \
  --subnet-id 1 \
  --mac 00:11:22:33:44:55 \
  --ip 192.168.1.10 \
  --hostname server
```

5. Monitor leases:
```bash
ndhcp-cli leases
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
