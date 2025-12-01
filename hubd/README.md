# hubd

The hub daemon powers HTTP access to Krypin state and control APIs. The server exposes a handful of JSON endpoints under `/areas`, `/devices`, `/entities`, `/states/{entity_id}`, and `/command/{entity_id}`.

## Authentication

Requests to every route except `/healthz` require an API credential when authentication is configured. Provide one or more comma-separated tokens through either `KRYPIN_AUTH_TOKENS` (preferred) or `KRYPIN_AUTH_TOKEN`.

- Example: `KRYPIN_AUTH_TOKENS=alpha-token,beta-token`
- Requests may present credentials as a bearer token or API key header:
  - `Authorization: Bearer alpha-token`
  - `x-api-key: alpha-token`

If no tokens are configured, authentication is disabled to simplify local development. When enabled, unauthorized requests are rejected with `401 Unauthorized`. Successful requests propagate a masked token identifier into handler context; state updates will default their `source` to that identifier unless the caller supplies one explicitly.
