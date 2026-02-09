<p align="center">
  <img src="assets/termstack.PNG" alt="TermStack Logo" width="600"/>
</p>

<h1 align="center">TermStack</h1>

<p align="center">
  <strong>Build beautiful TUIs with YAML. No code. Just vibes.</strong>
</p>

<p align="center">
  <a href="#-claude-code-integration-ai-powered-yaml-generation">AI Generation</a> •
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#examples">Examples</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.70+-orange.svg" alt="Rust Version"/>
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"/>
  <img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs Welcome"/>
  <img src="https://img.shields.io/badge/made%20with-coffee%20%E2%98%95-brown" alt="Made with Coffee"/>
</p>

---

TermStack is a config-driven Terminal User Interface (TUI) framework that lets you create powerful terminal dashboards using simple YAML configuration files. Inspired by the legendary [k9s](https://k9scli.io/), but for *everything*.

Think of it as "k9s for anything" — Kubernetes, REST APIs, dog breeds (yes, really), your custom CLI tools, or that weird internal API your company built in 2015 that nobody wants to touch.

## Demo

[![asciicast](https://asciinema.org/a/ozYJrUw1VZIpptEI.svg)](https://asciinema.org/a/ozYJrUw1VZIpptEI)

*Click to view the interactive terminal recording - you can pause, copy text, and replay at your own pace!*

## 🤖 Claude Code Integration (AI-Powered YAML Generation!)

**The fastest way to build TermStack configs** — Just describe what you want in plain English!

TermStack includes a Claude Code skill that auto-generates complete YAML configurations from natural language. Stop writing YAML manually and let AI handle the syntax, structure, and best practices.

### Quick Start

In Claude Code, just run:

```bash
/termstack "browse the Dog API and show breeds with their life expectancy"
```

Claude will:
1. ✅ Research the API (if needed)
2. ✅ Generate a complete, working YAML config
3. ✅ Save it to `examples/` 
4. ✅ Give you the command to run it

### What It Generates

The skill knows about **all** TermStack features:
- ✅ HTTP, CLI, Script, and Stream adapters
- ✅ Table, Text, and Logs views
- ✅ Conditional navigation and actions
- ✅ Styling, transforms, and filters
- ✅ Multi-page navigation with context passing
- ✅ Authentication patterns (Bearer tokens, API keys, etc.)
- ✅ Environment variable usage for secrets

### Example Prompts

```bash
# Browse any REST API
/termstack "create a GitHub repository browser with issues and PRs"

# Kubernetes dashboards
/termstack "k9s-style interface for viewing pods and logs"

# AWS CLI integration
/termstack "browse S3 buckets and view object lists"

# Custom APIs
/termstack "browse JSONPlaceholder posts and comments with user details"
```

**No more copy-pasting from docs.** Just describe what you want and let Claude figure out the YAML indentation (the hardest part, honestly).

### Learn More

- 📖 [Complete Documentation](docs/README.md) - Guides, cookbooks, and references
- 🔑 [Authentication Guide](docs/guides/authentication.md) - HTTP APIs & CLI auth patterns
- 📝 [Templates & Context](docs/guides/templates-and-context.md) - Variables, navigation, filters
- 🍳 [GitHub API Cookbook](docs/cookbook/github-api.md) - Real-world example
- ☁️ [AWS CLI Cookbook](docs/cookbook/aws-cli.md) - CLI integration patterns

---

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

**Option 1: Quick Install with curl (Recommended)**

Download and install the latest release automatically:

```bash
# macOS Apple Silicon (ARM64)
curl -fsSL https://github.com/pa/termstack/releases/latest/download/termstack-macos-arm64.tar.gz \
  | tar -xz \
  && chmod +x termstack \
  && sudo mv termstack /usr/local/bin/termstack

# macOS Intel (x86_64)
curl -fsSL https://github.com/pa/termstack/releases/latest/download/termstack-macos-amd64.tar.gz \
  | tar -xz \
  && chmod +x termstack \
  && sudo mv termstack /usr/local/bin/termstack

# Linux x86_64
curl -fsSL https://github.com/pa/termstack/releases/latest/download/termstack-linux-amd64.tar.gz \
  | tar -xz \
  && chmod +x termstack \
  && sudo mv termstack /usr/local/bin/termstack

# Linux ARM64
curl -fsSL https://github.com/pa/termstack/releases/latest/download/termstack-linux-arm64.tar.gz \
  | tar -xz \
  && chmod +x termstack \
  && sudo mv termstack /usr/local/bin/termstack

# Verify installation
termstack --help
```

**Option 2: Manual Download**

Download pre-built binaries from the [Releases page](https://github.com/pa/termstack/releases):

| Platform | Architecture | Download |
|----------|--------------|----------|
| macOS | Intel (x86_64) | `termstack-macos-amd64.tar.gz` |
| macOS | Apple Silicon (ARM64) | `termstack-macos-arm64.tar.gz` |
| Linux | x86_64 | `termstack-linux-amd64.tar.gz` |
| Linux | ARM64 | `termstack-linux-arm64.tar.gz` |

```bash
# Extract and install
tar -xzf termstack-<platform>.tar.gz
chmod +x termstack
sudo mv termstack /usr/local/bin/

# Verify
termstack --help
```

**Or build from source:**

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
termstack hello.yaml
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
termstack examples/dog-api.yaml
```

Uses the amazing [DogAPI](https://dogapi.dog/) — a free, open API with no authentication required. Perfect for demos, testing, or just learning about Corgis at 2 AM.

### Kubernetes Dashboard (Because k9s wasn't enough)

A k9s-style interface for when you need YAML-ception:

```bash
termstack examples/kubernetes-cli.yaml
```

Navigate Namespaces → Pods → Logs → Existential Crisis about your YAML indentation.

### More Examples

| Example | Description | Command |
|---------|-------------|---------|
| `dog-api.yaml` | Browse dog breeds and facts | `termstack examples/dog-api.yaml` |
| `kubernetes-cli.yaml` | Kubernetes resource browser | `termstack examples/kubernetes-cli.yaml` |
| `stream-test.yaml` | Streaming logs demo | `termstack examples/stream-test.yaml` |
| `style-test.yaml` | Styling capabilities | `termstack examples/style-test.yaml` |

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

Press `Shift+A` to open the action menu, or use `Ctrl+key` shortcuts directly:

```yaml
actions:
  - key: "ctrl+d"
    name: "Delete"
    confirm: "Really delete {{ name }}? (no undo!)"
    command: "kubectl"
    args: ["delete", "pod", "{{ name }}"]
    refresh: true
  - key: "ctrl+v"
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
| `/` | Search (`%col% term` for column) |
| `Shift+A` | Action menu |
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
- The first working prototype was pair-programmed with Claude over many cups of coffee
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

# Run with example (development)
cargo run -- examples/dog-api.yaml

# Or use the installed binary
termstack examples/dog-api.yaml
```

## Troubleshooting

**Q: My YAML isn't working!**

A: Check your indentation. Then check it again. YAML is 90% indentation anxiety.

**Q: The TUI is blank!**

A: Make sure your data source is accessible. Try `--verbose` for debug output.

**Q: Actions aren't triggering!**

A: Press `Shift+A` to open the action menu, or use `Ctrl+key` shortcuts directly.

**Q: Can I use this in production?**

A: Technically yes. Should you? Ask your manager, not us.

## License

MIT License — Do whatever you want, just don't blame us.

## Author

**Pramodh Ayyappan** ([@pa](https://github.com/pa))

Built in partnership with Claude as a pair programming assistant while learning Rust. A testament to what modern AI-assisted development can achieve when human creativity meets AI capabilities.

## Acknowledgments

- [k9s](https://k9scli.io/) — The inspiration for this madness
- [ratatui](https://ratatui.rs/) — Making terminal UIs actually fun
- Coffee — The real MVP

---

<p align="center">
  Made with mass amounts of mass by a developer who believes terminals deserve better UX
</p>

<p align="center">
  <sub>If you read this far, you deserve a cookie. Go get one.</sub>
</p>
