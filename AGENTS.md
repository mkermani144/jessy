When updating `config/profile.example.yaml`, always apply the same config-key changes to `config/profile.yaml` in the same task.
When changing domain/data models, propagate updates everywhere in the same task: storage/schema, adapters, use-cases, reports/UI, config/docs, and tests.

## Hexarch Rules
- Layers:
  - `domain`: models + policies + service logic.
  - `ports`: trait interfaces owned by domain/core.
  - `adapters`: concrete implementations of ports.
  - `main/composition`: wiring only.
- Dependency direction:
  - domain -> ports only.
  - adapters -> ports (+ infra crates).
  - composition -> domain + adapters.
  - never import adapters into domain.
- Adapter placement:
  - adapters may live in the main crate.
  - step crates may stay domain/core-only if preferred.
  - keep business logic out of adapters regardless of location.
