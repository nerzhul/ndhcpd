-- Create subnets table
CREATE TABLE IF NOT EXISTS subnets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    network TEXT NOT NULL,
    netmask INTEGER NOT NULL,
    gateway TEXT NOT NULL,
    dns_servers TEXT NOT NULL,
    domain_name TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(network, netmask)
);

-- Create dynamic_ranges table
CREATE TABLE IF NOT EXISTS dynamic_ranges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subnet_id INTEGER NOT NULL,
    range_start TEXT NOT NULL,
    range_end TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (subnet_id) REFERENCES subnets(id) ON DELETE CASCADE
);

-- Create static_ips table
CREATE TABLE IF NOT EXISTS static_ips (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subnet_id INTEGER NOT NULL,
    mac_address TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    hostname TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(mac_address),
    UNIQUE(ip_address),
    FOREIGN KEY (subnet_id) REFERENCES subnets(id) ON DELETE CASCADE
);

-- Create leases table
CREATE TABLE IF NOT EXISTS leases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subnet_id INTEGER NOT NULL,
    mac_address TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    lease_start INTEGER NOT NULL,
    lease_end INTEGER NOT NULL,
    hostname TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (subnet_id) REFERENCES subnets(id) ON DELETE CASCADE
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_dynamic_ranges_subnet ON dynamic_ranges(subnet_id);
CREATE INDEX IF NOT EXISTS idx_static_ips_subnet ON static_ips(subnet_id);
CREATE INDEX IF NOT EXISTS idx_static_ips_mac ON static_ips(mac_address);
CREATE INDEX IF NOT EXISTS idx_leases_subnet ON leases(subnet_id);
CREATE INDEX IF NOT EXISTS idx_leases_mac ON leases(mac_address);
CREATE INDEX IF NOT EXISTS idx_leases_active ON leases(active, lease_end);
