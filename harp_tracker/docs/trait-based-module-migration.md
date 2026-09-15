# Trait-Based Module Migration Design

## Overview

The current backend is organized around explicitly named module collections and a fixed enum-based tracking model. In practice, this means the tracker logic is built around concrete modules such as APRS, Iridium, SondeHub, and WSPR rather than around a shared module contract.

This creates a maintenance problem:

- each new module requires a new field, new factory function, and new update path
- the tracker update flow is duplicated and manually assembled
- the UI has to know about each module type separately
- module status and data flow are not centralized

The migration goal is to replace the rigid module-specific structure with a shared trait-based model while preserving the existing runtime behavior and data pipeline.

This document describes the backend changes needed for that migration and the exact rollout plan.

---

## Current state that must be migrated

The existing tracker model in the backend is centered around concrete module-specific storage:

- `Tracker` owns dedicated vectors such as `aprs`, `iridium`, `sondehub`, and `wspr`
- each module type has a separate constructor function
- each module type has a dedicated return function
- `update_tracker()` manually chains the update calls by type
- `TrackingType` is an enum used for CSV output and event labeling

This works for a small number of built-in sources, but it makes the system hard to extend and difficult to reflect cleanly in the UI.

---

## Target architecture

The target backend design is a registry-driven, trait-based module system.

### Core principles

1. Modules all implement a shared Rust trait.
2. The tracker does not own separate per-module storage forever.
3. All modules produce or consume a common telemetry event model.
4. A central registry handles registration, lookup, and update flow.
5. The UI reads module state from descriptors instead of custom code paths.

---

## Shared backend types

### Telemetry event model

A common event object should be used for normalized position data regardless of source type.

```rust
#[derive(Clone, Debug)]
pub struct TelemetryEvent {
    pub source_id: String,
    pub timestamp: u64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f64>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub raw: serde_json::Value,
}
```

This event is the canonical backend representation of a module update.

### Module descriptor

Every module should expose metadata for the UI and backend services.

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub last_update: Option<u64>,
    pub module_type: String,
}
```

### Module status

Status should be runtime state, not module-specific code.

```rust
#[derive(Clone, Debug)]
pub struct ModuleStatus {
    pub enabled: bool,
    pub connected: bool,
    pub last_update: Option<u64>,
    pub error: Option<String>,
}
```

---

## Trait design

Each module must implement a shared contract.

```rust
pub trait Module: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn supports_source(&self, source: &str) -> bool;
    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String>;
    fn status(&self) -> ModuleStatus;
    fn descriptor(&self) -> ModuleDescriptor;
}
```

### Why this trait is enough

This contract keeps the logic minimal while still allowing each module to do custom work internally.

- built-in modules keep their own logic
- each module exposes metadata
- the registry can handle lifecycle state generically
- the tracker no longer needs a large switch over module types

---

## Module registry

A registry centralizes all module operations.

```rust
pub struct ModuleRegistry {
    modules: std::collections::HashMap<String, Box<dyn Module>>,
}
```

### Registry responsibilities

- register modules
- list active modules
- resolve by source id or module id
- ingest telemetry events through the shared route
- return static module metadata for the UI

```rust
impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: std::collections::HashMap::new(),
        }
    }

    pub fn register<M: Module + 'static>(&mut self, module: M) {
        let id = module.id().to_string();
        self.modules.insert(id, Box::new(module));
    }

    pub fn list(&self) -> Vec<ModuleDescriptor> {
        self.modules.values().map(|m| m.descriptor()).collect()
    }

    pub fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        let target = self
            .modules
            .values_mut()
            .find(|m| m.supports_source(&event.source_id));

        match target {
            Some(module) => module.ingest(event),
            None => Err(format!("No module registered for source '{}'", event.source_id)),
        }
    }
}
```

---

## Migration of current built-in modules

The current concrete modules must be wrapped behind the new trait contract without breaking their behavior.

### Existing modules to migrate

- APRS
- Iridium
- SondeHub
- WSPR

Each module should be converted into a struct that owns its current internal configuration and exposes the standard `Module` trait.

### Migration pattern

For each current module:

1. keep the internal logic and config fields
2. add `id()` and `name()`
3. add `supports_source()` based on the module’s source name or alias
4. implement `ingest()` by translating the normalized event into the module-specific update call
5. implement `status()` using the existing active/connected/error state
6. implement `descriptor()` for UI consumption

Example pattern:

```rust
pub struct AprsModule {
    id: String,
    api_key: String,
    call_sign: String,
    enabled: bool,
    last_update: Option<u64>,
    last_error: Option<String>,
}

impl Module for AprsModule {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "APRS" }

    fn supports_source(&self, source: &str) -> bool {
        source.eq_ignore_ascii_case("aprs") || source.eq_ignore_ascii_case(&self.call_sign)
    }

    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        if let (Some(lat), Some(lon), Some(alt)) = (event.lat, event.lon, event.alt) {
            self.last_update = Some(event.timestamp);
            self.enabled = true;
            self.last_error = None;
            println!("APRS ingest: lat={}, lon={}, alt={}", lat, lon, alt);
            Ok(())
        } else {
            Err("APRS payload missing lat/lon/alt".into())
        }
    }

    fn status(&self) -> ModuleStatus {
        ModuleStatus {
            enabled: self.enabled,
            connected: true,
            last_update: self.last_update,
            error: self.last_error.clone(),
        }
    }

    fn descriptor(&self) -> ModuleDescriptor {
        ModuleDescriptor {
            id: self.id.clone(),
            name: self.name().to_string(),
            enabled: self.enabled,
            connected: true,
            last_update: self.last_update,
            module_type: "aprs".to_string(),
        }
    }
}
```

---

## Migration of the tracker core

The current `Tracker` struct is the main migration target.

### Current problem

The tracker stores each module type in independent collections and manually updates them through separate functions. This tightly couples the tracker to module implementations and prevents consistent registry-based behavior.

### New tracker role

The tracker should become a runtime coordinator rather than the owner of hard-coded module buckets.

New shape:

```rust
pub struct Tracker {
    registry: ModuleRegistry,
    position_time: PositionTime,
    csv_path: Option<PathBuf>,
}
```

### New responsibilities

- register built-in modules
- process telemetry events from the shared event model
- write serialized output to CSV using the common event metadata
- expose module descriptors to the UI

### Registry-driven update flow

The tracker delegates updates to every registered module through the registry. The
runtime controller no longer contains one update function per module type:

```rust
impl Tracker {
    pub fn process_event(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        self.registry.ingest(event)
    }

    pub fn module_descriptors(&self) -> Vec<ModuleDescriptor> {
        self.registry.list()
    }
}
```

This removes type-specific update logic from the runtime controller.

---

## Migration of the CSV pipeline

The CSV pipeline currently writes a `TrackingType` enum into the file.

This should remain semantically valid, but it should become a value derived from the module identity rather than a rigid enum set.

### Existing behavior

`write_to_csv()` writes rows like:

- `track_type`
- lat
- lon
- alt
- horiz_vel
- vert_vel
- time

### Proposed behavior

The CSV row should still write the module name or source id as the track label, but it should not depend on hard-coded enum variants for every module type.

```rust
pub fn write_to_csv(
    source_id: &str,
    pos_time: &PositionTime,
    csv_path: Option<PathBuf>,
) -> io::Result<()> {
    let mut file = OpenOptions::new().append(true).open(csv_path.unwrap())?;
    writeln!(
        file,
        "{},{:.6},{:.6},{:.2},{:.2},{:.2},{}",
        source_id,
        pos_time.lat,
        pos_time.lon,
        pos_time.alt,
        pos_time.horiz_vel,
        pos_time.vert_vel,
        pos_time.last_update
    )?;
    Ok(())
}
```

This keeps compatibility with the data recording process while removing the enum bottleneck.

---

## Migration of status and UI integration

The UI currently relies on explicit module knowledge.

### Old model

The frontend or app logic checks for specific module groups such as:

- APRS active
- Iridium active
- SondeHub active
- WSPR active

### New model

The UI should query a generic backend endpoint that returns module descriptors.

Example response:

```json
[
  {
    "id": "aprs",
    "name": "APRS",
    "enabled": true,
    "connected": true,
    "last_update": 1726250000,
    "module_type": "aprs"
  },
  {
    "id": "sondehub",
    "name": "SondeHub",
    "enabled": true,
    "connected": false,
    "last_update": null,
    "module_type": "sondehub"
  }
]
```

The UI then renders a module card for each entry instead of checking for static module types.

---

## Migration phases

### Phase 1: Introduce shared backend types

Add the following without changing existing behavior:

- `TelemetryEvent`
- `ModuleStatus`
- `ModuleDescriptor`
- `Module` trait
- `ModuleRegistry`

At this point, the existing code still works.

### Phase 2: Convert existing modules

Convert each built-in module into a direct implementation of the shared trait while keeping its internal telemetry and API logic intact.

### Phase 3: Replace tracker hard-coded runner logic

Remove:

- `return_aprs()`
- `return_iridium()`
- `return_sondehub()`
- `return_wspr()`
 - the module-specific update helpers
 - the legacy per-module storage and accessors

Replace them with generic registry methods.

### Phase 4: Add descriptor-based UI contract

Expose module metadata via a single API response used by the UI.

### Phase 5: Remove legacy enum-driven assumptions

Remove the direct dependence on `TrackingType` for runtime registration and UI classification where possible, while preserving a stable CSV value when needed.

### Phase 6: Final cleanup

- remove dead constructors that only existed for the old structure
- remove duplicated update code
- validate the remaining code paths with targeted integration checks

---

## Compatibility strategy

The migration should be staged to avoid a risky one-time rewrite.

### Recommended compatibility approach

1. Add the trait and registry without removing old storage paths.
2. Instantiate modules through both old and new code paths temporarily.
3. Route updates through the registry while keeping the existing module behavior intact.
4. Replace the UI calls that depend on old module-specific functions.
5. Remove old-specific code only after tests confirm parity.

### Compatibility rules

- no behavior change for existing modules during the first rollout
- no breaking changes to the output data format unless necessary
- no removal of old fields until a final cleanup phase
- each module must report a stable `ModuleDescriptor`

---

## Migration risks

### Risk 1: duplicate update paths

If both the old module-specific functions and the new registry paths run, duplicate updates may occur. This must be prevented by a single routing point.

### Risk 2: stale state across modules

Modules may keep state that is not represented in the new shared event model. The migration preserves that internal module logic while exposing it through the shared trait.

### Risk 3: incorrect source matching

The registry must not assume every module corresponds to a single name. Matching should support both a stable module id and a source alias.

### Risk 4: UI assumptions break

The UI must be migrated at the same time as the backend structure; otherwise it will still assume old module lists and old module-specific status.

---

## Validation checklist

The migration is complete when all of the following are true:

- all built-in modules implement the shared `Module` trait
- all modules are registered in the central registry
- the tracker updates are driven by the registry instead of hard-coded module arrays
- the UI receives module descriptors from a generic endpoint
- CSV writes continue in the expected format
- status and active state are tracked generically
- the system can add a new module by implementation plus registration without modifying core update orchestration

---

## Acceptance criteria

The migration is successful when:

1. adding a new module requires only module implementation and registration
2. the tracker core does not need explicit logic for each module type
3. module descriptors are available for UI rendering
4. existing modules still behave the same as before the migration
5. the backend is ready for future module additions without a large rewrite

---

## Summary

The trait-based migration is a structural cleanup and modernization of the tracker backend.

It replaces the hard-coded module collections and per-module runner logic with a shared module contract, a central registry, and a normalized telemetry event model. This keeps the functionality of the current backend while making future additions far simpler and allowing the UI to reflect module state generically.

This design is intentionally migration-focused: it preserves current behavior while introducing the new abstraction layer in stages, so the project can move to the trait-based model without destabilizing the application.
