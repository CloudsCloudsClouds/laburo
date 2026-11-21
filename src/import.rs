//! One-way import of the hand-maintained `casos.yaml` dictionary.
//!
//! The YAML is read through [`serde_yaml_ng::Value`] instead of `serde` structs
//! so the import tolerates missing and extra fields; the dictionary evolves
//! faster than this crate. Existing cases are left untouched, which makes
//! re-running the import safe.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_yaml_ng::{Mapping, Value};

use crate::model::StepKind;
use crate::repo;

/// Import every test case found in `path` and return how many were created.
///
/// Each YAML `prueba` becomes a `test_case` with manual steps, linked to the
/// requirement built from its enclosing `caso`. Test cases whose id already
/// exists are skipped, so duplicate imports are idempotent.
pub fn import_casos(conn: &Connection, path: &Path) -> Result<usize> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading casos YAML {}", path.display()))?;
    let document: Value = serde_yaml_ng::from_str(&raw)
        .with_context(|| format!("parsing casos YAML {}", path.display()))?;
    let root = document
        .as_mapping()
        .context("casos YAML root must be a mapping")?;

    let mut imported = 0;

    if let Some(modules) = sequence(root, "modulos") {
        for module_value in modules {
            let Some(module) = module_value.as_mapping() else {
                continue;
            };
            let module_id = first_text(module, &["id"]);
            let module_name = first_text(module, &["nombre", "name"]);
            let label = module_label(&module_id, &module_name);

            // A module may list pruebas directly, without use cases.
            if let Some(pruebas) = sequence(module, "pruebas") {
                imported += import_pruebas(conn, &label, &module_id, None, pruebas)?;
            }
            if let Some(casos) = sequence(module, "casos") {
                imported += import_use_cases(conn, &label, &module_id, casos)?;
            }
        }
    }

    // Flat shape: top-level casos, each shaped like a module's casos entry.
    if let Some(casos) = sequence(root, "casos") {
        imported += import_use_cases(conn, "", "", casos)?;
    }

    Ok(imported)
}

fn import_use_cases(
    conn: &Connection,
    module_label: &str,
    module_id: &str,
    casos: &[Value],
) -> Result<usize> {
    let mut imported = 0;

    for (index, caso_value) in casos.iter().enumerate() {
        let Some(caso) = caso_value.as_mapping() else {
            continue;
        };

        // Flat entries may carry their module inline; fall back to the
        // enclosing module when they do not.
        let explicit_module = first_text(caso, &["modulo", "module"]);
        let (label, id) = if explicit_module.is_empty() {
            (module_label.to_string(), module_id.to_string())
        } else {
            (explicit_module.clone(), explicit_module)
        };

        let requirement_id = requirement_id(caso, &id, index);
        let mut title = first_text(caso, &["nombre", "title"]);
        if title.is_empty() {
            title.clone_from(&requirement_id);
        }
        let acceptance = first_text(caso, &["regla", "acceptance"]);
        repo::create_requirement(
            conn,
            &requirement_id,
            &title,
            "casos.yaml",
            &acceptance,
            "functional",
        )?;

        let pruebas = sequence(caso, "pruebas").unwrap_or_default();
        imported += import_pruebas(conn, &label, &id, Some(&requirement_id), pruebas)?;
    }

    Ok(imported)
}

fn import_pruebas(
    conn: &Connection,
    module_label: &str,
    module_id: &str,
    requirement_id: Option<&str>,
    pruebas: &[Value],
) -> Result<usize> {
    let mut imported = 0;

    for (index, prueba_value) in pruebas.iter().enumerate() {
        let Some(prueba) = prueba_value.as_mapping() else {
            continue;
        };
        let case_id = test_case_id(prueba, requirement_id, module_id, index);

        if !case_exists(conn, &case_id)? {
            insert_case(conn, &case_id, module_label, prueba)?;
            imported += 1;
        }

        if let Some(requirement_id) = requirement_id {
            repo::link_case_requirement(conn, &case_id, requirement_id)?;
        }
    }

    Ok(imported)
}

fn insert_case(
    conn: &Connection,
    case_id: &str,
    module_label: &str,
    prueba: &Mapping,
) -> Result<()> {
    let mut title = first_text(prueba, &["titulo", "title"]);
    if title.is_empty() {
        title = case_id.to_string();
    }
    let description = first_text(prueba, &["objetivo", "description"]);
    let priority = canonical_priority(&first_text(prueba, &["prioridad", "priority"]));
    let tags = tags_for(prueba);

    repo::create_case(
        conn,
        case_id,
        &title,
        &description,
        module_label,
        &priority,
        "local",
        "",
        &tags,
    )?;
    write_steps(conn, case_id, &parsed_steps(prueba))?;
    Ok(())
}

fn case_exists(conn: &Connection, case_id: &str) -> Result<bool> {
    let count = conn.query_row(
        "SELECT count(*) FROM test_case WHERE id=?1",
        [case_id],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(count > 0)
}

fn tags_for(prueba: &Mapping) -> Vec<String> {
    let level = canonical_tag(&first_text(prueba, &["nivel", "level"]));
    let kind = canonical_tag(&first_text(prueba, &["tipo", "type"]));
    let mut tags: Vec<String> = Vec::new();
    for tag in [level.as_str(), kind.as_str(), "imported"] {
        let tag = tag.trim();
        if !tag.is_empty() && !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn canonical_priority(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "alta" | "high" => "high".to_string(),
        "media" | "medium" => "medium".to_string(),
        "baja" | "low" => "low".to_string(),
        "crítica" | "critica" | "critical" => "critical".to_string(),
        "" => "medium".to_string(),
        _ => value,
    }
}

fn canonical_tag(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "funcional" | "functional" => "functional".to_string(),
        "no funcional" | "no-funcional" | "non functional" | "non-functional" => {
            "non-functional".to_string()
        }
        "negativa" | "negative" => "negative".to_string(),
        "regresión" | "regresion" | "regression" => "regression".to_string(),
        "configuración" | "configuracion" | "configuration" => "configuration".to_string(),
        "unitaria" | "unit" => "unit".to_string(),
        "integración" | "integracion" | "integration" => "integration".to_string(),
        "sistema" | "system" => "system".to_string(),
        "" => String::new(),
        _ => value,
    }
}

struct StepData {
    position: Option<i64>,
    action: String,
    data: String,
    expected: String,
}

fn parsed_steps(prueba: &Mapping) -> Vec<StepData> {
    let mut steps = Vec::new();
    for row in sequence(prueba, "pasos").unwrap_or_default() {
        let Some(columns) = row.as_sequence() else {
            continue;
        };
        let position = columns
            .first()
            .and_then(text)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|position| *position > 0);
        let action = columns.get(1).and_then(text).unwrap_or_default();
        let data = columns.get(2).and_then(text).unwrap_or_default();
        let expected = columns.get(3).and_then(text).unwrap_or_default();
        if action.is_empty() && data.is_empty() && expected.is_empty() {
            continue;
        }
        steps.push(StepData {
            position,
            action,
            data,
            expected,
        });
    }
    steps
}

fn write_steps(conn: &Connection, case_id: &str, steps: &[StepData]) -> Result<()> {
    let mut used: HashSet<i64> = HashSet::new();
    for step in steps {
        let next = used.iter().copied().max().unwrap_or(0) + 1;
        let mut position = step
            .position
            .filter(|position| !used.contains(position))
            .unwrap_or(next);
        while used.contains(&position) {
            position += 1;
        }
        used.insert(position);
        repo::add_step(
            conn,
            case_id,
            position,
            &step.action,
            &step.data,
            &step.expected,
            StepKind::Manual,
            None,
        )?;
    }
    Ok(())
}

fn requirement_id(caso: &Mapping, module_id: &str, index: usize) -> String {
    let raw = first_text(caso, &["id"]);
    if raw.is_empty() {
        let module = if module_id.is_empty() { "M" } else { module_id };
        return format!("UC-{module}-{}", index + 1);
    }
    if raw.starts_with("UC-") {
        raw
    } else {
        format!("UC-{raw}")
    }
}

fn test_case_id(
    prueba: &Mapping,
    requirement_id: Option<&str>,
    module_id: &str,
    index: usize,
) -> String {
    let raw = first_text(prueba, &["id"]);
    if !raw.is_empty() {
        return raw;
    }
    match requirement_id.and_then(|id| id.strip_prefix("UC-")) {
        Some(stem) if !stem.is_empty() => format!("TC-{stem}-{}", index + 1),
        _ => {
            let module = if module_id.is_empty() { "M" } else { module_id };
            format!("TC-{module}-{}", index + 1)
        }
    }
}

fn module_label(id: &str, name: &str) -> String {
    match (id.is_empty(), name.is_empty()) {
        (true, true) => String::new(),
        (false, true) => id.to_string(),
        (true, false) => name.to_string(),
        (false, false) => format!("{id} — {name}"),
    }
}

fn sequence<'a>(map: &'a Mapping, key: &str) -> Option<&'a [Value]> {
    map.get(Value::String(key.to_owned()))
        .and_then(Value::as_sequence)
        .map(Vec::as_slice)
}

fn first_text(map: &Mapping, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = map.get(Value::String((*key).to_owned())) {
            if let Some(text) = text(value) {
                return text;
            }
        }
    }
    String::new()
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => {
            let trimmed = raw.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    const FIXTURE: &str = r#"
modulos:
  - id: "M-01"
    nombre: "Gestión web"
    casos:
      - id: "UC-M01-01"
        nombre: "Gestionar contenido"
        actor: "Administrador"
        regla: "El contenido se guarda y se lista."
        prioridad: "Alta"
        estado: "propuesta"
        pruebas:
          - id: "TC-M01-01-1"
            titulo: "El contenido se guarda"
            tipo: "Funcional"
            nivel: "unit"
            ubicacion: "test/models/content_test.rb"
            prioridad: "Alta"
            estado: "propuesta"
            objetivo: "Comprobar que el contenido persiste."
            esperado: "El contenido existe."
            precondiciones:
              - ["Estado inicial", "Base vacía"]
            datos:
              - ["Dato", "Título"]
            pasos:
              - ["1", "Crear el contenido", "Título", "Se crea sin errores"]
              - ["2", "Releer el contenido", "—", "El título coincide"]
            criterio: "El contenido existe con su título."
"#;

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("casos.yaml");
        std::fs::write(&path, FIXTURE).unwrap();
        (dir, path)
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn imports_module_case_and_prueba() {
        let (_dir, path) = fixture();
        let db = Db::open_memory().unwrap();

        let imported = import_casos(&db.conn, &path).unwrap();
        assert_eq!(imported, 1);

        let case = repo::get_case(&db.conn, "TC-M01-01-1").unwrap().unwrap();
        assert_eq!(case.title, "El contenido se guarda");
        assert_eq!(case.description, "Comprobar que el contenido persiste.");
        assert_eq!(case.priority, "high");
        assert_eq!(case.status, "active");
        assert_eq!(case.environment, "local");
        assert_eq!(case.gherkin, "");
        assert!(case.module.contains("M-01"));
        assert!(case.module.contains("Gestión web"));

        assert_eq!(case.tags, vec!["functional", "imported", "unit"]);

        assert_eq!(case.steps.len(), 2);
        assert_eq!(case.steps[0].position, 1);
        assert_eq!(case.steps[0].action, "Crear el contenido");
        assert_eq!(case.steps[0].data, "Título");
        assert_eq!(case.steps[0].expected, "Se crea sin errores");
        assert_eq!(case.steps[0].kind, StepKind::Manual);
        assert_eq!(case.steps[1].position, 2);
        assert_eq!(case.steps[1].action, "Releer el contenido");

        let requirement: (String, String, String, String) = db
            .conn
            .query_row(
                "SELECT title, source, acceptance, kind FROM requirement WHERE id='UC-M01-01'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(requirement.0, "Gestionar contenido");
        assert_eq!(requirement.1, "casos.yaml");
        assert_eq!(requirement.2, "El contenido se guarda y se lista.");
        assert_eq!(requirement.3, "functional");

        let links: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM case_requirement
                 WHERE requirement_id='UC-M01-01' AND case_id='TC-M01-01-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }

    #[test]
    fn duplicate_import_is_idempotent() {
        let (_dir, path) = fixture();
        let db = Db::open_memory().unwrap();

        assert_eq!(import_casos(&db.conn, &path).unwrap(), 1);
        assert_eq!(import_casos(&db.conn, &path).unwrap(), 0);

        assert_eq!(count(&db.conn, "test_case"), 1);
        assert_eq!(count(&db.conn, "test_step"), 2);
        assert_eq!(count(&db.conn, "requirement"), 1);
        assert_eq!(count(&db.conn, "case_requirement"), 1);
    }

    #[test]
    fn imports_flat_casos_and_tolerates_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("casos.yaml");
        std::fs::write(
            &path,
            r#"
casos:
  - id: "UC-M09-01"
    nombre: "Caso plano"
    pruebas:
      - id: "TC-M09-01-1"
        titulo: "Prueba plana"
"#,
        )
        .unwrap();
        let db = Db::open_memory().unwrap();

        assert_eq!(import_casos(&db.conn, &path).unwrap(), 1);

        let case = repo::get_case(&db.conn, "TC-M09-01-1").unwrap().unwrap();
        assert_eq!(case.title, "Prueba plana");
        assert_eq!(case.description, "");
        assert_eq!(case.priority, "medium");
        assert_eq!(case.environment, "local");
        assert!(case.steps.is_empty());
        assert_eq!(case.tags, vec!["imported"]);

        let source: String = db
            .conn
            .query_row(
                "SELECT source FROM requirement WHERE id='UC-M09-01'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "casos.yaml");
    }
}
