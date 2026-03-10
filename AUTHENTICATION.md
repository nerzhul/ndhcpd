# API Authentication

## Overview

The DHCP server supports token-based authentication to secure access to the REST API. Tokens are hashed with Argon2 (with salt) before being stored in the database.

## Configuration

Authentication can be enabled/disabled in the `config.yaml` configuration file:

```yaml
api:
  listen_address: 127.0.0.1
  port: 8080
  unix_socket: /var/run/ndhcpd.sock
  require_authentication: true  # Enable authentication
```

**Important**: 
- Authentication only applies to TCP connections
- Unix socket connections are **always exempt** from authentication (for local administration)
- The `/health` endpoint is **always public** (no authentication required)

## Token Management

### Create a new token

Via Unix socket (no authentication):
```bash
curl --unix-socket /var/run/ndhcpd.sock \
  -X POST http://localhost/api/tokens \
  -H "Content-Type: application/json" \
  -d '{"name": "my-token"}'
```

Response:
```json
{
  "id": 1,
  "name": "my-token",
  "token": "aB3dEf7gH9jK2lM4nP6qR8sT0uV1wX3yZ5..."
}
```

**⚠️ Important**: The plaintext token is only displayed once during creation. Keep it in a safe place!

### List tokens

```bash
curl --unix-socket /var/run/ndhcpd.sock \
  http://localhost/api/tokens
```

### Disable/Enable a token

```bash
curl --unix-socket /var/run/ndhcpd.sock \
  -X PATCH http://localhost/api/tokens/1/toggle
```

### Delete a token

```bash
curl --unix-socket /var/run/ndhcpd.sock \
  -X DELETE http://localhost/api/tokens/1
```

## Using the API with authentication

Once authentication is enabled, all TCP requests must include the `Authorization` header:

```bash
curl -H "Authorization: Bearer aB3dEf7gH9jK2lM4nP6qR8sT0uV1wX3yZ5..." \
  http://127.0.0.1:8080/api/subnets
```

### Public endpoints (no authentication required)

- `GET /health` - Health check
- `GET /swagger-ui/*` - Swagger UI documentation
- `GET /api-docs/openapi.json` - OpenAPI specification

## Security

- Tokens are generated with 32 bytes of cryptographically secure random data
- Tokens are hashed with **Argon2** (GPU-resistant hashing algorithm)
- Each token has a **unique salt** stored in the database
- Plaintext tokens are **never stored**
- Unix socket connections are exempt from authentication (local access only)

## Database

Tokens are stored in the `api_tokens` table:

```sql
CREATE TABLE api_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_used_at INTEGER,
    enabled INTEGER NOT NULL DEFAULT 1
);
```

## Complete workflow example

1. Enable authentication in `config.yaml`:
```yaml
api:
  require_authentication: true
```

2. Restart the server:
```bash
systemctl restart ndhcpd
```

3. Create a token (via Unix socket):
```bash
TOKEN=$(curl -s --unix-socket /var/run/ndhcpd.sock \
  -X POST http://localhost/api/tokens \
  -H "Content-Type: application/json" \
  -d '{"name": "admin-token"}' | jq -r '.token')
```

4. Use the token for TCP requests:
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/subnets
```

## Troubleshooting

### 401 Unauthorized error

- Check that the `Authorization` header is present and correct
- Check that the token has not been disabled or deleted
- Expected format: `Authorization: Bearer <token>`

### Access denied even with a valid token

- Check that `require_authentication` is set to `true` in the configuration
- Check that you are not using an old disabled token

### Create a token without Unix socket access

If you don't have access to the Unix socket, you can:
1. Temporarily disable authentication
2. Create a token via the TCP API
3. Re-enable authentication
4. Use the created token
