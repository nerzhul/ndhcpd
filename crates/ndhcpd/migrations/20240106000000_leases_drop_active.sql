-- Drop active column from leases table.
-- Lease validity is now determined solely by lease_end (Unix timestamp).
-- Expiring a lease deletes the row instead of flipping a flag.

DROP INDEX IF EXISTS idx_leases_active;
ALTER TABLE leases DROP COLUMN active;
CREATE INDEX IF NOT EXISTS idx_leases_end ON leases(lease_end);
