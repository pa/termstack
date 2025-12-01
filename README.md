# TermStack

A generic TUI (Terminal User Interface) framework for building rich, navigable dashboards using simple YAML configuration files. No UI coding required!

Inspired by [k9s](https://k9scli.io/), TermStack enables you to create powerful terminal applications with just YAML.

## Features

- 🎨 **Config-driven**: Define pages, data sources, views, and actions in YAML
- 🔄 **Dynamic rendering**: Automatically renders tables, details, logs, and YAML views
- 📡 **Multi-source data**: Fetch from CLI commands or HTTP endpoints
- 🎭 **Template engine**: Use Tera templates for dynamic content
- 🧭 **Navigation**: Navigate between pages with context passing
- ⚡ **Async**: Non-blocking data fetching with responsive UI

## Quick Start

### Installation

```bash
# Clone the repository
git clone <repo-url>
cd termstack

# Build the project
cargo build --release
```

### Run Examples

```bash
# Validate a config
cargo run -- examples/kubernetes.yaml --validate

# Run the Kubernetes TUI (requires kubectl)
cargo run -- examples/kubernetes.yaml

# Run with verbose output
cargo run -- examples/kubernetes.yaml --verbose
```

### Usage

```bash
termstack [OPTIONS] <CONFIG>

Arguments:
  <CONFIG>  Path to the YAML configuration file

Options:
  -v, --validate  Validate config and exit (don't run TUI)
  -V, --verbose   Verbose output
  -h, --help      Print help
```

## Configuration

Create a YAML file defining your TUI:

```yaml
version: v1

app:
  name: "My Dashboard"
  description: "Custom dashboard"

globals:
  api_url: "http://localhost:8080"

start: main_page

pages:
  main_page:
    title: "Main Page"
    data:
      type: cli
      command: "kubectl"
      args: ["get", "pods", "-o", "json"]
      items: "$.items[*]"
    view:
      layout: table
      columns:
        - path: "$.metadata.name"
          display: "Name"
        - path: "$.status.phase"
          display: "Status"
    next:
      page: detail_page
      context:
        pod_name: "$.metadata.name"
```

See [examples/](examples/) for more complete examples:
- `kubernetes.yaml` - Kubernetes resource browser
- `raptor.yaml` - Facets Raptor TUI

## Example: Kubernetes Browser

The included Kubernetes example provides a k9s-style interface:

- **Namespaces** → **Pods** → **Pod Details** → **Logs**
- **Namespaces** → **Deployments** → **Deployment Details**
- **Namespaces** → **Services** → **Service Details**

### Features:
- Navigate with `j`/`k` or arrow keys
- Press `Enter` to drill down
- Press `Esc` to go back
- Press `d` to delete resources (with confirmation)
- Press `l` to view logs
- Press `y` to see raw YAML
- Press `q` to quit

## Architecture

TermStack is built on:

- **[ratatui](https://ratatui.rs/)** - Terminal UI framework
- **[tera](https://tera.netlify.app/)** - Template engine
- **[tokio](https://tokio.rs/)** - Async runtime
- **[serde](https://serde.rs/)** - Serialization
- **[reqwest](https://docs.rs/reqwest/)** - HTTP client

## Current Status

**Phase 1 Complete** ✅
- Configuration system with validation
- Data providers (CLI, HTTP, JSONPath, caching)
- Template engine with custom filters
- Navigation scaffolding
- Error handling

**In Progress** 🚧
- View rendering (table, detail, YAML)
- Input handling and keybindings
- Action execution
- Full integration

See [PROGRESS.md](PROGRESS.md) for detailed status.

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Check

```bash
cargo check
```

### Documentation

```bash
cargo doc --open
```

## Configuration Reference

See [docs/SPECIFICATION.md](docs/SPECIFICATION.md) for complete configuration schema and examples.

### Key Concepts

- **Pages**: Define views and navigation flow
- **Data Sources**: CLI commands or HTTP endpoints
- **Views**: Table, detail, logs, or YAML layouts
- **Actions**: Execute commands or navigate
- **Templates**: Use `{{ variables }}` for dynamic content
- **JSONPath**: Extract data with `$.path.to.field`

### Template Variables

- `{{ globals.var }}` - Global variables
- `{{ page.field }}` - Data from previous pages
- `{{ value }}` - Current row value
- `{{ row.field }}` - Current row field

### Custom Filters

- `{{ timestamp | timeago }}` - "2h ago"
- `{{ bytes | filesizeformat }}` - "1.5 GB"
- `{{ status | status_color }}` - Color mapping

## Keybindings

### Global
- `q` / `Esc` - Quit / Go back
- `?` - Help
- `Ctrl+C` - Force quit

### Navigation (planned)
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Go to top
- `G` - Go to bottom
- `Enter` - Select / Navigate

### Actions (planned)
- `r` - Refresh
- `/` - Search
- `y` - YAML view

## Contributing

See [PROGRESS.md](PROGRESS.md) for current development status and next steps.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Inspired by [k9s](https://k9scli.io/) - Kubernetes CLI manager.

---

**Note**: This is an early development version. Full TUI functionality is still being implemented.
