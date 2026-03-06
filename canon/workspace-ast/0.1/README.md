# LogLine Workspace Canon AST 0.1

This folder is the canonical source for workspace manifest schema generation.

## Canon files

- `workspace-ast.json`: formal AST for workspace manifests and plugins.
- `workspace.manifest.schema.json`: permissive validator.
- `workspace.manifest.strict.schema.json`: strict validator (includes strict-only conditional rules).
- `inputs.linear.schema.json`: standalone schema for the Linear input plugin.

## Strict rule encoded

- If `inputs.primary == "linear"`, `inputs.linear` must be present.

## Current consumer

- `code247.logline.world` mirrors these files under `code247.logline.world/schemas/`.
