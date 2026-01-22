# TermStack Documentation

**Complete documentation for building config-driven Terminal User Interfaces with TermStack.**

---

## 📚 Quick Navigation

### Getting Started
- **[Main README](../README.md)** - Project overview and quick start
- **[Installation](../README.md#installation)** - Download and install TermStack
- **[Your First TUI](../README.md#your-first-tui-in-30-seconds)** - 30-second tutorial

### Guides
- **[⭐ Authentication Guide](guides/authentication.md)** - **HTTP APIs & CLI authentication**
- [Templates & Context](../README.md#template-filters) - Tera template syntax
- [Configuration](../README.md#configuration) - YAML configuration overview

### Cookbook (Real Examples)
- **[⭐ GitHub API Browser](cookbook/github-api.md)** - **Complete GitHub integration**
- **[⭐ AWS CLI Integration](cookbook/aws-cli.md)** - **S3, EC2, Lambda dashboards**
- [Kubernetes Dashboard](../examples/kubernetes-cli.yaml) - k9s-style interface
- [Dog API Browser](../examples/dog-api.yaml) - REST API example

### Reference
- [Configuration Schema](../README.md#configuration) - Complete YAML reference
- [Keybindings](../README.md#keybindings) - All keyboard shortcuts
- [Template Filters](../README.md#template-filters) - Built-in filters
- [Technical Specification](SPECIFICATION.md) - Architecture details

---

## 🔑 Authentication Quick Reference

### HTTP APIs (Bearer Token)
```yaml
globals:
  api_token: "{{ env.GITHUB_TOKEN }}"

pages:
  repos:
    data:
      adapter: http
      url: "https://api.github.com/user/repos"
      headers:
        Authorization: "Bearer {{ api_token }}"
```

```bash
export GITHUB_TOKEN="ghp_your_token"
termstack config.yaml
```

### HTTP APIs (API Key)
```yaml
headers:
  X-API-Key: "{{ env.API_KEY }}"
```

### CLI Tools (Automatic)
```yaml
# kubectl inherits ~/.kube/config automatically
data:
  adapter: cli
  command: "kubectl"
  args: ["get", "pods", "-o", "json"]
```

### CLI Tools (Custom Env)
```yaml
data:
  adapter: cli
  command: "aws"
  args: ["s3", "ls"]
  env:
    AWS_PROFILE: "production"
    AWS_REGION: "us-west-2"
```

**[→ See Full Authentication Guide](guides/authentication.md)**

---

## 🚀 Quick Start

### Installation
```bash
# Download binary or build from source
cargo build --release

# Run example
termstack examples/dog-api.yaml
```

### Hello World
```yaml
version: v1

app:
  name: "My First TUI"

start: main

pages:
  main:
    title: "Hello, TermStack!"
    data:
      adapter: cli
      command: "echo"
      args: ['[{"message": "Welcome!", "status": "awesome"}]']
      items: "$[*]"
    view:
      type: table
      columns:
        - path: "$.message"
          display: "Message"
        - path: "$.status"
          display: "Status"
```

---

## 📖 Documentation Structure

### For Beginners
1. **Start here**: [Main README](../README.md)
2. **Try examples**: `examples/dog-api.yaml`, `examples/kubernetes-cli.yaml`
3. **Learn auth**: [Authentication Guide](guides/authentication.md)
4. **Build real apps**: [GitHub Example](cookbook/github-api.md)

### For Advanced Users
1. **Architecture**: [Technical Specification](SPECIFICATION.md)
2. **Performance**: [Optimization Docs](../OPTIMIZATION_COMPLETE.md)
3. **Memory**: Deep navigation with LRU cache (auto-enabled)

---

## 🔥 Popular Examples

### GitHub API Browser
Browse repositories, issues, and pull requests with the GitHub REST API.

**Features**: Bearer token auth, multi-page navigation, conditional styling

**[→ Complete Example](cookbook/github-api.md)**

### AWS CLI Integration
Browse S3 buckets, EC2 instances, and Lambda functions using AWS CLI.

**Features**: Profile/region support, credential files, status indicators

**[→ Complete Example](cookbook/aws-cli.md)**

### Kubernetes Dashboard
k9s-style interface for Kubernetes resources.

**Features**: Auto-auth via kubectl, logs streaming, resource actions

**[→ See YAML](../examples/kubernetes-cli.yaml)**

---

## 📝 Configuration Overview

### Basic Structure
```yaml
version: v1

app:
  name: "App Name"

globals:
  api_key: "{{ env.API_KEY }}"

start: main_page

pages:
  main_page:
    title: "Page Title"
    data:
      adapter: http  # or cli, stream, script
      url: "https://api.example.com/data"
      headers:
        Authorization: "Bearer {{ api_key }}"
      items: "$.data[*]"
    view:
      type: table
      columns:
        - path: "$.name"
          display: "Name"
```

### Data Adapters

| Adapter | Use Case | Auth Method |
|---------|----------|-------------|
| `http` | REST APIs | Headers (Bearer, API Key, Basic) |
| `cli` | Command-line tools | Environment vars, credential files |
| `stream` | Real-time logs | Same as CLI |
| `script` | Custom scripts | Environment vars |

### Views

| View | Use Case | Features |
|------|----------|----------|
| `table` | List data | Columns, sorting, filtering, styling |
| `text` | Details | Syntax highlighting (YAML, JSON, etc.) |
| `logs` | Streaming | Follow mode, wrapping, ANSI colors |

---

## 🆘 Getting Help

### Common Issues

**"401 Unauthorized"** → Check your API token  
**"Command not found"** → Install CLI tool (kubectl, aws, gh)  
**"Template error"** → Verify environment variable is set  
**"Empty results"** → Check JSONPath expression in `items:`

### Resources

- **Examples**: Browse `examples/` directory for working configs
- **GitHub Issues**: Report bugs at [github.com/pa/termstack](https://github.com/pa/termstack)
- **Authentication**: [Complete Auth Guide](guides/authentication.md)

---

## ⚡ Quick Tips

### Security
✅ Use `{{ env.VAR }}` for secrets  
✅ Add `*.secrets.yaml` to `.gitignore`  
✅ Use read-only credentials  
❌ Never commit tokens to git

### Performance
- Auto-refresh: `refresh_interval: "5m"`
- Timeouts: `timeout: "30s"`
- Memory: LRU cache auto-manages deep navigation

### Debugging
```bash
# Validate config
termstack --validate config.yaml

# Verbose output
termstack --verbose config.yaml
```

---

## 📊 Feature Matrix

| Feature | Status | Example |
|---------|--------|---------|
| HTTP APIs | ✅ | [GitHub](cookbook/github-api.md) |
| CLI Tools | ✅ | [AWS](cookbook/aws-cli.md), [kubectl](../examples/kubernetes-cli.yaml) |
| Environment Variables | ✅ | `{{ env.VAR }}` |
| Template Engine | ✅ | Tera templates |
| Multi-page Navigation | ✅ | Context passing |
| Conditional Routing | ✅ | Based on data |
| Actions | ✅ | Commands, HTTP, navigation |
| Styling | ✅ | Colors, conditions |
| Streaming Logs | ✅ | Follow mode |
| Auto-refresh | ✅ | Configurable intervals |

---

## 🎯 What's Next?

### Build Your First Integration

1. **Pick a data source**:
   - REST API? → [Authentication Guide](guides/authentication.md)
   - CLI tool? → [AWS Example](cookbook/aws-cli.md)

2. **Start with a template**:
   - GitHub API? → [Copy this](cookbook/github-api.md#complete-configuration)
   - AWS? → [Copy this](cookbook/aws-cli.md#complete-configuration)
   - Custom? → [See README](../README.md#your-first-tui-in-30-seconds)

3. **Test and iterate**:
   ```bash
   termstack --validate config.yaml
   termstack config.yaml
   ```

### Learn More

- **Authentication**: [Complete guide](guides/authentication.md) with all methods
- **GitHub Integration**: [Step-by-step](cookbook/github-api.md) with token setup
- **AWS Integration**: [Multiple services](cookbook/aws-cli.md) with profiles

---

<p align="center">
  <strong>Made with ❤️ by the TermStack community</strong><br>
  <sub>Build terminal UIs without writing code</sub>
</p>
