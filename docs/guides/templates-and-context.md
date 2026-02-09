# Templates & Context Guide

Complete guide to TermStack's template engine, context variables, and dynamic content rendering.

## Table of Contents

- [Overview](#overview)
- [Template Syntax](#template-syntax)
- [Context Variables](#context-variables)
  - [Globals](#globals)
  - [Page Context](#page-context)
  - [Current Row](#current-row)
  - [Environment Variables](#environment-variables)
- [Navigation & Context Passing](#navigation--context-passing)
- [Template Filters](#template-filters)
- [JSONPath Expressions](#jsonpath-expressions)
- [Common Patterns](#common-patterns)
- [Troubleshooting](#troubleshooting)

---

## Overview

TermStack uses the [Tera](https://tera.netlify.app/) template engine (similar to Jinja2) for dynamic content rendering. Templates are used throughout your configuration to:

- Interpolate variables in URLs, commands, and arguments
- Build dynamic page titles
- Transform and format data
- Apply conditional styling
- Pass context between pages

**Template Syntax:** `{{ variable }}` or `{{ variable | filter }}`

Templates can be used in:
- Page titles
- Data adapter URLs and commands
- Column transforms
- Conditional expressions
- Action confirmations
- Navigation context

---

## Template Syntax

### Basic Interpolation

```yaml
# Simple variable
title: "{{ page_name }}"

# With filter
transform: "{{ value | upper }}"

# Expression
condition: "{{ value > 100 }}"

# Complex expression
transform: "{{ value | filesizeformat | upper }}"
```

### Checking for Templates

TermStack only evaluates strings containing `{{` and `}}`:

```yaml
# These are templates
title: "Pod: {{ pod_name }}"
url: "{{ api_base }}/users/{{ user_id }}"

# These are NOT templates (literals)
title: "Static Title"
url: "https://api.example.com/users"
```

---

## Context Variables

The template context provides four namespaces of variables:

### 1. Globals

**Definition:** Variables defined in the `globals:` section of your config.

**Scope:** Available everywhere in your configuration.

**Use Case:** API base URLs, common credentials, shared configuration.

**Example:**

```yaml
globals:
  api_base: "https://api.github.com"
  github_token: "{{ env.GITHUB_TOKEN }}"
  org_name: "mycompany"
  default_timeout: "30s"

pages:
  repos:
    data:
      adapter: http
      url: "{{ api_base }}/orgs/{{ org_name }}/repos"
      headers:
        Authorization: "Bearer {{ github_token }}"
      timeout: "{{ default_timeout }}"
```

**Access Pattern:**
```yaml
{{ variable_name }}  # Direct access by name
```

---

### 2. Page Context

**Definition:** Data passed from one page to another during navigation.

**Scope:** Available on the target page after navigation.

**Use Case:** Drill-down navigation (list → detail), passing IDs, names, or any data.

**How It Works:**

1. **Source Page** — Define `next.context` with JSONPath expressions
2. **TermStack** — Extracts values from the selected row
3. **Target Page** — Access values as top-level variables

**Example:**

```yaml
pages:
  # ========================================
  # Source Page (List View)
  # ========================================
  repos:
    title: "Repositories"
    data:
      adapter: http
      url: "{{ api_base }}/repos"
      items: "$[*]"
    view:
      type: table
      columns:
        - path: "$.name"
          display: "Name"
        - path: "$.owner.login"
          display: "Owner"
    next:
      page: repo_detail
      context:
        # Extract values from selected row
        repo_name: "$.name"           # → becomes {{ repo_name }}
        repo_owner: "$.owner.login"   # → becomes {{ repo_owner }}
        repo_id: "$.id"               # → becomes {{ repo_id }}
        full_name: "$.full_name"      # → becomes {{ full_name }}

  # ========================================
  # Target Page (Detail View)
  # ========================================
  repo_detail:
    # Use context variables in title
    title: "{{ repo_owner }}/{{ repo_name }}"
    data:
      adapter: http
      # Use context variables in URL
      url: "{{ api_base }}/repos/{{ repo_owner }}/{{ repo_name }}"
    view:
      type: text
      syntax: yaml
    actions:
      - key: "ctrl+i"
        name: "Issues"
        page: repo_issues
        context:
          # Pass context forward to next page
          repo_owner: "{{ repo_owner }}"
          repo_name: "{{ repo_name }}"
```

**Access Pattern:**
```yaml
{{ context_variable_name }}  # Direct access by the key name you defined
```

**Important Notes:**

- Context variables have the **same priority** as globals
- If a context variable has the same name as a global, the context variable wins
- Context is page-specific and doesn't persist when navigating away
- You can pass context forward by re-mapping it in the next page's actions

---

### 3. Current Row

**Definition:** Data from the currently selected row in a table view.

**Scope:** Available in `condition` expressions for styling and conditional navigation.

**Use Case:** Row-specific styling, conditional routing based on row data.

**Example:**

```yaml
pages:
  pods:
    data:
      adapter: cli
      command: "kubectl"
      args: ["get", "pods", "-o", "json"]
      items: "$.items[*]"
    view:
      type: table
      columns:
        - path: "$.metadata.name"
          display: "Pod"
        - path: "$.status.phase"
          display: "Status"
          style:
            # {{ value }} refers to the column's value
            - condition: "{{ value == 'Running' }}"
              color: green
            - condition: "{{ value == 'Failed' }}"
              color: red
            - default: true
              color: yellow
      row_style:
        # {{ row }} gives access to entire row object
        - condition: "{{ row.status.phase == 'Terminating' }}"
          dim: true
        - condition: "{{ row.status.phase == 'Failed' }}"
          color: red
    next:
      # Conditional navigation based on row data
      - condition: "{{ row.kind == 'Pod' }}"
        page: pod_detail
      - condition: "{{ row.kind == 'Service' }}"
        page: service_detail
      - default: true
        page: generic_detail
```

**Access Patterns:**
```yaml
{{ value }}           # Current column value (in column conditions)
{{ row.field }}       # Any field in the current row
{{ row.nested.data }} # Nested field access
```

---

### 4. Environment Variables

**Definition:** System environment variables loaded at startup.

**Scope:** Available everywhere in your configuration.

**Use Case:** Secure credential management, environment-specific configuration.

**Example:**

```yaml
globals:
  # Use environment variables for secrets
  github_token: "{{ env.GITHUB_TOKEN }}"
  api_key: "{{ env.API_KEY }}"
  aws_profile: "{{ env.AWS_PROFILE | default(value='default') }}"

pages:
  repos:
    data:
      adapter: http
      url: "{{ env.GITHUB_API | default(value='https://api.github.com') }}/repos"
      headers:
        Authorization: "Bearer {{ env.GITHUB_TOKEN }}"
```

**Access Pattern:**
```yaml
{{ env.VARIABLE_NAME }}                           # Direct access
{{ env.VARIABLE_NAME | default(value='fallback') }} # With fallback
```

**Setting Environment Variables:**

```bash
# Inline
export GITHUB_TOKEN="ghp_your_token"
termstack config.yaml

# Or in shell profile
echo 'export GITHUB_TOKEN="ghp_your_token"' >> ~/.bashrc

# Or use .env file (make sure to .gitignore it!)
cat > .env << EOF
GITHUB_TOKEN=ghp_your_token
API_KEY=your_api_key
EOF

# Load and run
set -a; source .env; set +a
termstack config.yaml
```

**Security Best Practices:**

✅ **DO:**
- Use `{{ env.VAR }}` for all credentials
- Add `.env` to `.gitignore`
- Use different tokens per environment
- Rotate tokens regularly

❌ **DON'T:**
- Hardcode credentials in YAML
- Commit credentials to version control
- Share production credentials
- Use the same token everywhere

**See Also:** [Authentication Guide](authentication.md) for comprehensive credential management.

---

## Navigation & Context Passing

### Simple Navigation

Press `Enter` on a table row to navigate:

```yaml
pages:
  users:
    # ... table view ...
    next:
      page: user_detail
      context:
        user_id: "$.id"
        user_name: "$.name"
```

### Conditional Navigation

Route to different pages based on row data:

```yaml
pages:
  files:
    # ... table view ...
    next:
      # Check type and route accordingly
      - condition: "{{ row.type == 'directory' }}"
        page: directory_view
        context:
          path: "$.path"
          name: "$.name"
      - condition: "{{ row.type == 'file' }}"
        page: file_view
        context:
          path: "$.path"
          name: "$.name"
      - default: true
        page: unknown_view
```

### Action-Based Navigation

Use actions to navigate with custom context:

```yaml
pages:
  repos:
    # ... table view ...
    actions:
      - key: "ctrl+i"
        name: "Issues"
        description: "View repository issues"
        page: issues
        context:
          repo_owner: "$.owner.login"
          repo_name: "$.name"
      - key: "ctrl+r"
        name: "Pull Requests"
        description: "View pull requests"
        page: pull_requests
        context:
          repo_owner: "$.owner.login"
          repo_name: "$.name"
```

**Usage:**
1. Press `Shift+A` to open the action menu (or use `Ctrl+key` shortcut directly)
2. Select the action (e.g., issues)
3. Navigate to the target page with context

### Multi-Level Navigation

Pass context through multiple pages:

```yaml
pages:
  # Level 1: Organizations
  orgs:
    next:
      page: repos
      context:
        org_name: "$.name"

  # Level 2: Repositories (receives org_name)
  repos:
    title: "{{ org_name }} Repositories"
    data:
      url: "{{ api_base }}/orgs/{{ org_name }}/repos"
    next:
      page: issues
      context:
        # Pass forward from previous page
        org_name: "{{ org_name }}"
        # Add new context
        repo_name: "$.name"

  # Level 3: Issues (receives both org_name and repo_name)
  issues:
    title: "{{ org_name }}/{{ repo_name }} Issues"
    data:
      url: "{{ api_base }}/repos/{{ org_name }}/{{ repo_name }}/issues"
```

---

## Template Filters

Filters transform values using the pipe syntax: `{{ value | filter }}`

### Built-in Tera Filters

```yaml
# String manipulation
transform: "{{ value | upper }}"           # UPPERCASE
transform: "{{ value | lower }}"           # lowercase
transform: "{{ value | title }}"           # Title Case
transform: "{{ value | trim }}"            # Remove whitespace

# Truncation
transform: "{{ value | truncate(length=50) }}"

# Default values
transform: "{{ value | default(value='N/A') }}"

# Array operations
transform: "{{ value | length }}"          # Array/string length
transform: "{{ value | first }}"           # First element
transform: "{{ value | last }}"            # Last element
transform: "{{ value | join(sep=', ') }}"  # Join array

# JSON
transform: "{{ value | json_encode }}"     # Convert to JSON string
```

### Custom TermStack Filters

```yaml
# Time ago (relative time)
transform: "{{ value | timeago }}"
# Input:  "2024-01-20T10:30:00Z"
# Output: "2 hours ago"

# File size formatting
transform: "{{ value | filesizeformat }}"
# Input:  1536000
# Output: "1.5 MB"

# Status color (for conditional styling)
style:
  - condition: "{{ value | status_color == 'green' }}"
    color: green
```

### Chaining Filters

```yaml
# Apply multiple filters in sequence
transform: "{{ value | default(value='unknown') | upper | trim }}"

# With JSONPath
transform: "{{ row.name | default(value='N/A') | truncate(length=30) }}"
```

---

## JSONPath Expressions

JSONPath is used to extract data from JSON responses.

### Basic Syntax

```yaml
# Root element
items: "$"

# Array of all items
items: "$[*]"

# Nested array
items: "$.data[*]"

# Deeply nested
items: "$.response.items[*]"

# Filter (where status is active)
items: "$[?(@.status=='active')]"
```

### Common Patterns

```yaml
# GitHub API (JSON:API format)
items: "$.data[*]"

# Kubernetes
items: "$.items[*]"

# Simple array
items: "$[*]"

# Single object
items: "$"
```

### Column Paths

```yaml
columns:
  # Simple field
  - path: "$.name"
  
  # Nested field
  - path: "$.owner.login"
  
  # Array index
  - path: "$.tags[0]"
  
  # Array length
  - path: "$.items"
    transform: "{{ value | length }}"
```

### Context Extraction

```yaml
next:
  page: detail
  context:
    # Extract simple field
    id: "$.id"
    
    # Extract nested field
    owner: "$.owner.login"
    
    # Extract from array
    first_tag: "$.tags[0]"
    
    # Pass entire object (useful for complex data)
    full_data: "$"
```

---

## Common Patterns

### Pattern 1: Master-Detail Navigation

```yaml
pages:
  # Master: List of items
  items:
    title: "Items"
    data:
      adapter: http
      url: "{{ api_base }}/items"
      items: "$[*]"
    view:
      type: table
      columns:
        - path: "$.id"
          display: "ID"
        - path: "$.name"
          display: "Name"
    next:
      page: item_detail
      context:
        item_id: "$.id"
        item_name: "$.name"

  # Detail: Single item
  item_detail:
    title: "Item: {{ item_name }}"
    data:
      adapter: http
      url: "{{ api_base }}/items/{{ item_id }}"
    view:
      type: text
      syntax: yaml
```

### Pattern 2: Context Forwarding

```yaml
pages:
  # Level 1
  users:
    next:
      page: repos
      context:
        user_id: "$.id"
        user_name: "$.login"

  # Level 2
  repos:
    title: "{{ user_name }}'s Repositories"
    data:
      url: "{{ api_base }}/users/{{ user_id }}/repos"
    next:
      page: commits
      context:
        # Forward previous context
        user_id: "{{ user_id }}"
        user_name: "{{ user_name }}"
        # Add new context
        repo_name: "$.name"

  # Level 3
  commits:
    title: "{{ user_name }}/{{ repo_name }} Commits"
    data:
      url: "{{ api_base }}/repos/{{ user_id }}/{{ repo_name }}/commits"
```

### Pattern 3: Dynamic Commands

```yaml
pages:
  namespaces:
    next:
      page: pods
      context:
        namespace: "$.metadata.name"

  pods:
    title: "Pods in {{ namespace }}"
    data:
      adapter: cli
      command: "kubectl"
      # Use context in command arguments
      args: ["get", "pods", "-n", "{{ namespace }}", "-o", "json"]
      items: "$.items[*]"
```

### Pattern 4: Environment-Specific Configuration

```yaml
globals:
  # Use different values per environment
  api_base: "{{ env.API_BASE | default(value='https://api.example.com') }}"
  env_name: "{{ env.ENV | default(value='development') }}"
  timeout: "{{ env.TIMEOUT | default(value='30s') }}"

pages:
  main:
    title: "Dashboard ({{ env_name }})"
    data:
      adapter: http
      url: "{{ api_base }}/status"
      timeout: "{{ timeout }}"
```

### Pattern 5: Conditional Actions

```yaml
actions:
  # Action with dynamic confirmation
  - key: "ctrl+d"
    name: "Delete"
    confirm: "Delete {{ row.name }} in {{ namespace }}? This cannot be undone!"
    command: "kubectl"
    args: ["delete", "pod", "{{ row.metadata.name }}", "-n", "{{ namespace }}"]
    refresh: true

  # Action with conditional visibility (in row condition)
  - key: "ctrl+r"
    name: "Restart"
    condition: "{{ row.status.phase == 'Failed' }}"
    command: "kubectl"
    args: ["rollout", "restart", "deployment", "{{ row.metadata.name }}"]
```

### Pattern 6: Dynamic Titles with Breadcrumbs

```yaml
pages:
  orgs:
    title: "Organizations"
    # ...

  repos:
    title: "{{ org_name }} > Repositories"
    # ...

  issues:
    title: "{{ org_name }} > {{ repo_name }} > Issues"
    # ...

  issue_detail:
    title: "{{ org_name }} > {{ repo_name }} > Issue #{{ issue_number }}"
    # ...
```

---

## Troubleshooting

### Issue: Variables Not Rendering

**Symptom:** `{{ variable }}` appears literally in output

**Causes & Solutions:**

```yaml
# ❌ BAD: Variable not defined
title: "{{ missing_var }}"

# ✅ GOOD: Use default filter
title: "{{ missing_var | default(value='Unknown') }}"

# ❌ BAD: Wrong context namespace
title: "{{ page_contexts.user_name }}"  # Don't access internal structure

# ✅ GOOD: Direct access
title: "{{ user_name }}"  # Context variables are top-level
```

### Issue: Context Not Passing

**Symptom:** Variables empty on target page

**Causes & Solutions:**

```yaml
# ❌ BAD: Wrong JSONPath
next:
  page: detail
  context:
    user_id: "id"  # Missing $. prefix

# ✅ GOOD: Correct JSONPath
next:
  page: detail
  context:
    user_id: "$.id"

# ❌ BAD: Trying to access data that doesn't exist
next:
  page: detail
  context:
    user_id: "$.data.user.id"  # Check your JSON structure!

# ✅ GOOD: Match your actual data structure
# If JSON is: {"id": 123, "name": "Alice"}
next:
  page: detail
  context:
    user_id: "$.id"
```

### Issue: Template Syntax Errors

**Symptom:** Tera rendering errors in verbose output

**Causes & Solutions:**

```yaml
# ❌ BAD: Invalid filter syntax
transform: "{{ value|upper }}"  # Missing spaces

# ✅ GOOD: Proper spacing
transform: "{{ value | upper }}"

# ❌ BAD: Unclosed brackets
title: "{{ name }"

# ✅ GOOD: Properly closed
title: "{{ name }}"

# ❌ BAD: Wrong quotes
condition: "{{ value == "active" }}"  # Nested quotes conflict

# ✅ GOOD: Use single quotes inside
condition: "{{ value == 'active' }}"
```

### Issue: JSONPath Not Extracting Data

**Symptom:** Empty table or no data displayed

**Debug Steps:**

1. **Check the raw JSON structure:**
   ```yaml
   view:
     type: text
     syntax: yaml  # View raw response
   ```

2. **Try different JSONPath patterns:**
   ```yaml
   items: "$"           # Root object
   items: "$[*]"        # Root array
   items: "$.data[*]"   # Nested in "data"
   items: "$.items[*]"  # Nested in "items"
   ```

3. **Check for JSON:API format:**
   ```yaml
   # Many APIs use this structure:
   # { "data": [...] }
   items: "$.data[*]"
   ```

### Issue: Environment Variables Not Loading

**Symptom:** `{{ env.VAR }}` is empty

**Solutions:**

```bash
# Check if variable is set
echo $GITHUB_TOKEN

# Set it if not
export GITHUB_TOKEN="your_token"

# Verify TermStack can see it
env | grep GITHUB_TOKEN

# Run TermStack
termstack config.yaml
```

**In config:**
```yaml
# Always use defaults for optional env vars
globals:
  token: "{{ env.TOKEN | default(value='') }}"
  
  # Or fail fast if required
  token: "{{ env.TOKEN }}"  # Will error if not set
```

### Debugging Tips

**Enable verbose mode:**
```bash
termstack --verbose config.yaml
```

**Add debug transforms:**
```yaml
columns:
  - path: "$"
    display: "Debug (Full Row)"
    width: 80
    transform: "{{ value | json_encode }}"
```

**Check context on target page:**
```yaml
pages:
  detail:
    title: "Debug: user_id={{ user_id }}, name={{ user_name }}"
```

---

## See Also

- [Authentication Guide](authentication.md) - Environment variables for credentials
- [GitHub API Cookbook](../cookbook/github-api.md) - Real-world navigation examples
- [AWS CLI Cookbook](../cookbook/aws-cli.md) - CLI adapter with context passing
- [Documentation Hub](../README.md) - All TermStack documentation

---

## Quick Reference

### Context Variable Priority

1. `env.*` - Environment variables (always available)
2. `globals.*` - Global variables (merged into top-level)
3. Page context - Passed from previous page (merged into top-level)
4. `value` - Current column value (in column conditions only)
5. `row.*` - Current row data (in conditions only)

### Template Locations

Templates can be used in:

- ✅ `globals.*` values
- ✅ `pages.*.title`
- ✅ `pages.*.data.url`
- ✅ `pages.*.data.command`
- ✅ `pages.*.data.args[*]`
- ✅ `pages.*.view.columns[*].transform`
- ✅ `pages.*.view.columns[*].style[*].condition`
- ✅ `pages.*.next.condition`
- ✅ `pages.*.next.context.*` (values)
- ✅ `pages.*.actions[*].confirm`
- ✅ `pages.*.actions[*].args[*]`
- ✅ `pages.*.actions[*].context.*`

### Filter Cheat Sheet

```yaml
# String
{{ value | upper }}
{{ value | lower }}
{{ value | title }}
{{ value | trim }}
{{ value | truncate(length=50) }}

# Arrays
{{ value | length }}
{{ value | first }}
{{ value | last }}
{{ value | join(sep=', ') }}

# Default values
{{ value | default(value='N/A') }}

# Custom
{{ value | timeago }}
{{ value | filesizeformat }}

# Chaining
{{ value | default(value='') | upper | trim }}
```

---

**Pro Tips:**

1. **Always use defaults** for optional environment variables
2. **Debug with verbose mode** when templates aren't working
3. **Use `type: text` views** to inspect raw JSON responses
4. **Extract IDs and names** in context for better navigation
5. **Forward context** when navigating multiple levels deep
6. **Use single quotes** inside template expressions
7. **Check JSONPath** in online testers before using in config

Happy templating! 🎉
