# GitHub API Browser

Build a terminal-based GitHub repository and issue browser using the GitHub REST API.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Complete Configuration](#complete-configuration)
- [Usage Instructions](#usage-instructions)
- [Features](#features)
- [Security Notes](#security-notes)
- [Troubleshooting](#troubleshooting)
- [API Reference](#api-reference)

---

## Overview

This example demonstrates how to build a functional GitHub repository browser using TermStack and the GitHub REST API. It includes:

- **Repository listing** with sorting and styling
- **Issue browser** with status filtering
- **Multi-page navigation** with context passing
- **Secure authentication** using environment variables

**What You'll Build:**
```
Repositories (list) → Repository Details (YAML view)
                   → Issues (list)
```

---

## Prerequisites

### 1. GitHub Personal Access Token

You need a GitHub personal access token to authenticate with the API.

**Create Token:**
1. Visit [https://github.com/settings/tokens](https://github.com/settings/tokens)
2. Click "Generate new token" → "Generate new token (classic)"
3. Give it a descriptive name (e.g., "TermStack Browser")
4. Select scopes:
   - ✅ `repo` (Full control of private repositories)
   - ✅ `read:org` (Read org and team membership)
5. Click "Generate token"
6. **Copy the token immediately** (you won't see it again!)

**Token Format:** `ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`

### 2. Set Environment Variable

```bash
# Linux/macOS
export GITHUB_TOKEN="ghp_your_token_here"

# Add to shell profile for persistence
echo 'export GITHUB_TOKEN="ghp_your_token_here"' >> ~/.bashrc
source ~/.bashrc

# Verify it's set
echo $GITHUB_TOKEN
```

### 3. TermStack Installation

```bash
# Install TermStack if not already installed
cargo install termstack

# Or build from source
git clone https://github.com/pa/termstack.git
cd termstack
cargo build --release
```

---

## Complete Configuration

Save this as `github-repos.yaml`:

```yaml
# =============================================================================
# GitHub Repository Browser
# =============================================================================
# Browse your GitHub repositories and issues using the GitHub REST API.
#
# Prerequisites:
# 1. GitHub personal access token with 'repo' and 'read:org' scopes
# 2. Set environment variable: export GITHUB_TOKEN="ghp_your_token"
#
# Usage: termstack github-repos.yaml
#
# This example demonstrates:
# - HTTP adapter with Bearer token authentication
# - Environment variable usage for secure token storage
# - Multi-page navigation with context passing
# - Conditional styling based on status
# - JSONPath data extraction
# - Template interpolation in URLs and headers
#
# API Documentation: https://docs.github.com/en/rest
# =============================================================================

version: v1

app:
  name: "GitHub Repository Browser"
  description: "Browse your GitHub repos and issues"
  theme: "default"

globals:
  # SECURITY: Token read from environment variable (not stored in file)
  github_token: "{{ env.GITHUB_TOKEN }}"
  
  # GitHub API base URL (can be overridden for GitHub Enterprise)
  api_base: "{{ env.GITHUB_API_BASE | default(value='https://api.github.com') }}"

start: repos

pages:
  # ==========================================================================
  # REPOSITORIES - List all repositories for authenticated user
  # ==========================================================================
  repos:
    title: "My Repositories"
    description: "All repositories you have access to"
    
    data:
      adapter: http
      url: "{{ api_base }}/user/repos"
      method: GET
      
      headers:
        # Bearer token authentication
        Authorization: "Bearer {{ github_token }}"
        
        # GitHub API version (recommended)
        Accept: "application/vnd.github.v3+json"
        
        # User agent (required by GitHub)
        User-Agent: "TermStack-GitHub-Browser"
      
      # Query parameters
      params:
        sort: "updated"      # Sort by last updated
        direction: "desc"    # Newest first
        per_page: "100"      # Max items per page
        type: "all"          # all, owner, public, private, member
      
      # Extract array of repositories
      items: "$[*]"
      
      # Auto-refresh every 5 minutes
      refresh_interval: "5m"
      
      # Timeout for API request
      timeout: "30s"
    
    view:
      type: table
      
      columns:
        # Repository name
        - path: "$.name"
          display: "Repository"
          width: 40
          style:
            - default: true
              color: cyan
              bold: true
        
        # Visibility (public/private)
        - path: "$.visibility"
          display: "Visibility"
          width: 10
          style:
            - condition: "{{ value == 'public' }}"
              color: green
            - condition: "{{ value == 'private' }}"
              color: yellow
            - default: true
              color: gray
        
        # Star count
        - path: "$.stargazers_count"
          display: "⭐ Stars"
          width: 10
          style:
            - default: true
              color: yellow
        
        # Fork count
        - path: "$.forks_count"
          display: "🔱 Forks"
          width: 10
          style:
            - default: true
              color: blue
        
        # Open issues count
        - path: "$.open_issues_count"
          display: "Issues"
          width: 8
          style:
            - condition: "{{ value | int > 0 }}"
              color: red
            - default: true
              color: green
        
        # Primary language
        - path: "$.language"
          display: "Language"
          width: 15
          style:
            - default: true
              color: magenta
        
        # Last updated time
        - path: "$.updated_at"
          display: "Updated"
          width: 20
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray
      
      # Sort by name column
      sort:
        column: "$.name"
        order: asc
    
    # Navigation: Press Enter to view repository details
    next:
      page: repo_detail
      context:
        repo_name: "$.full_name"
        repo_url: "$.url"
        repo_owner: "$.owner.login"
    
    # Actions: Press 'a' then action key
    actions:
      - key: "i"
        name: "View Issues"
        description: "Browse repository issues"
        page: "repo_issues"
        context:
          repo_name: "$.full_name"
          repo_owner: "$.owner.login"

  # ==========================================================================
  # REPOSITORY DETAILS - Detailed view of selected repository
  # ==========================================================================
  repo_detail:
    title: "Repository: {{ repos.name }}"
    description: "Detailed information about {{ repos.full_name }}"
    
    data:
      adapter: http
      url: "{{ repos.url }}"
      method: GET
      
      headers:
        Authorization: "Bearer {{ github_token }}"
        Accept: "application/vnd.github.v3+json"
        User-Agent: "TermStack-GitHub-Browser"
      
      timeout: "30s"
    
    view:
      type: text
      syntax: yaml  # Display as formatted YAML
    
    # Press Enter to go back to repository list
    actions:
      - key: "i"
        name: "View Issues"
        description: "Browse issues for this repository"
        page: "repo_issues"
        context:
          repo_name: "{{ repos.full_name }}"

  # ==========================================================================
  # REPOSITORY ISSUES - List issues for selected repository
  # ==========================================================================
  repo_issues:
    title: "Issues - {{ repos.name }}"
    description: "Open issues for {{ repos.full_name }}"
    
    data:
      adapter: http
      url: "{{ api_base }}/repos/{{ repos.full_name }}/issues"
      method: GET
      
      headers:
        Authorization: "Bearer {{ github_token }}"
        Accept: "application/vnd.github.v3+json"
        User-Agent: "TermStack-GitHub-Browser"
      
      params:
        state: "open"        # open, closed, all
        sort: "updated"      # created, updated, comments
        direction: "desc"    # asc, desc
        per_page: "50"
      
      items: "$[*]"
      
      refresh_interval: "2m"
      timeout: "30s"
    
    view:
      type: table
      
      columns:
        # Issue number
        - path: "$.number"
          display: "#"
          width: 6
          style:
            - default: true
              color: cyan
        
        # Issue title
        - path: "$.title"
          display: "Title"
          width: 60
          style:
            - default: true
              color: white
              bold: true
        
        # State (open/closed)
        - path: "$.state"
          display: "State"
          width: 10
          style:
            - condition: "{{ value == 'open' }}"
              color: green
              bold: true
            - condition: "{{ value == 'closed' }}"
              color: red
            - default: true
              color: gray
        
        # Comment count
        - path: "$.comments"
          display: "💬"
          width: 5
          style:
            - condition: "{{ value | int > 0 }}"
              color: cyan
            - default: true
              color: gray
        
        # Author
        - path: "$.user.login"
          display: "Author"
          width: 15
          style:
            - default: true
              color: magenta
        
        # Created time
        - path: "$.created_at"
          display: "Created"
          width: 20
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray
      
      sort:
        column: "$.updated_at"
        order: desc
```

---

## Usage Instructions

### 1. Save the Configuration

Save the YAML configuration above as `github-repos.yaml` in your project directory.

### 2. Set Your GitHub Token

```bash
export GITHUB_TOKEN="ghp_your_actual_token_here"
```

**Verify it's set:**
```bash
echo $GITHUB_TOKEN
# Should output: ghp_xxxxx...
```

### 3. Run TermStack

```bash
termstack github-repos.yaml
```

### 4. Navigate the Interface

**Main View (Repositories):**
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Jump to top
- `G` - Jump to bottom
- `Enter` - View repository details
- `a` then `i` - View repository issues
- `/` - Search repositories
- `r` - Refresh data
- `q` - Quit
- `Esc` - Go back

**Repository Details View:**
- Scroll through YAML details
- `a` then `i` - View issues
- `Esc` - Back to repository list

**Issues View:**
- Browse issues for the selected repository
- `Esc` - Back to repository list

---

## Features

### Authentication

- ✅ **Secure**: Token stored in environment variable, not in file
- ✅ **Bearer Token**: Standard OAuth2 authentication
- ✅ **API Versioning**: Uses GitHub API v3
- ✅ **User Agent**: Required by GitHub API

### Repository List

- ✅ **Sorting**: By name, updated date
- ✅ **Filtering**: All repos, public, private
- ✅ **Statistics**: Stars, forks, issues
- ✅ **Language**: Primary programming language
- ✅ **Styling**: Color-coded visibility and issues
- ✅ **Time**: Human-readable "updated 2 hours ago"

### Repository Details

- ✅ **Full Info**: Complete repository metadata
- ✅ **YAML Format**: Pretty-printed, syntax highlighted
- ✅ **Scrollable**: Navigate large responses

### Issue Tracking

- ✅ **Status**: Open/closed with color coding
- ✅ **Sorting**: By created, updated, comments
- ✅ **Metadata**: Author, comment count, timestamps
- ✅ **Navigation**: Back to repository list

---

## Security Notes

### ✅ DO

**Use Environment Variables:**
```yaml
globals:
  github_token: "{{ env.GITHUB_TOKEN }}"
```

**Add to .gitignore:**
```bash
echo "*.secrets.yaml" >> .gitignore
echo ".env" >> .gitignore
```

**Use Scoped Tokens:**
- Only grant required scopes (repo, read:org)
- Don't use tokens with admin access
- Create separate tokens for different tools

**Rotate Tokens:**
```bash
# Regenerate tokens every 90 days
# Visit: https://github.com/settings/tokens
```

### ❌ DON'T

**Hardcode Tokens:**
```yaml
# BAD - token visible in file
globals:
  github_token: "ghp_visible_token_in_file"
```

**Commit Secrets:**
```bash
# BAD - token in git history
git add github-repos.yaml  # Contains token
git commit
```

**Share Config Files:**
```bash
# BAD - exposes your token
slack upload github-repos.yaml
```

---

## Troubleshooting

### 401 Unauthorized

**Problem:** API returns "Bad credentials" or 401 status

**Solutions:**
1. Verify token is set:
   ```bash
   echo $GITHUB_TOKEN
   ```

2. Check token is valid:
   ```bash
   curl -H "Authorization: Bearer $GITHUB_TOKEN" \
     https://api.github.com/user
   ```

3. Regenerate token if expired:
   - Visit [https://github.com/settings/tokens](https://github.com/settings/tokens)
   - Delete old token
   - Generate new token with same scopes

### 403 Forbidden

**Problem:** API returns "Resource not accessible by integration" or 403 status

**Solutions:**
1. Check token scopes:
   - Visit [https://github.com/settings/tokens](https://github.com/settings/tokens)
   - Verify `repo` and `read:org` are checked

2. Wait for rate limit reset:
   ```bash
   curl -H "Authorization: Bearer $GITHUB_TOKEN" \
     https://api.github.com/rate_limit
   ```

### Empty Repository List

**Problem:** No repositories shown

**Solutions:**
1. Check if you have any repositories:
   ```bash
   curl -H "Authorization: Bearer $GITHUB_TOKEN" \
     https://api.github.com/user/repos | jq length
   ```

2. Adjust `type` parameter:
   ```yaml
   params:
     type: "all"  # Try: all, owner, public, private, member
   ```

### Template Error

**Problem:** Error: "Variable 'env.GITHUB_TOKEN' not found"

**Solutions:**
1. Set environment variable:
   ```bash
   export GITHUB_TOKEN="ghp_your_token"
   ```

2. Verify TermStack version supports `{{ env.VAR }}` syntax

3. Use shell wrapper as fallback:
   ```bash
   #!/bin/bash
   export GITHUB_TOKEN="ghp_your_token"
   termstack github-repos.yaml
   ```

---

## API Reference

### GitHub API Documentation

- **REST API**: [https://docs.github.com/en/rest](https://docs.github.com/en/rest)
- **Authentication**: [https://docs.github.com/en/rest/authentication](https://docs.github.com/en/rest/authentication)
- **Rate Limits**: [https://docs.github.com/en/rest/rate-limit](https://docs.github.com/en/rest/rate-limit)

### Endpoints Used

| Endpoint | Purpose | Docs |
|----------|---------|------|
| `GET /user/repos` | List user repositories | [Link](https://docs.github.com/en/rest/repos/repos#list-repositories-for-the-authenticated-user) |
| `GET /repos/{owner}/{repo}` | Get repository details | [Link](https://docs.github.com/en/rest/repos/repos#get-a-repository) |
| `GET /repos/{owner}/{repo}/issues` | List repository issues | [Link](https://docs.github.com/en/rest/issues/issues#list-repository-issues) |

### Rate Limits

**Authenticated requests:**
- 5,000 requests per hour
- Check remaining: `curl -H "Authorization: Bearer $TOKEN" https://api.github.com/rate_limit`

**Unauthenticated requests:**
- 60 requests per hour (don't do this)

---

## Extensions

Want to add more features? Try these:

### Pull Requests

```yaml
pages:
  repo_prs:
    data:
      adapter: http
      url: "{{ api_base }}/repos/{{ repos.full_name }}/pulls"
      headers:
        Authorization: "Bearer {{ github_token }}"
      params:
        state: "open"
```

### Releases

```yaml
pages:
  repo_releases:
    data:
      adapter: http
      url: "{{ api_base }}/repos/{{ repos.full_name }}/releases"
      headers:
        Authorization: "Bearer {{ github_token }}"
```

### Organizations

```yaml
pages:
  orgs:
    data:
      adapter: http
      url: "{{ api_base }}/user/orgs"
      headers:
        Authorization: "Bearer {{ github_token }}"
```

---

## See Also

- [Authentication Guide](../guides/authentication.md) - Comprehensive auth documentation
- [Templates & Context Guide](../guides/templates-and-context.md) - Template syntax and navigation
- [AWS CLI Integration](aws-cli.md) - Similar example for AWS
- [Documentation Hub](../README.md) - Central documentation index
