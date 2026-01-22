# Authentication Guide

Complete guide to authenticating with HTTP APIs and CLI tools in TermStack.

## Table of Contents

- [Overview](#overview)
- [HTTP API Authentication](#http-api-authentication)
  - [Bearer Tokens](#bearer-tokens)
  - [API Keys](#api-keys)
  - [Basic Authentication](#basic-authentication)
  - [Custom Headers](#custom-headers)
- [CLI Tool Authentication](#cli-tool-authentication)
  - [Environment Inheritance](#environment-inheritance)
  - [Custom Environment Variables](#custom-environment-variables)
  - [Credential Files](#credential-files)
- [Security Best Practices](#security-best-practices)
- [Environment Variables Support](#environment-variables-support)
- [Troubleshooting](#troubleshooting)

---

## Overview

TermStack supports multiple authentication methods for both HTTP APIs and CLI tools. Authentication is handled through:

- **HTTP APIs**: Headers with template interpolation
- **CLI Tools**: Environment variable inheritance and custom env vars
- **Template Engine**: Dynamic credential insertion using Tera templates

### Authentication Methods Supported

| Method | HTTP APIs | CLI Tools | Example Use Case |
|--------|-----------|-----------|------------------|
| Bearer Tokens | ✅ | N/A | GitHub API, REST APIs |
| API Keys | ✅ | N/A | Custom APIs, SaaS platforms |
| Basic Auth | ✅ | N/A | Jenkins, private APIs |
| Environment Variables | ✅ | ✅ | All scenarios (recommended) |
| Credential Files | N/A | ✅ | kubectl, aws, gh CLI |
| Custom Headers | ✅ | N/A | Custom auth schemes |

---

## HTTP API Authentication

HTTP authentication is handled through the `headers` field in your data source configuration. All header values support template interpolation using `{{ variable }}` syntax.

### Bearer Tokens

**Most common for modern APIs** (GitHub, GitLab, REST APIs)

#### Basic Usage

```yaml
version: v1

app:
  name: "API Browser"

globals:
  # Token stored in globals (use with caution - see security section)
  api_token: "ghp_your_github_token_here"

start: main

pages:
  main:
    title: "API Data"
    data:
      adapter: http
      url: "https://api.github.com/user/repos"
      method: GET
      headers:
        Authorization: "Bearer {{ api_token }}"
        Accept: "application/json"
      items: "$[*]"
```

#### Using Environment Variables (Recommended)

```yaml
globals:
  # SECURE: Read token from environment variable
  api_token: "{{ env.GITHUB_TOKEN }}"

pages:
  repos:
    data:
      adapter: http
      url: "https://api.github.com/user/repos"
      headers:
        Authorization: "Bearer {{ api_token }}"
```

**Usage:**
```bash
export GITHUB_TOKEN="ghp_your_token_here"
termstack config.yaml
```

#### Dynamic Tokens from Previous Pages

```yaml
pages:
  login:
    # First page gets token
    data:
      adapter: http
      url: "https://api.example.com/login"
      method: POST
      body: '{"username": "user", "password": "pass"}'
      items: "$"
    next:
      page: protected_data
      context:
        auth_token: "$.token"
  
  protected_data:
    # Second page uses token from login
    data:
      adapter: http
      url: "https://api.example.com/data"
      headers:
        Authorization: "Bearer {{ login.token }}"
```

---

### API Keys

**Common in SaaS APIs** (Stripe, SendGrid, custom APIs)

#### Header-Based API Key

```yaml
globals:
  api_key: "{{ env.API_KEY }}"

pages:
  data:
    data:
      adapter: http
      url: "https://api.example.com/v1/data"
      headers:
        X-API-Key: "{{ api_key }}"
        Content-Type: "application/json"
```

#### Authorization Header with API Key

```yaml
headers:
  Authorization: "ApiKey {{ api_key }}"
  # OR
  Authorization: "Token {{ api_key }}"
```

#### Multiple API Keys

```yaml
globals:
  primary_key: "{{ env.PRIMARY_API_KEY }}"
  secondary_key: "{{ env.SECONDARY_API_KEY }}"

pages:
  primary_data:
    data:
      adapter: http
      headers:
        X-API-Key: "{{ primary_key }}"
  
  secondary_data:
    data:
      adapter: http
      headers:
        X-API-Key: "{{ secondary_key }}"
```

---

### Basic Authentication

**Used in enterprise systems** (Jenkins, internal APIs, legacy systems)

#### Pre-encoded Credentials

```yaml
globals:
  # Generate: echo -n "username:password" | base64
  basic_auth: "{{ env.BASIC_AUTH_TOKEN }}"

pages:
  protected:
    data:
      adapter: http
      url: "https://jenkins.example.com/api/json"
      headers:
        Authorization: "Basic {{ basic_auth }}"
```

**Generate base64 token:**
```bash
# Linux/macOS
echo -n "username:password" | base64

# Result: dXNlcm5hbWU6cGFzc3dvcmQ=
export BASIC_AUTH_TOKEN="dXNlcm5hbWU6cGFzc3dvcmQ="
```

#### Dynamic Basic Auth

```yaml
globals:
  username: "{{ env.API_USERNAME }}"
  password: "{{ env.API_PASSWORD }}"
  # Note: Encoding must be done externally

pages:
  data:
    data:
      adapter: http
      url: "https://api.example.com/data"
      headers:
        Authorization: "Basic {{ env.ENCODED_CREDENTIALS }}"
```

---

### Custom Headers

**For custom authentication schemes** (proprietary APIs, multi-factor auth)

#### Single Custom Header

```yaml
globals:
  session_id: "{{ env.SESSION_ID }}"
  tenant_id: "{{ env.TENANT_ID }}"

pages:
  data:
    data:
      adapter: http
      headers:
        X-Session-ID: "{{ session_id }}"
        X-Tenant-ID: "{{ tenant_id }}"
        X-Request-ID: "{{ request_id }}"
```

#### Multiple Authentication Headers

```yaml
headers:
  X-API-Key: "{{ env.API_KEY }}"
  X-API-Secret: "{{ env.API_SECRET }}"
  X-Timestamp: "{{ timestamp }}"
  X-Signature: "{{ signature }}"
```

#### Conditional Headers

```yaml
pages:
  data:
    data:
      adapter: http
      url: "https://api.example.com/data"
      headers:
        Authorization: "Bearer {{ api_token }}"
        # Add debug header in development
        X-Debug-Mode: "{{ env.DEBUG | default(value='false') }}"
```

---

## CLI Tool Authentication

CLI tools typically authenticate using environment variables and credential files. TermStack processes inherit the parent environment and can override specific variables.

### Environment Inheritance

**How it works:**
- TermStack inherits **all** environment variables from the shell that launches it
- CLI commands executed by TermStack receive this environment
- Tools like `kubectl`, `aws`, `gh` read their standard config files automatically

#### Kubectl (Kubernetes)

```yaml
version: v1

app:
  name: "Kubernetes Browser"

start: namespaces

pages:
  namespaces:
    title: "Namespaces"
    data:
      adapter: cli
      command: "kubectl"
      args: ["get", "namespaces", "-o", "json"]
      items: "$.items[*]"
      # No auth config needed - inherits from ~/.kube/config
```

**Authentication:**
- kubectl reads `~/.kube/config` automatically
- Uses the active context
- Respects `KUBECONFIG` environment variable

**Setup:**
```bash
# kubectl is already configured
kubectl config current-context

# Run TermStack - automatically authenticated
termstack kubernetes.yaml
```

#### AWS CLI

```yaml
pages:
  s3_buckets:
    data:
      adapter: cli
      command: "aws"
      args: ["s3", "ls", "--output", "json"]
      # Inherits ~/.aws/credentials and ~/.aws/config
```

**Authentication:**
- Reads `~/.aws/credentials`
- Reads `~/.aws/config`
- Respects `AWS_PROFILE`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`

**Setup:**
```bash
# Configure AWS CLI
aws configure

# Or use profiles
aws configure --profile production

# Run TermStack
AWS_PROFILE=production termstack aws.yaml
```

#### GitHub CLI

```yaml
pages:
  repos:
    data:
      adapter: cli
      command: "gh"
      args: ["repo", "list", "--json", "name,description"]
      items: "$[*]"
      # Inherits authentication from gh CLI
```

**Authentication:**
```bash
# Login once
gh auth login

# TermStack automatically authenticated
termstack github-cli.yaml
```

---

### Custom Environment Variables

Override or add environment variables for specific commands.

#### Override Credential Paths

```yaml
pages:
  custom_kube:
    data:
      adapter: cli
      command: "kubectl"
      args: ["get", "pods", "-o", "json"]
      env:
        KUBECONFIG: "/custom/path/to/kubeconfig.yaml"
```

#### Set AWS Profile

```yaml
pages:
  production_s3:
    data:
      adapter: cli
      command: "aws"
      args: ["s3", "ls", "--output", "json"]
      env:
        AWS_PROFILE: "production"
        AWS_REGION: "us-west-2"
```

#### Custom Tool Configuration

```yaml
pages:
  custom_cli:
    data:
      adapter: cli
      command: "custom-tool"
      args: ["list"]
      env:
        TOOL_CONFIG_PATH: "{{ env.HOME }}/.config/tool/config.yaml"
        TOOL_API_KEY: "{{ env.TOOL_API_KEY }}"
        TOOL_DEBUG: "true"
```

#### Template Variables in Environment

```yaml
globals:
  environment: "production"
  region: "us-west-2"

pages:
  data:
    data:
      adapter: cli
      command: "aws"
      args: ["s3", "ls"]
      env:
        AWS_PROFILE: "{{ environment }}"
        AWS_REGION: "{{ region }}"
```

---

### Credential Files

Many CLI tools use credential files for authentication. Here's how they work with TermStack:

#### kubectl (~/.kube/config)

**File Location:** `~/.kube/config` (or `$KUBECONFIG`)

**Structure:**
```yaml
apiVersion: v1
clusters:
  - cluster:
      server: https://cluster.example.com
    name: my-cluster
contexts:
  - context:
      cluster: my-cluster
      user: my-user
    name: my-context
current-context: my-context
users:
  - name: my-user
    user:
      token: eyJhbGc...
```

**TermStack Usage:**
```yaml
# Automatically uses current-context
pages:
  pods:
    data:
      adapter: cli
      command: "kubectl"
      args: ["get", "pods", "-o", "json"]

# Or override config file
pages:
  custom_pods:
    data:
      adapter: cli
      command: "kubectl"
      args: ["get", "pods", "-o", "json"]
      env:
        KUBECONFIG: "/path/to/other/config"
```

#### AWS CLI (~/.aws/credentials)

**File Location:** `~/.aws/credentials`

**Structure:**
```ini
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[production]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
```

**TermStack Usage:**
```yaml
# Uses [default] profile
pages:
  s3:
    data:
      adapter: cli
      command: "aws"
      args: ["s3", "ls"]

# Use specific profile
pages:
  prod_s3:
    data:
      adapter: cli
      command: "aws"
      args: ["s3", "ls"]
      env:
        AWS_PROFILE: "production"
```

#### GitHub CLI (gh)

**Authentication:** Token stored by `gh auth login`

**Setup:**
```bash
gh auth login
```

**TermStack Usage:**
```yaml
pages:
  repos:
    data:
      adapter: cli
      command: "gh"
      args: ["repo", "list", "--json", "name"]
      items: "$[*]"
```

---

## Security Best Practices

### 🔒 DO

✅ **Use Environment Variables**
```yaml
globals:
  api_key: "{{ env.API_KEY }}"
```
```bash
export API_KEY="secret-key"
termstack config.yaml
```

✅ **Use Credential Files**
```bash
# Let tools read their standard config
kubectl, aws, gh, etc. read ~/.kube/config, ~/.aws/credentials
```

✅ **Add Secrets to .gitignore**
```bash
echo "*.secrets.yaml" >> .gitignore
echo ".env" >> .gitignore
echo "secrets/" >> .gitignore
```

✅ **Use Read-Only/Scoped Tokens**
```yaml
# GitHub: Use personal access token with minimal scopes
# AWS: Use IAM role with read-only permissions
# Kubernetes: Use ServiceAccount with viewer role
```

✅ **Rotate Credentials Regularly**
```bash
# GitHub tokens: Regenerate every 90 days
# AWS keys: Rotate every 90 days
# Kubernetes tokens: Use short-lived tokens
```

✅ **Use Shell Wrappers**
```bash
#!/bin/bash
# run-termstack.sh
export API_KEY=$(cat ~/.secrets/api-key.txt)
export GITHUB_TOKEN=$(cat ~/.secrets/github-token.txt)

termstack config.yaml
```

### ⛔ DON'T

❌ **Store Secrets in YAML Files**
```yaml
# BAD - visible in file, committed to git
globals:
  api_key: "sk-1234567890abcdef"
  password: "MyPassword123"
```

❌ **Commit Secrets to Git**
```bash
# BAD - secrets exposed in git history
git add config.yaml  # Contains hardcoded secrets
git commit -m "Add config"
```

❌ **Use Root/Admin Credentials**
```yaml
# BAD - excessive permissions
globals:
  aws_access_key: "AKIAIOSFODNN7EXAMPLE"  # Admin user
```

❌ **Share Config Files with Secrets**
```bash
# BAD - secrets exposed to others
slack upload config.yaml  # Contains API keys
```

❌ **Log or Display Secrets**
```yaml
# BAD - secrets in logs
pages:
  debug:
    title: "API Key: {{ api_key }}"  # Visible on screen
```

---

## Environment Variables Support

TermStack supports environment variable interpolation using the `{{ env.VAR }}` syntax in templates.

### Syntax

```yaml
globals:
  variable: "{{ env.ENV_VAR_NAME }}"
```

### Available Everywhere

Environment variables work in:

- ✅ **Globals section**
  ```yaml
  globals:
    api_key: "{{ env.API_KEY }}"
  ```

- ✅ **HTTP URLs**
  ```yaml
  url: "{{ env.API_BASE_URL }}/endpoint"
  ```

- ✅ **HTTP Headers**
  ```yaml
  headers:
    Authorization: "Bearer {{ env.AUTH_TOKEN }}"
  ```

- ✅ **CLI Arguments**
  ```yaml
  args: ["--api-key", "{{ env.API_KEY }}"]
  ```

- ✅ **CLI Environment Variables**
  ```yaml
  env:
    CUSTOM_VAR: "{{ env.SOURCE_VAR }}"
  ```

### Default Values

Use Tera's `default` filter to provide fallback values:

```yaml
globals:
  # Use DEBUG env var, default to 'false' if not set
  debug_mode: "{{ env.DEBUG | default(value='false') }}"
  
  # Use custom API base, default to production
  api_base: "{{ env.API_BASE | default(value='https://api.example.com') }}"
  
  # Use custom timeout, default to 30s
  timeout: "{{ env.TIMEOUT | default(value='30s') }}"
```

### Complete Example

```yaml
version: v1

app:
  name: "Secure API Browser"

globals:
  # All secrets from environment
  github_token: "{{ env.GITHUB_TOKEN }}"
  api_base: "{{ env.GITHUB_API_BASE | default(value='https://api.github.com') }}"
  timeout: "{{ env.API_TIMEOUT | default(value='30s') }}"

start: repos

pages:
  repos:
    title: "Repositories"
    data:
      adapter: http
      url: "{{ api_base }}/user/repos"
      headers:
        Authorization: "Bearer {{ github_token }}"
        User-Agent: "TermStack"
      timeout: "{{ timeout }}"
      items: "$[*]"
```

**Usage:**
```bash
# Required
export GITHUB_TOKEN="ghp_your_token_here"

# Optional (uses defaults if not set)
export GITHUB_API_BASE="https://api.github.com"
export API_TIMEOUT="60s"

termstack config.yaml
```

---

## Troubleshooting

### HTTP API Issues

#### 401 Unauthorized

**Symptom:** API returns 401 status code

**Causes:**
- Invalid or expired token
- Token not properly set in environment
- Missing `Authorization` header

**Solutions:**
```bash
# Verify environment variable is set
echo $GITHUB_TOKEN

# Check token is valid (GitHub example)
curl -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/user

# Verify YAML syntax
termstack --validate config.yaml
```

#### 403 Forbidden

**Symptom:** API returns 403 status code

**Causes:**
- Valid token but insufficient permissions
- Rate limiting
- API key doesn't have required scopes

**Solutions:**
```bash
# GitHub: Check token scopes at https://github.com/settings/tokens
# AWS: Check IAM permissions
# Custom API: Verify API key has required permissions
```

#### Missing Environment Variable

**Symptom:** Error: "Variable 'env.VAR_NAME' not found"

**Solutions:**
```bash
# Verify variable is set
printenv | grep VAR_NAME

# Set the variable
export VAR_NAME="value"

# Add to shell profile for persistence
echo 'export VAR_NAME="value"' >> ~/.bashrc
source ~/.bashrc
```

### CLI Tool Issues

#### Command Not Found

**Symptom:** Error: "command not found: kubectl"

**Solutions:**
```bash
# Verify command exists
which kubectl

# Install if missing
# kubectl: https://kubernetes.io/docs/tasks/tools/
# aws: https://aws.amazon.com/cli/
# gh: https://cli.github.com/
```

#### Invalid Credentials

**Symptom:** kubectl returns "Unauthorized" or aws returns "InvalidAccessKeyId"

**Solutions:**
```bash
# kubectl: Verify config
kubectl config view
kubectl config current-context

# aws: Verify credentials
aws configure list
aws sts get-caller-identity

# gh: Verify authentication
gh auth status
```

#### Wrong Context/Profile

**Symptom:** Command runs but shows wrong data (wrong cluster, wrong AWS account)

**Solutions:**
```bash
# kubectl: Switch context
kubectl config use-context <context-name>

# aws: Use specific profile
AWS_PROFILE=production termstack config.yaml

# Or set in YAML
env:
  AWS_PROFILE: "production"
```

---

## Examples

### Complete GitHub Example

See [GitHub API Browser](../cookbook/github-api.md) for a complete working example.

### Complete AWS Example

See [AWS CLI Integration](../cookbook/aws-cli.md) for comprehensive AWS examples.

### Quick Reference

```yaml
# HTTP Bearer Token
headers:
  Authorization: "Bearer {{ env.TOKEN }}"

# HTTP API Key
headers:
  X-API-Key: "{{ env.API_KEY }}"

# HTTP Basic Auth
headers:
  Authorization: "Basic {{ env.BASIC_AUTH }}"

# CLI with environment
data:
  adapter: cli
  command: "kubectl"
  args: ["get", "pods"]
  env:
    KUBECONFIG: "/path/to/config"
```

---

## See Also

- [HTTP Adapter Guide](http-adapter.md) - Detailed HTTP configuration
- [CLI Adapter Guide](cli-adapter.md) - Detailed CLI configuration
- [Templates & Context](templates-and-context.md) - Template syntax and variables
- [GitHub API Cookbook](../cookbook/github-api.md) - Complete GitHub integration
- [AWS CLI Cookbook](../cookbook/aws-cli.md) - Complete AWS integration
