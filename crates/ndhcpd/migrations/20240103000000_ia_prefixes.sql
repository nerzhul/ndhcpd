-- Create ia_prefixes table for IPv6 prefix delegation via Router Advertisement (SLAAC)
CREATE TABLE IF NOT EXISTS ia_prefixes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    interface TEXT NOT NULL,
    prefix TEXT NOT NULL,
    prefix_len INTEGER NOT NULL,
    preferred_lifetime INTEGER NOT NULL DEFAULT 86400,
    valid_lifetime INTEGER NOT NULL DEFAULT 2592000,
    dns_servers TEXT NOT NULL DEFAULT '',
    dns_lifetime INTEGER NOT NULL DEFAULT 86400,
    enabled INTEGER NOT NULL DEFAULT 1
);

-- Create index for fast interface lookups
CREATE INDEX IF NOT EXISTS idx_ia_prefixes_interface ON ia_prefixes(interface);

-- Create index for fast enabled prefix lookups
CREATE INDEX IF NOT EXISTS idx_ia_prefixes_enabled ON ia_prefixes(enabled);