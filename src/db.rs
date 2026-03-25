//! Database loading for proc-macro expansion time.
//! Boots an in-memory CozoDB with samskara-core + sema-core + sema + noesis-schema,
//! then queries it for domain variants, field types, and RPC interfaces.

use std::sync::OnceLock;

static DB: OnceLock<Result<criome_cozo::CriomeDb, String>> = OnceLock::new();

/// Schema files embedded at compile time.
/// Uses CARGO_MANIFEST_DIR so paths resolve correctly both standalone and as a workspace member.
const SAMSKARA_WORLD_INIT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/flake-crates/samskara/schema/samskara-world-init.cozo"));
const SAMSKARA_WORLD_SEED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/flake-crates/samskara/schema/samskara-world-seed.cozo"));
const NOESIS_WORLD_INIT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/flake-crates/noesis-schema/noesis-world-init.cozo"));
const NOESIS_WORLD_SEED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/flake-crates/noesis-schema/noesis-world-seed.cozo"));
const NOESIS_FIELD_TYPE_SEED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/flake-crates/noesis-schema/noesis-field-type-seed.cozo"));

pub struct FieldInfo {
    pub name: String,
    pub kind: String,
    pub target_domain: String,
}

pub struct MethodInfo {
    pub name: String,
    pub description: String,
}

/// Load or return the cached in-memory CozoDB with all schemas.
pub fn load_db() -> Result<&'static criome_cozo::CriomeDb, String> {
    let result = DB.get_or_init(|| init_db().map_err(|e| e.to_string()));
    match result {
        Ok(db) => Ok(db),
        Err(e) => Err(format!("DB init failed: {e}")),
    }
}

fn init_db() -> Result<criome_cozo::CriomeDb, Box<dyn std::error::Error>> {
    let db = criome_cozo::CriomeDb::open_memory()?;

    // Load samskara-core boot schema (Phase, Dignity, Domain, etc.)
    for stmt in criome_cozo::Script::from_str(samskara_core::boot::CORE_WORLD_INIT) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("core init: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(samskara_core::boot::CORE_WORLD_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("core seed: {e}"))?;
        }
    }

    // Load samskara world schema (Element, Measure, Sign, etc.)
    for stmt in criome_cozo::Script::from_str(SAMSKARA_WORLD_INIT) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("samskara init: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(SAMSKARA_WORLD_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("samskara seed: {e}"))?;
        }
    }

    // Load sema-core (generators, astrological domains, structure, Name)
    for stmt in criome_cozo::Script::from_str(sema_core::INIT) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema-core init: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(sema_core::SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema-core seed: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(sema_core::FIELD_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema-core field: {e}"))?;
        }
    }

    // Load sema language layer (programming logic, protocol, testing)
    for stmt in criome_cozo::Script::from_str(sema::INIT) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema init: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(sema::SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema seed: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(sema::FIELD_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("sema field: {e}"))?;
        }
    }

    // Load noesis schema (field_type, translation, new domains, etc.)
    for stmt in criome_cozo::Script::from_str(NOESIS_WORLD_INIT) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("noesis init: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(NOESIS_WORLD_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("noesis seed: {e}"))?;
        }
    }
    for stmt in criome_cozo::Script::from_str(NOESIS_FIELD_TYPE_SEED) {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() && !is_comment_only(trimmed) {
            db.run_script(trimmed).map_err(|e| format!("field_type seed: {e}"))?;
        }
    }

    Ok(db)
}

/// Query all variants (key values) of a domain.
pub fn query_domain_variants(db: &criome_cozo::CriomeDb, domain: &str) -> Result<Vec<String>, String> {
    let query = format!("?[name] := *{domain}{{name}} :order name");
    let result = db.run_script(&query).map_err(|e| format!("query domain {domain}: {e}"))?;

    let rows = result.get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("domain {domain}: missing rows"))?;

    let variants: Vec<String> = rows.iter()
        .filter_map(|row| {
            row.as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
                .map(String::from)
        })
        .collect();

    Ok(variants)
}

/// Query field_type entries for a relation (for product! macro).
pub fn query_product_fields(db: &criome_cozo::CriomeDb, relation: &str) -> Result<Vec<FieldInfo>, String> {
    // First get columns from ::columns introspection
    let col_query = format!("::columns {relation}");
    let col_result = db.run_script(&col_query).map_err(|e| format!("columns {relation}: {e}"))?;
    let col_rows = col_result.get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("product {relation}: missing column rows"))?;

    let mut fields = Vec::new();
    for row in col_rows {
        let arr = row.as_array().ok_or("column row not array")?;
        let col_name = arr.first()
            .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
            .ok_or("missing column name")?
            .to_string();

        // Look up field_type for this (relation, column)
        let ft_query = format!(
            "?[kind, target_domain] := *field_type{{relation: \"{relation}\", column: \"{col_name}\", kind, target_domain, unit_domain, description, phase, dignity}}"
        );
        let ft_result = db.run_script(&ft_query);

        let (kind, target_domain) = match ft_result {
            Ok(ref v) => {
                let ft_rows = v.get("rows").and_then(|r| r.as_array());
                match ft_rows.and_then(|r| r.first()).and_then(|r| r.as_array()) {
                    Some(ft_arr) => {
                        let k = ft_arr.first()
                            .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
                            .unwrap_or("data")
                            .to_string();
                        let td = ft_arr.get(1)
                            .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
                            .unwrap_or("")
                            .to_string();
                        (k, td)
                    }
                    None => ("data".to_string(), String::new()),
                }
            }
            Err(_) => ("data".to_string(), String::new()),
        };

        fields.push(FieldInfo {
            name: col_name,
            kind,
            target_domain,
        });
    }

    Ok(fields)
}

/// Query RPC methods for an interface (for morphism! macro).
pub fn query_rpc_methods(db: &criome_cozo::CriomeDb, interface: &str) -> Result<Vec<MethodInfo>, String> {
    let query = format!(
        "?[method, description] := *rpc_method{{interface: \"{interface}\", method, description, phase, dignity}} :order method"
    );
    let result = db.run_script(&query).map_err(|e| format!("query rpc_method {interface}: {e}"))?;
    let rows = result.get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("morphism {interface}: missing rows"))?;

    let methods: Vec<MethodInfo> = rows.iter()
        .filter_map(|row| {
            let arr = row.as_array()?;
            let name = arr.first()
                .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))?
                .to_string();
            let desc = arr.get(1)
                .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
                .unwrap_or("")
                .to_string();
            Some(MethodInfo { name, description: desc })
        })
        .collect();

    Ok(methods)
}

/// Query all single-key domain names from the Domain registry.
/// These are the domains that become capnp enums (single String key).
pub fn query_all_enum_domains(db: &criome_cozo::CriomeDb) -> Result<Vec<String>, String> {
    // Get all domain names
    let result = db.run_script("?[name] := *Domain{name} :order name")
        .map_err(|e| format!("query Domain: {e}"))?;
    let rows = result.get("rows")
        .and_then(|v| v.as_array())
        .ok_or("Domain: missing rows")?;

    let mut enum_domains = Vec::new();
    for row in rows {
        let name = row.as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str().or_else(|| v.get("Str").and_then(|s| s.as_str())))
            .unwrap_or_default()
            .to_string();

        if name.is_empty() { continue; }

        // Check it's a single-key domain by querying ::columns
        let col_query = format!("::columns {name}");
        let col_result = match db.run_script(&col_query) {
            Ok(r) => r,
            Err(_) => continue, // relation doesn't exist — skip
        };
        let col_rows = col_result.get("rows").and_then(|v| v.as_array());
        if let Some(cols) = col_rows {
            let key_count = cols.iter().filter(|c| {
                c.as_array()
                    .and_then(|arr| arr.get(1))
                    .and_then(|v| v.as_bool().or_else(|| v.get("Bool").and_then(|b| b.as_bool())))
                    .unwrap_or(false)
            }).count();
            if key_count == 1 {
                // Check it has at least one variant
                let variant_query = format!("?[name] := *{name}{{name}} :limit 1");
                if let Ok(vr) = db.run_script(&variant_query) {
                    if vr.get("rows").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false) {
                        enum_domains.push(name);
                    }
                }
            }
        }
    }

    Ok(enum_domains)
}

fn is_comment_only(s: &str) -> bool {
    s.lines().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//")
    })
}
