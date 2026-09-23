use std::collections::HashMap;

use serde_json::Value;
use serde::Serialize;

use crate::track_lib::position_time::PositionTime;

///Represents a telemetry event from a module
#[derive(Clone, Debug)]
pub struct TelemetryEvent {
    pub source_id: String,
    pub timestamp: u64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f64>,
    pub metadata: HashMap<String, Value>,
    pub raw: Value,
}
///Represents a module descriptor, used to identify and describe a module
#[derive(Clone, Debug, serde::Serialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub last_update: Option<u64>,
    pub module_type: String,
}
/// Represents the status of a module
#[derive(Clone, Debug, Default)]
pub struct ModuleStatus {
    pub enabled: bool,
    pub connected: bool,
    pub last_update: Option<u64>,
    pub error: Option<String>,
}
/// Represents a field in a module's definition
#[derive(Clone, Debug, Serialize)]
pub struct ModuleField {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub secret: bool,
    pub placeholder: Option<String>,
}
/// Represents a module definition
#[derive(Clone, Debug, Serialize)]
pub struct ModuleDefinition {
    pub module_type: String,
    pub display_name: String,
    pub description: String,
    pub fields: Vec<ModuleField>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModuleSnapshot {
    pub id: String,
    pub module_type: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub last_update: Option<u64>,
}
///represents registration for a module
pub struct ModuleRegistration {
    pub definition: fn() -> ModuleDefinition,
    pub create: fn(String, Value) -> Result<Box<dyn Module>, String>,
}

inventory::collect!(ModuleRegistration);

/// Registers a module with the tracker
pub trait Module: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn module_type(&self) -> &str;
    fn supports_source(&self, source: &str) -> bool {
        self.id().eq_ignore_ascii_case(source)
    }
    fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String>;
    fn update(&mut self) -> Result<(), String>;
    fn position(&self) -> Option<PositionTime>;
    fn status(&self) -> ModuleStatus;
    fn set_status(&mut self, status: ModuleStatus);
    fn descriptor(&self) -> ModuleDescriptor;
}
/// Hashmap of modules by their ID
#[derive(Default)]
pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn Module>>,
}
/// Registry for tracking modules, has methods for registering and managing modules
impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn catalog(&self) -> Vec<ModuleDefinition> {
        inventory::iter::<ModuleRegistration>
            .into_iter()
            .map(|registration| (registration.definition)())
            .collect()
    }

    pub fn create_module(
        &mut self,
        module_type: &str,
        id: String,
        config: Value,
    ) -> Result<(), String> {
        let registration = inventory::iter::<ModuleRegistration>
            .into_iter()
            .find(|registration| (registration.definition)().module_type == module_type)
            .ok_or_else(|| format!("Unknown module type '{module_type}'"))?;

        let module = (registration.create)(id, config)?;
        self.register_boxed(module);
        Ok(())
    }

    fn register_boxed(&mut self, module: Box<dyn Module>) {
        let id = module.id().to_string();
        self.modules.insert(id, module);
    }

    pub fn register<M: Module + 'static>(&mut self, module: M) {
        let id = module.id().to_string();
        self.modules.insert(id, Box::new(module));
    }

    pub fn list(&self) -> Vec<ModuleDescriptor> {
        self.modules.values().map(|module| module.descriptor()).collect()
    }

    pub fn has_module(&self, id: &str) -> bool {
        self.modules.contains_key(id)
    }

    /// Case-insensitive lookup of a registered module id by name.
    pub fn find_module_id(&self, id: &str) -> Option<String> {
        self.modules
            .keys()
            .find(|key| key.eq_ignore_ascii_case(id))
            .cloned()
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.modules.remove(id).is_some()
    }

    pub fn ingest(&mut self, event: &TelemetryEvent) -> Result<(), String> {
        let Some(module) = self
            .modules
            .values_mut()
            .find(|module| module.supports_source(&event.source_id))
        else {
            return Err(format!("No module registered for source '{}'", event.source_id));
        };

        module.ingest(event)
    }

    pub fn update_all(&mut self) -> Vec<Result<(), String>> {
        self.modules.values_mut().map(|module| module.update()).collect()
    }

    pub fn positions(&self) -> Vec<(PositionTime, String)> {
        self.modules
            .values()
            .filter_map(|module| module.position().map(|position| (position, module.module_type().to_string())))
            .collect()
    }

    /// Positions of connected modules keyed by module id, for lookups by name.
    pub fn positions_by_id(&self) -> Vec<(String, PositionTime)> {
        self.modules
            .values()
            .filter_map(|module| module.position().map(|position| (module.id().to_string(), position)))
            .collect()
    }

    pub fn snapshots(&self) -> Vec<ModuleSnapshot> {
        self.modules.values().map(|module| {
            let status = module.status();
            ModuleSnapshot {
                id: module.id().to_string(),
                module_type: module.module_type().to_string(),
                name: module.name().to_string(),
                enabled: status.enabled,
                connected: status.connected,
                last_update: status.last_update,
            }
        }).collect()
    }

    pub fn update_status_for_source(&mut self, source: &str, status: ModuleStatus) {
        if let Some(module) = self
            .modules
            .values_mut()
            .find(|module| module.supports_source(source))
        {
            module.set_status(status);
        }
    }
}
