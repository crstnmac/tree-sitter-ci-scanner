# Scanner

A fast, tree-sitter based static analysis tool for CI/CD pipelines.

## Features

- **Multi-language support** — JavaScript, TypeScript, Python, HTML, and more
- **Custom rulesets** — Define rules via YAML config
- **Fast parsing** — Incremental tree-sitter parsing with parallel scanning
- **SARIF output** — Native CI/CD integration
- **JSON output** — For programmatic consumption
- **Recursive scanning** — Scan entire directories
- **Exit codes** — CI-friendly (0 for clean, 1 for issues)

## Installation

### From releases (recommended)
```bash
# Linux
curl -sSL https://github.com/crstnmac/tree-sitter-ci-scanner/releases/latest/download/scanner-x86_64-unknown-linux-gnu -o /usr/local/bin/scanner
chmod +x /usr/local/bin/scanner

# macOS
curl -sSL https://github.com/crstnmac/tree-sitter-ci-scanner/releases/latest/download/scanner-x86_64-apple-darwin -o /usr/local/bin/scanner
chmod +x /usr/local/bin/scanner
```

### Build from source
```bash
git clone https://github.com/crstnmac/tree-sitter-ci-scanner.git
cd tree-sitter-ci-scanner
cargo install --path .
```

### Cargo install
```bash
cargo install scanner --git https://github.com/crstnmac/tree-sitter-ci-scanner.git
```

## Usage

### Basic scanning

```bash
# Scan a single file
scanner scan app.js

# Scan a directory (non-recursive)
scanner scan src/

# Scan recursively
scanner scan src/ --recursive

# Specify output format
scanner scan src/ --format json --output results.json

# Specify output format (default: sarif)
scanner scan src/ --format sarif --output results.sarif
```

### Using custom config

```bash
# Use a custom configuration file
scanner scan src/ --config custom-rules.yaml
```

### Filtering rules

```bash
# Only run specific rules
scanner scan src/ --rules js-no-console-log,js-no-eval
```

### Listing available rules

```bash
# List all rules
scanner rules

# List rules for a specific language
scanner rules --language javascript
```

### Version

```bash
scanner version
```

## Configuration

Create a `.scanner.yaml` file in your project root:

```yaml
rules:
  - id: js-no-console-log
    name: No console.log statements
    severity: warning
    language: javascript
    query: |
      (call_expression
        function: (member_expression
          object: (identifier) @obj
          property: (property_identifier) @prop
          (#eq? @obj "console")
          (#eq? @prop "log"))
        arguments: (arguments) @args)
    message: "Remove console.log statements before committing code"

  - id: js-no-eval
    name: No eval() usage
    severity: error
    language: javascript
    query: |
      (call_expression
        function: (identifier) @func
        (#eq? @func "eval"))
    message: "Using eval() is dangerous and should be avoided"
```

### Severity Levels

- `error` — Critical issues that should block PRs
- `warning` — Problems that should be reviewed
- `note` — Informational findings

### Supported Languages

- JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`)
- TypeScript (`.ts`, `.tsx`)
- Python (`.py`)
- HTML (`.html`, `.htm`)

## CI/CD Integration

### GitHub Actions

```yaml
name: Code Scan

on: [push, pull_request]

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install scanner
        run: |
          curl -sSL https://github.com/crstnmac/tree-sitter-ci-scanner/releases/latest/download/scanner-x86_64-unknown-linux-gnu -o /usr/local/bin/scanner
          chmod +x /usr/local/bin/scanner
      - run: scanner scan src/ --recursive --output results.sarif
      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
          category: scanner
          codeql_path: /tmp/codeql
```

### GitLab CI

```yaml
code_scan:
  stage: scan
  script:
    - scanner scan src/ --recursive --output results.sarif
  artifacts:
    reports:
      sast: results.sarif
```

See `.github/workflows/scan.yml` and `.gitlab-ci.yml` for complete examples.

## Exit Codes

- `0` — No issues found
- `1` — Issues found
- `2` — Runtime error (e.g., invalid config, missing file)

## Examples

### Scanning with specific rules

```bash
# Only check for console.log and eval
scanner scan src/ --rules js-no-console-log,js-no-eval --output issues.json
```

### Multiple file types

```bash
# Scan a directory containing mixed languages
scanner scan project/ --recursive
```

### Custom config for monorepo

```yaml
# .scanner.yaml in monorepo root
rules:
  - id: custom-api-key-exposed
    name: API keys should not be exposed
    severity: error
    language: javascript
    query: |
      (assignment_expression
        left: (member_expression
          property: (property_identifier) @prop
          (#match? @prop "api.*key|secret|token"))
      right: (string) @value)
    message: "Found potential exposed API key or secret"
```

## SaaS Dashboard Server

The `server/` crate is a multi-tenant web server that adds a policy dashboard,
cross-repo findings visibility, and GitHub commit status enforcement on top of
the scanner CLI.

### Quick start with Docker

```bash
# 1. Generate secrets
export COOKIE_SECRET=$(openssl rand -hex 64)
export ENCRYPTION_KEY=$(openssl rand -hex 32)

# 2. Create a GitHub OAuth App at https://github.com/settings/developers
#    Set the callback URL to: http://localhost:3000/auth/github/callback

# 3. Create a .env file
cat > .env <<EOF
GITHUB_CLIENT_ID=your_client_id
GITHUB_CLIENT_SECRET=your_client_secret
GITHUB_CALLBACK_URL=http://localhost:3000/auth/github/callback
BASE_URL=http://localhost:3000
COOKIE_SECRET=${COOKIE_SECRET}
ENCRYPTION_KEY=${ENCRYPTION_KEY}
EOF

# 4. Start Postgres and the server
docker-compose up --build
```

Open http://localhost:3000 and sign in.

### Running locally (without Docker)

```bash
# Start Postgres
docker run -d --name scanner-db \
  -e POSTGRES_DB=scanner \
  -e POSTGRES_USER=scanner \
  -e POSTGRES_PASSWORD=scanner \
  -p 5432:5432 \
  postgres:16-alpine

# Source secrets
export DATABASE_URL=postgres://scanner:scanner@localhost:5432/scanner
export COOKIE_SECRET=$(openssl rand -hex 64)
export ENCRYPTION_KEY=$(openssl rand -hex 32)
export GITHUB_CLIENT_ID=your_client_id
export GITHUB_CLIENT_SECRET=your_client_secret
export GITHUB_CALLBACK_URL=http://localhost:3000/auth/github/callback
export BASE_URL=http://localhost:3000
export PORT=3000

# Build and run (migrations run automatically on startup)
cargo run -p server
```

### Authentication

Two methods are supported — use whichever suits your setup:

| Method | How |
|--------|-----|
| **Username + password** | Register at `/auth/register` |
| **GitHub OAuth** | Click "Continue with GitHub" on the login page |

The first user to create an account in an organisation becomes admin.

### Server environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | Postgres connection string |
| `PORT` | No (default: `3000`) | HTTP listen port |
| `GITHUB_CLIENT_ID` | Yes | GitHub OAuth App client ID |
| `GITHUB_CLIENT_SECRET` | Yes | GitHub OAuth App client secret |
| `GITHUB_CALLBACK_URL` | Yes | Must match the OAuth App's callback URL |
| `BASE_URL` | Yes | Public URL of this server |
| `COOKIE_SECRET` | Yes | Hex-encoded ≥64-byte key for cookie encryption (`openssl rand -hex 64`) |
| `ENCRYPTION_KEY` | Yes | Hex-encoded 32-byte key for PAT encryption at rest (`openssl rand -hex 32`) |
| `SESSION_HOURS` | No (default: `24`) | Session lifetime in hours |

### API endpoints

After creating an API key at `/settings/keys`, CI jobs can call the server directly:

```bash
# Fetch merged rules for your org
curl -H "Authorization: Bearer <key>" \
  http://localhost:3000/api/v1/rules

# Submit a SARIF result (evaluates policy + stores findings + posts GitHub status)
curl -X POST \
  -H "Authorization: Bearer <key>" \
  -H "Content-Type: application/json" \
  -d '{
    "sarif": { ...sarif_log... },
    "repo": "owner/repo",
    "commit_sha": "abc123",
    "branch": "main"
  }' \
  http://localhost:3000/api/v1/scan
```

### Build the server binary only

```bash
cargo build --release -p server
./target/release/server
```

## Contributing

Contributions welcome! Please open an issue or pull request.

### Development

```bash
# Clone repository
git clone https://github.com/crstnmac/tree-sitter-ci-scanner.git
cd tree-sitter-ci-scanner

# Build
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- scan test.js
```

## License

MIT OR Apache-2.0
