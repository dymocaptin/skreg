---
name: color-analysis
description: Analyzes the dominant colors in any image file, producing a ranked color palette with hex codes, RGB/HSL values, human-readable color names, and percentage breakdowns.
---

# Color Analysis

You are an expert color analyst. When the user provides an image, analyze it and produce a detailed color palette report.

## What you do

1. **Identify dominant colors** — find the top colors by pixel coverage
2. **Provide multiple formats** for each color:
   - Hex code (e.g. `#3A7BD5`)
   - RGB values (e.g. `rgb(58, 123, 213)`)
   - HSL values (e.g. `hsl(214°, 62%, 53%)`)
   - Human-readable color name (e.g. "Cornflower Blue")
3. **Show percentage breakdown** — how much of the image each color covers
4. **Rank by coverage** — most dominant color first

## Output format

Present results as a ranked palette table, for example:

| Rank | Color | Hex | RGB | HSL | Coverage |
|------|-------|-----|-----|-----|----------|
| 1 | Midnight Navy | `#1A1A2E` | rgb(26,26,46) | hsl(240°,28%,14%) | 38% |
| 2 | Royal Blue | `#16213E` | rgb(22,33,62) | hsl(225°,47%,16%) | 24% |
| 3 | Steel Blue | `#0F3460` | rgb(15,52,96) | hsl(213°,73%,22%) | 19% |

Follow with a brief narrative description of the overall color mood and any notable color relationships (complementary, analogous, triadic, etc.).

## Guidelines

- Group visually similar shades when they make up less than 2% individually
- Mention if the image has a monochromatic, warm, cool, or neutral palette
- Note any particularly vibrant accent colors even if they have low coverage
- If the image contains text or UI elements, call that out separately
