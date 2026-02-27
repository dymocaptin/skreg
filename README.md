# skreg

A package registry for AI coding assistant skills.

## What is skreg?

skreg is a registry for skills — reusable instruction sets that extend what
your AI coding assistant can do. Think of it like npm or crates.io, but for
prompts you share across projects and teams.

Browse and install community skills from [skreg.ai](https://skreg.ai). Package
your own with a single command and publish them for others to use — or run a
private registry your team controls.

## Install

```bash
cargo install skreg-cli
```

Requires Rust ([rustup.rs](https://rustup.rs)). After installing, verify with:

```bash
skreg --version
```

## Using skills

### Finding a skill

Browse and search for skills at [skreg.ai](https://skreg.ai).

> **Coming soon:** `skreg search <query>` — find skills directly from the CLI.

### Installing a skill

```bash
skreg install <namespace>/<name>
```

For example:

```bash
skreg install dymocaptin/color-analysis
```

Skills are installed to `~/.skreg/packages/` and are available to your AI
coding assistant automatically.

### Using an installed skill

> **Coming soon:** Native Claude Code integration — skills installed via skreg
> will be available directly through the Claude Code plugin marketplace.

## Building skills

### Anatomy of a skill

A skill is a directory with two files:

```
my-skill/
├── SKILL.md        # The skill prompt and instructions
└── manifest.json   # Metadata
```

`SKILL.md` starts with a YAML frontmatter block followed by the instructions
your AI assistant will follow when the skill is active:

```markdown
---
name: color-analysis
description: Analyzes the dominant colors in any image file, producing a ranked
             color palette with hex codes, RGB/HSL values, human-readable color
             names, and percentage breakdowns.
---

# Color Analysis

You are an expert color analyst. When the user provides an image, analyze it
and produce a detailed color palette report.
...
```

`manifest.json` declares the package identity:

```json
{
  "namespace": "dymocaptin",
  "name": "color-analysis",
  "version": "1.0.0",
  "description": "Analyzes the dominant colors in any image file, producing a ranked color palette with hex codes, RGB/HSL values, human-readable color names, and percentage breakdowns."
}
```

### Packaging

From inside your skill directory, run:

```bash
skreg pack
```

This produces a `<name>-<version>.skill` archive ready to publish.

### Publishing

First, log in to your registry:

```bash
skreg login <your-namespace>
```

Then publish:

```bash
skreg publish
```

skreg will pack, upload, and vet your skill. Once it passes review it appears
on [skreg.ai](https://skreg.ai).

## Self-hosting

You can run your own skreg registry. Deploy the infrastructure with Pulumi,
then point the CLI at your domain:

```bash
skreg login <your-namespace> --registry https://registry.example.com
```

See [infra/README.md](infra/README.md) for the full deployment guide.

## Contributing

skreg is open source. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build
from source, run tests, and submit changes.

## License

Apache 2.0 — see [LICENSE](LICENSE).
