# Installation Guide

## System Installation

### 1. Build the Project

```bash
cargo build --release
```

### 2. Create User and Directories

```bash
# Create system user
sudo useradd -r -s /bin/false ndhcpd

# Create directories
sudo mkdir -p /etc/ndhcpd
sudo mkdir -p /var/lib/ndhcpd

# Set ownership
sudo chown ndhcpd:ndhcpd /var/lib/ndhcpd
```

### 3. Install Binaries

```bash
sudo cp target/release/ndhcpd /usr/local/bin/
sudo cp target/release/ndhcp-cli /usr/local/bin/
sudo chmod +x /usr/local/bin/ndhcpd
sudo chmod +x /usr/local/bin/ndhcp-cli

# Grant capability to bind to port 67
sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/ndhcpd
```

### 4. Install Configuration

```bash
sudo cp config.example.yaml /etc/ndhcpd/config.yaml
sudo chown root:root /etc/ndhcpd/config.yaml
sudo chmod 644 /etc/ndhcpd/config.yaml

# Edit configuration as needed
sudo nano /etc/ndhcpd/config.yaml
```

### 5. Install Systemd Service

```bash
sudo cp ndhcpd.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable ndhcpd
sudo systemctl start ndhcpd
```

### 6. Check Status

```bash
sudo systemctl status ndhcpd
sudo journalctl -u ndhcpd -f
```

## Usage

The CLI can be used by any user:

```bash
# List subnets
`ndhcp-cli subnet list

# Create a subnet
`ndhcp-cli subnet create \
  --network 192.168.1.0 \
  --netmask 24 \
  --gateway 192.168.1.1 \
  --dns-servers 8.8.8.8,8.8.4.4

# Add dynamic range
`ndhcp-cli range create \
  --subnet-id 1 \
  --start 192.168.1.100 \
  --end 192.168.1.200
```

## Uninstallation

```bash
# Stop and disable service
sudo systemctl stop ndhcpd
sudo systemctl disable ndhcpd

# Remove files
sudo rm /etc/systemd/system/ndhcpd.service
sudo rm /usr/local/bin/ndhcpd
sudo rm /usr/local/bin/ndhcp-cli
sudo rm -rf /etc/ndhcpd
sudo rm -rf /var/lib/ndhcpd

# Remove user
sudo userdel ndhcpd

# Reload systemd
sudo systemctl daemon-reload
```

## Docker Installation

Alternatively, you can run in Docker:

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libsqlite3-0 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ndhcpd /usr/local/bin/
COPY config.example.yaml /etc/ndhcpd/config.yaml
RUN mkdir -p /var/lib/ndhcpd
EXPOSE 67/udp 8080/tcp
CMD ["/usr/local/bin/ndhcpd"]
```

Build and run:

```bash
docker build -t ndhcpd .
docker run -d \
  --name ndhcpd \
  -p 67:67/udp \
  -p 8080:8080 \
  -v /etc/ndhcpd:/etc/ndhcpd \
  -v /var/lib/ndhcpd:/var/lib/ndhcpd \
  ndhcpd
```
