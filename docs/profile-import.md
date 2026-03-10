# Profile Import

The importer parses the source profile, resolves referenced assets relative to the source location, copies managed assets into app storage, rewrites internal paths, and persists a normalized profile plus an import report.

Supported inline blocks:
- `<ca>`
- `<cert>`
- `<key>`
- `<tls-auth>`
- `<tls-crypt>`

The original imported profile is preserved read-only in the managed profile directory for diagnostics.

