<p align="center">
  <img src="https://raw.githubusercontent.com/pa/termstack/main/assets/logo.png" alt="TermStack Logo" width="200"/>
</p>

<h1 align="center">TermStack</h1>

<p align="center">
  <strong>Build beautiful TUIs with YAML. No code. Just vibes.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#examples">Examples</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#keybindings">Keybindings</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.70+-orange.svg" alt="Rust Version"/>
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"/>
  <img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs Welcome"/>
  <img src="https://img.shields.io/badge/made%20with-coffee%20%E2%98%95-brown" alt="Made with Coffee"/>
</p>

---

> **"Why write 1000 lines of Rust when you can write 50 lines of YAML?"**
> 
> — Someone who's definitely not lazy, just efficient

TermStack is a config-driven Terminal User Interface (TUI) framework that lets you create powerful terminal dashboards using simple YAML configuration files. Inspired by the legendary [k9s](https://k9scli.io/), but for *everything*.

Think of it as "k9s for anything" — Kubernetes, REST APIs, dog breeds (yes, really), your custom CLI tools, or that weird internal API your company built in 2015 that nobody wants to touch.

## Features

- **Config-driven** — Define pages, data sources, views, and actions in YAML. Your keyboard will thank you.
- **Multiple Data Adapters** — HTTP APIs, CLI commands, scripts, and streaming data. We don't discriminate.
- **Rich Views** — Tables, text, logs, YAML views. Make your terminal pretty (finally).
- **Template Engine** — Tera templates for dynamic content. `{{ variables }}` everywhere!
- **Navigation** — Drill down into data like you're mining for Bitcoin, but actually useful.
- **Conditional Routing** — Different pages for different data types. Files vs folders? We got you.
- **Actions** — Execute commands, delete stuff (with confirmation, we're not monsters), refresh data.
- **Styling** — Color-code everything. Because life's too short for monochrome terminals.
- **Async** — Non-blocking data fetching. Your UI stays responsive while we do the heavy lifting.

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/pa/termstack.git
cd termstack

# Build it (grab a coffee, Rust compilation needs it)
cargo build --release

# The binary is now at ./target/release/termstack
```

### Your First TUI in 30 Seconds

Create `hello.yaml`:

```yaml
version: v1

app:
  name: "My First TUI"
  description: "Look mom, no code!"

start: main

pages:
  main:
    title: "Hello, Terminal!"
    data:
      adapter: cli
      command: "echo"
      args: ['[{"message": "Welcome to TermStack!", "status": "awesome"}]']
      items: "$[*]"
    view:
      type: table
      columns:
        - path: "$.message"
          display: "Message"
          width: 40
        - path: "$.status"
          display: "Status"
          width: 20
          style:
            - default: true
              color: green
              bold: true
```

Run it:

```bash
cargo run -- hello.yaml
```

Congratulations! You just built a TUI without writing a single line of code. Your CS professor would be so proud (or horrified, either way).

### Usage

```bash
termstack [OPTIONS] <CONFIG>

Arguments:
  <CONFIG>  Path to the YAML configuration file

Options:
  -v, --validate  Validate config and exit (for the paranoid)
  -V, --verbose   Verbose output (for debugging those 3 AM sessions)
  -h, --help      Print help
```

## Examples

### Dog Breeds Browser (Real API, No Auth!)

Browse dog breeds like you're on Tinder, but for pets:

```bash
cargo run -- examples/dog-api.yaml
```

Uses the amazing [DogAPI](https://dogapi.dog/) — a free, open API with no authentication required. Perfect for demos, testing, or just learning about Corgis at 2 AM.

### Kubernetes Dashboard (Because k9s wasn't enough)

A k9s-style interface for when you need YAML-ception:

```bash
cargo run -- examples/kubernetes-cli.yaml
```

Navigate Namespaces → Pods → Logs → Existential Crisis about your YAML indentation.

### More Examples

| Example | Description | Command |
|---------|-------------|---------|
| `dog-api.yaml` | Browse dog breeds and facts | `cargo run -- examples/dog-api.yaml` |
| `kubernetes-cli.yaml` | Kubernetes resource browser | `cargo run -- examples/kubernetes-cli.yaml` |
| `stream-test.yaml` | Streaming logs demo | `cargo run -- examples/stream-test.yaml` |
| `style-test.yaml` | Styling capabilities | `cargo run -- examples/style-test.yaml` |

## Configuration

### Basic Structure

```yaml
version: v1

app:
  name: "App Name"
  description: "What it does"
  theme: "default"  # We have themes! (just the one, but it's nice)

globals:
  api_base: "https://api.example.com"
  # Variables accessible everywhere as {{ variable_name }}

start: main_page  # Where the magic begins

pages:
  main_page:
    title: "Page Title"
    data:
      adapter: http  # or cli, script, stream
      url: "{{ api_base }}/endpoint"
      items: "$.data[*]"  # JSONPath is your friend
    view:
      type: table
      columns:
        - path: "$.name"
          display: "Name"
          width: 30
    next:
      page: detail_page
      context:
        item_id: "$.id"
```

### Data Adapters

#### HTTP — For REST APIs

```yaml
data:
  adapter: http
  url: "https://dogapi.dog/api/v2/breeds"
  method: GET
  headers:
    Accept: "application/json"
  items: "$.data[*]"
  timeout: "30s"
  refresh_interval: "5m"  # Auto-refresh!
```

#### CLI — For shell commands

```yaml
data:
  adapter: cli
  command: "kubectl"
  args: ["get", "pods", "-o", "json"]
  items: "$.items[*]"
```

#### Stream — For real-time data

```yaml
data:
  type: stream
  command: "kubectl"
  args: ["logs", "-f", "my-pod"]
  buffer_size: 100
  follow: true
```

### Views

**Table** — The workhorse:
```yaml
view:
  type: table
  columns:
    - path: "$.name"
      display: "Name"
      width: 30
      style:
        - condition: "{{ value == 'active' }}"
          color: green
        - default: true
          color: white
```

**Text** — For detailed views:
```yaml
view:
  type: text
  syntax: yaml  # Syntax highlighting!
```

**Logs** — For streaming:
```yaml
view:
  type: logs
  follow: true
  wrap: true
```

### Navigation

**Simple** (Enter key):
```yaml
next:
  page: detail_page
  context:
    item_id: "$.id"
```

**Conditional** (Smart routing):
```yaml
next:
  - condition: "{{ row.type == 'folder' }}"
    page: folder_view
  - condition: "{{ row.type == 'file' }}"
    page: file_view
  - default: true
    page: fallback
```

### Actions

Press `a` to enter action mode, then the action key:

```yaml
actions:
  - key: "d"
    name: "Delete"
    confirm: "Really delete {{ name }}? (no undo!)"
    command: "kubectl"
    args: ["delete", "pod", "{{ name }}"]
    refresh: true
  - key: "v"
    name: "View Details"
    page: "detail_page"
```

### Styling

Make it pretty:

```yaml
style:
  - condition: "{{ value == 'Running' }}"
    color: green
    bold: true
  - condition: "{{ value == 'Failed' }}"
    color: red
  - default: true
    color: gray
```

Available colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `gray`

### Template Filters

```yaml
# Time ago
transform: "{{ value | timeago }}"  # "2 hours ago"

# File size
transform: "{{ value | filesizeformat }}"  # "1.5 MB"

# String manipulation
transform: "{{ value | upper }}"  # "SHOUTING"
```

## Keybindings

| Key | Action | 
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` | Go to top |
| `G` | Go to bottom |
| `Enter` | Select / Navigate |
| `Esc` | Go back |
| `/` | Search |
| `a` | Action mode |
| `r` | Refresh |
| `q` | Quit |

## Architecture

Built with Rust and love:

- **[ratatui](https://ratatui.rs/)** — Terminal UI framework (the good stuff)
- **[tera](https://tera.netlify.app/)** — Template engine (Jinja2, but Rusty)
- **[tokio](https://tokio.rs/)** — Async runtime (zoom zoom)
- **[serde](https://serde.rs/)** — Serialization (YAML → Rust magic)
- **[reqwest](https://docs.rs/reqwest/)** — HTTP client (fetch all the things)

## Open Source APIs Used for Testing

Big shoutout to these awesome free APIs that made testing TermStack a joy:

| API | Description | Auth | Link |
|-----|-------------|------|------|
| **DogAPI** | Dog breeds, facts, and groups | None | [dogapi.dog](https://dogapi.dog/) |
| **JSONPlaceholder** | Fake REST API for testing | None | [jsonplaceholder.typicode.com](https://jsonplaceholder.typicode.com/) |
| **httpbin** | HTTP request & response testing | None | [httpbin.org](https://httpbin.org/) |

## Fun Facts

- TermStack was born because someone got tired of writing the same table rendering code for the 47th time
- The first working prototype was built entirely on coffee and spite
- "YAML" stands for "YAML Ain't Markup Language" and we're not sorry about the recursion
- The `q` key quits because `quit` has too many letters
- Every bug is a feature waiting to be documented

## Contributing

We welcome contributions! Here's how:

1. Fork the repo
2. Create a branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Commit (`git commit -m 'Add amazing feature'`)
6. Push (`git push origin feature/amazing-feature`)
7. Open a PR

### Development

```bash
# Build
cargo build

# Test
cargo test

# Check (fast compile check)
cargo check

# Run with example
cargo run -- examples/dog-api.yaml
```

## Troubleshooting

**Q: My YAML isn't working!**

A: Check your indentation. Then check it again. YAML is 90% indentation anxiety.

**Q: The TUI is blank!**

A: Make sure your data source is accessible. Try `--verbose` for debug output.

**Q: Actions aren't triggering!**

A: Press `a` first to enter action mode, then your action key.

**Q: Can I use this in production?**

A: Technically yes. Should you? Ask your manager, not us.

## License

MIT License — Do whatever you want, just don't blame us.

## Acknowledgments

- [k9s](https://k9scli.io/) — The inspiration for this madness
- [ratatui](https://ratatui.rs/) — Making terminal UIs actually fun
- Coffee — The real MVP
- That one Stack Overflow answer from 2019 that saved the day

---

<p align="center">
  Made with and by developers who believe terminals deserve better UX
</p>

<p align="center">
  <sub>If you read this far, you deserve a cookie. Go get one.</sub>
</p>
