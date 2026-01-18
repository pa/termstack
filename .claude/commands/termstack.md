---
name: termstack
description: Generate a TermStack YAML configuration for a TUI that browses APIs or displays data
arguments:
  - name: prompt
    description: Description of what TUI you want to create (e.g., "browse GitHub repos", "display weather data")
    required: true
allowed-tools:
  - Read
  - Write
  - WebFetch
  - Bash
---

# TermStack YAML Generator

Generate a TermStack TUI configuration based on the user's requirements.

## Instructions

1. **Understand the request**: Parse what API or data source the user wants to browse
2. **Research the API**: If needed, use WebFetch to check the API documentation or test endpoints
3. **Generate the YAML**: Create a complete, working TermStack configuration
4. **Save the file**: Write to `examples/` directory with a descriptive name
5. **Test the config**: Run `cargo run -- examples/filename.yaml` to validate

## Key Rules

- Use `{{ variable }}` for template variables (NOT `{{ page.variable }}`)
- Context variables passed via `next.context` are accessed directly by key name
- Always include proper JSONPath for `items` field to extract arrays
- Add meaningful column styling with colors
- Include both `next` (for Enter) and `actions` (for shortcuts)

## Reference

Load the skill file for complete documentation:
- `.claude/skills/termstack-yaml-generator/SKILL.md`

## Examples

Reference working examples:
- `examples/dog-api.yaml` - REST API browser
- `examples/kubernetes-cli.yaml` - CLI-based k8s dashboard

## Output

Generate the YAML and save it to `examples/[name].yaml`, then provide instructions to run it.
