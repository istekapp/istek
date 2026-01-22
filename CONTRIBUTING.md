# Contributing to Istek

Thank you for your interest in contributing to Istek!

## How to Contribute

### Feature Requests

We do not accept feature implementation PRs at this time. However, we welcome feature suggestions! Please open a [GitHub Discussion](https://github.com/istekapp/istek/discussions) to share your ideas.

### Bug Fixes

Bug fix PRs are always welcome. Before submitting:

1. Check if an issue already exists for the bug
2. If not, open an issue describing the bug
3. Fork the repo and create a branch from `main`
4. Make your fix
5. Ensure the app builds and runs correctly
6. Submit a PR referencing the issue

## Development Setup

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (latest stable)
- [pnpm](https://pnpm.io/)

### Getting Started

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

### Project Structure

```
istek/
├── src-tauri/          # Rust backend (Tauri)
│   └── src/
│       ├── api/        # API endpoints
│       └── lib.rs      # Main entry point
├── components/         # Vue components
├── composables/        # Vue composables
├── pages/              # Nuxt pages
└── types/              # TypeScript types
```

## Questions?

Join the discussion on [GitHub Discussions](https://github.com/istekapp/istek/discussions) or reach out on [Twitter/X](https://x.com/niceydev).
