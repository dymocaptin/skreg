# skreg

A package registry for AI coding assistant skills, built on cryptographic publisher identity and trust.

## What is skreg?

skreg is a registry for skills — reusable instruction sets that extend what
your AI coding assistant can do. Think of it like npm or crates.io, but for
prompts you share across projects and teams.

Browse and install community skills from [skreg.ai](https://skreg.ai). Package
your own with a single command and publish them for others to use — or run a
private registry your team controls.

## Trust & Verification

Every package published to skreg carries a cryptographic publisher signature. Two verification tiers are supported:

| Tier | Badge | How |
|------|-------|-----|
| **Self-signed** | `◈ self-signed` | Publisher generated their own key. Key consistency is enforced at the namespace level — once a key is used it cannot be swapped without an explicit rotation. |
| **CA-verified** | `✦ verified` | Publisher obtained a CA-issued cert from the skreg Publisher CA. Proves organisation identity. |

All packages — regardless of tier — pass content, safety, and structure vetting before appearing in search. The registry does not counter-sign packages; only the publisher's key appears in the signature.

Key material is stored in `~/.skreg/keys/` with `chmod 700`. Run `skreg certify` to obtain a CA-verified cert. Run `skreg rotate` to safely rotate your namespace's signing key.

## Install

### Pre-built binary (macOS and Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/dymocaptin/skreg/releases/download/v0.1.2/skreg-cli-installer.sh | sh
```

### Homebrew (macOS and Linux)

```bash
brew tap dymocaptin/tap
brew install skreg-cli
```

### npm

```bash
npm install -g skreg-cli
```

### cargo (build from source)

Requires Rust ([rustup.rs](https://rustup.rs)).

```bash
git clone https://github.com/dymocaptin/skreg.git
cargo install --path skreg/crates/skreg-cli
```

After installing with any method, verify with:

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
