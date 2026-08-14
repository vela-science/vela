//! Read-only inspection of the four-document native integration waist.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const LIMIT: u64 = 2 * 1024 * 1024;
const MANIFEST: &str = "vela.integration-manifest.v0.1";
const PROFILE: &str = "vela.integration-profile.v0.1";
const BINDING: &str = "vela.integration-binding.v0.1";
const METHOD: &str = "vela.integration-method.v0.1";
const REFERENCE: &str = "vela.exact-reference.v0.1";
const OUTPUTS: &[&str] = &["exact_reference", "submission_draft", "verification_input"];
const RELATIONS: &[&str] = &["exact", "close", "broader", "narrower", "related"];
const DISPOSITIONS: &[&str] = &[
    "preserved",
    "normalized",
    "derived",
    "approximated",
    "omitted",
    "unsupported",
    "assumed",
    "unresolved",
];
const AUTHORITY_KEYS: &[&str] = &[
    "authority",
    "authority_key",
    "decision",
    "event",
    "repository_id",
    "standing",
    "accepted",
    "acceptance",
    "standing_effect",
    "review_status",
    "review_outcome",
    "outcome",
    "result",
    "evidence_availability",
    "build_result",
    "review_result",
    "ci_result",
];

#[derive(Clone, Serialize)]
struct View {
    kind: &'static str,
    id: String,
    path: String,
    root: String,
}

#[derive(Serialize)]
struct Inspection {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    authority_effect: &'static str,
    repository: String,
    revision: String,
    manifest_root: String,
    profiles: Vec<View>,
    bindings: Vec<View>,
    methods: Vec<View>,
    does_not_establish: [&'static str; 4],
}

struct Document {
    path: String,
    value: Value,
    root: String,
}

pub(crate) fn cmd_integration_check(path: &Path, json_out: bool) {
    let inspection = inspect(path)
        .unwrap_or_else(|error| crate::cli::fail_kind_return(crate::ui::ErrorKind::Domain, &error));
    if json_out {
        crate::cli::print_json(&json!({
            "schema": "vela.cli.integration-check.v1",
            "ok": true,
            "command": "integration check",
            "authority_effect": "none",
            "manifest_root": inspection.manifest_root,
            "documents_checked": 1 + inspection.profiles.len() + inspection.bindings.len() + inspection.methods.len(),
            "does_not_establish": inspection.does_not_establish,
        }));
    } else {
        println!("integration contract: ok");
        println!("manifest root: {}", inspection.manifest_root);
        println!(
            "documents: {} Profile(s), {} Binding(s), {} Method(s)",
            inspection.profiles.len(),
            inspection.bindings.len(),
            inspection.methods.len()
        );
        println!("authority effect: none");
    }
}

pub(crate) fn cmd_integration_inspect(path: &Path, json_out: bool) {
    let inspection = inspect(path)
        .unwrap_or_else(|error| crate::cli::fail_kind_return(crate::ui::ErrorKind::Domain, &error));
    if json_out {
        crate::cli::print_json(&inspection);
    } else {
        println!("{} @ {}", inspection.repository, inspection.revision);
        println!("manifest {}", inspection.manifest_root);
        for item in inspection
            .profiles
            .iter()
            .chain(&inspection.bindings)
            .chain(&inspection.methods)
        {
            println!("{} {} {} {}", item.kind, item.id, item.root, item.path);
        }
        println!("authority effect: none");
    }
}

fn inspect(raw_root: &Path) -> Result<Inspection, String> {
    let root = fs::canonicalize(raw_root)
        .map_err(|error| format!("resolve native repository {}: {error}", raw_root.display()))?;
    if !root.is_dir() {
        return Err("native repository root is not a directory".into());
    }
    let manifest = load(&root, "vela.toml", MANIFEST, "manifest_root")?;
    let value = object(&manifest.value, "Manifest")?;
    fields(
        value,
        &[
            "schema",
            "manifest_root",
            "repository",
            "profiles",
            "bindings",
            "methods",
            "rights",
            "availability",
            "outputs",
            "authority_effect",
        ],
        &["limitations", "nonclaims"],
        "Manifest",
    )?;
    fields(
        child(value, "rights", "Manifest")?,
        &["license", "dependencies", "redistribution"],
        &["repository_authored_license"],
        "Manifest rights",
    )?;
    fields(
        child(value, "availability", "Manifest")?,
        &["class", "observed_at", "retention", "access"],
        &[],
        "Manifest availability",
    )?;
    validate_outputs(value.get("outputs"), "Manifest")?;
    let repository = child(value, "repository", "Manifest")?;
    fields(
        repository,
        &["identity", "revision_policy", "revision"],
        &[],
        "Manifest repository",
    )?;
    if text(repository.get("revision_policy"), "revision policy")? != "exact_git_commit" {
        return Err("Manifest repository revision policy is not exact_git_commit".into());
    }
    let repository_identity = text(repository.get("identity"), "repository identity")?;
    let revision = text(repository.get("revision"), "repository revision")?;
    if !commit(revision) {
        return Err("Manifest repository revision is not a full lowercase Git commit".into());
    }

    let profiles = inventory(&root, value.get("profiles"), "Profile", PROFILE)?;
    let bindings = inventory(&root, value.get("bindings"), "Binding", BINDING)?;
    let methods = inventory(&root, value.get("methods"), "Method", METHOD)?;
    let profile_roots = keyed(&profiles, "profile_id")?;
    let method_roots = keyed(&methods, "method_id")?;
    for profile in &profiles {
        validate_profile(profile)?;
    }
    for method in &methods {
        validate_method(&root, method)?;
    }
    for binding in &bindings {
        validate_binding(
            binding,
            &profile_roots,
            &method_roots,
            repository_identity,
            revision,
        )?;
    }
    Ok(Inspection {
        schema: "vela.cli.integration-inspection.v1",
        ok: true,
        command: "integration inspect",
        authority_effect: "none",
        repository: repository_identity.into(),
        revision: revision.into(),
        manifest_root: manifest.root,
        profiles: views(&profiles, "Profile", "profile_id")?,
        bindings: views(&bindings, "Binding", "binding_id")?,
        methods: views(&methods, "Method", "method_id")?,
        does_not_establish: [
            "native Method execution",
            "scientific acceptance",
            "a Vela Decision or Event",
            "Standing",
        ],
    })
}

fn inventory(
    root: &Path,
    raw: Option<&Value>,
    kind: &'static str,
    schema: &str,
) -> Result<Vec<Document>, String> {
    let entries = array(raw, &format!("Manifest {kind} inventory"))?;
    if entries.is_empty() {
        return Err(format!("Manifest has no {kind} inventory"));
    }
    let (id_field, root_field) = match kind {
        "Profile" => ("profile_id", "profile_root"),
        "Binding" => ("binding_id", "binding_root"),
        "Method" => ("method_id", "method_root"),
        _ => unreachable!(),
    };
    let (mut paths, mut roots, mut ids) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut documents = Vec::new();
    for entry in entries {
        let item = object(entry, "Manifest inventory item")?;
        let required = if kind == "Profile" {
            &["id", "version", "path", "root"][..]
        } else if kind == "Method" {
            &["id", "path", "root"][..]
        } else {
            &["path", "root"][..]
        };
        fields(
            item,
            required,
            if kind == "Binding" { &["id"] } else { &[] },
            kind,
        )?;
        let path = text(item.get("path"), "inventory path")?;
        let expected = text(item.get("root"), "inventory root")?;
        if !digest(expected) || !paths.insert(path) || !roots.insert(expected) {
            return Err(format!(
                "Manifest {kind} inventory path or root is invalid or repeated"
            ));
        }
        let document = load(root, path, schema, root_field)?;
        let body = object(&document.value, kind)?;
        if !ids.insert(text(body.get(id_field), id_field)?.to_string()) {
            return Err(format!("Manifest {kind} inventory identity is repeated"));
        }
        if document.root != expected
            || text(body.get(root_field), root_field)? != expected
            || item
                .get("id")
                .is_some_and(|id| id.as_str() != body.get(id_field).and_then(Value::as_str))
            || (kind == "Profile"
                && item.get("version").and_then(Value::as_str)
                    != body.get("version").and_then(Value::as_str))
        {
            return Err(format!("Manifest {kind} inventory drift at {path}"));
        }
        documents.push(document);
    }
    Ok(documents)
}

fn validate_profile(document: &Document) -> Result<(), String> {
    let value = object(&document.value, "Profile")?;
    fields(
        value,
        &[
            "schema",
            "profile_root",
            "profile_id",
            "version",
            "conformance",
            "rights",
            "limitations",
            "nonclaims",
            "authority_effect",
        ],
        &["source"],
        "Profile",
    )?;
    if text(value.get("version"), "Profile version")? != "0.1" {
        return Err("unsupported Profile version".into());
    }
    fields(
        child(value, "rights", "Profile")?,
        &["license", "redistribution"],
        &["repository_authored_license", "dependencies"],
        "Profile rights",
    )?;
    nonempty(
        value,
        &["conformance", "limitations", "nonclaims"],
        "Profile",
    )?;
    source(value.get("source"), "Profile")
}

fn validate_method(root: &Path, document: &Document) -> Result<(), String> {
    let value = object(&document.value, "Method")?;
    fields(
        value,
        &[
            "schema",
            "method_root",
            "method_id",
            "version",
            "implementation",
            "environment",
            "inputs",
            "outputs",
            "limitations",
            "nonclaims",
            "authority_effect",
        ],
        &[],
        "Method",
    )?;
    if text(value.get("version"), "Method version")? != "0.1" {
        return Err("unsupported Method version".into());
    }
    let implementation = child(value, "implementation", "Method")?;
    fields(
        implementation,
        &["path", "digest"],
        &[],
        "Method implementation",
    )?;
    let path = text(implementation.get("path"), "Method implementation path")?;
    let expected = text(implementation.get("digest"), "Method implementation digest")?;
    if !digest(expected) || file_digest(root, path)? != expected {
        return Err(format!("Method implementation digest drift at {path}"));
    }
    nonempty(
        value,
        &["inputs", "outputs", "limitations", "nonclaims"],
        "Method",
    )
}

fn validate_binding(
    document: &Document,
    profiles: &BTreeMap<String, String>,
    methods: &BTreeMap<String, String>,
    repository_identity: &str,
    repository_revision: &str,
) -> Result<(), String> {
    let value = object(&document.value, "Binding")?;
    fields(
        value,
        &[
            "schema",
            "binding_root",
            "binding_id",
            "profile",
            "references",
            "mappings",
            "translations",
            "methods",
            "outputs",
            "authority_effect",
        ],
        &["limitations", "nonclaims", "custody", "source"],
        "Binding",
    )?;
    validate_outputs(value.get("outputs"), "Binding")?;
    source(value.get("source"), "Binding")?;
    let profile = child(value, "profile", "Binding")?;
    fields(profile, &["id", "version", "root"], &[], "Binding Profile")?;
    let profile_id = text(profile.get("id"), "Binding Profile id")?;
    if text(profile.get("version"), "Binding Profile version")? != "0.1"
        || profiles.get(profile_id).map(String::as_str)
            != Some(text(profile.get("root"), "Binding Profile root")?)
    {
        return Err("Binding Profile identity, version, or root drift".into());
    }
    let references = array(value.get("references"), "Binding references")?;
    if references.is_empty() {
        return Err("Binding has no Exact Reference".into());
    }
    for reference in references {
        exact_reference(reference, repository_identity, repository_revision)?;
    }
    for mapping in array(value.get("mappings"), "Binding mappings")? {
        let mapping = object(mapping, "mapping")?;
        fields(mapping, &["source", "target", "relation"], &[], "mapping")?;
        text(mapping.get("source"), "mapping source")?;
        text(mapping.get("target"), "mapping target")?;
        if !RELATIONS.contains(&text(mapping.get("relation"), "mapping relation")?) {
            return Err("unsupported mapping relation".into());
        }
    }
    for translation in array(value.get("translations"), "Binding translations")? {
        let translation = object(translation, "translation")?;
        fields(
            translation,
            &["source", "target", "disposition"],
            &[],
            "translation",
        )?;
        text(translation.get("source"), "translation source")?;
        text(translation.get("target"), "translation target")?;
        if !DISPOSITIONS.contains(&text(
            translation.get("disposition"),
            "translation disposition",
        )?) {
            return Err("unsupported translation disposition".into());
        }
    }
    let bound = array(value.get("methods"), "Binding Methods")?;
    if bound.is_empty() {
        return Err("Binding has no Method".into());
    }
    for method in bound {
        let method = object(method, "Binding Method")?;
        fields(method, &["id", "root"], &[], "Binding Method")?;
        let id = text(method.get("id"), "Binding Method id")?;
        if methods.get(id).map(String::as_str) != Some(text(method.get("root"), "root")?) {
            return Err(format!("Binding Method identity or root drift for {id}"));
        }
    }
    Ok(())
}

fn exact_reference(
    raw: &Value,
    repository_identity: &str,
    repository_revision: &str,
) -> Result<(), String> {
    let value = object(raw, "Exact Reference")?;
    fields(
        value,
        &[
            "schema",
            "native_identity",
            "revision",
            "content_fixity",
            "locator",
        ],
        &["selector"],
        "Exact Reference",
    )?;
    if text(value.get("schema"), "Exact Reference schema")? != REFERENCE {
        return Err("unsupported Exact Reference schema".into());
    }
    let identity = child(value, "native_identity", "Exact Reference")?;
    fields(
        identity,
        &["system", "object_kind", "identifier"],
        &[],
        "native identity",
    )?;
    text(identity.get("system"), "native system")?;
    text(identity.get("object_kind"), "native object kind")?;
    let identifier = text(identity.get("identifier"), "native identifier")?;
    let revision = child(value, "revision", "Exact Reference")?;
    fields(revision, &["kind", "value"], &[], "revision")?;
    if text(revision.get("kind"), "revision kind")? != "git_commit"
        || !commit(text(revision.get("value"), "revision value")?)
    {
        return Err("Exact Reference revision is not a full Git commit".into());
    }
    let fixity = child(value, "content_fixity", "Exact Reference")?;
    fields(
        fixity,
        &["media_type", "digest", "size"],
        &[],
        "content fixity",
    )?;
    text(fixity.get("media_type"), "content media type")?;
    if !digest(text(fixity.get("digest"), "content digest")?)
        || !fixity
            .get("size")
            .is_some_and(|size| size.as_i64().is_some_and(|size| size >= 0))
    {
        return Err("Exact Reference content fixity is invalid".into());
    }
    if let Some(selector) = value.get("selector") {
        let selector = object(selector, "selector")?;
        fields(selector, &["kind", "value"], &[], "selector")?;
        text(selector.get("kind"), "selector kind")?;
        let selected = text(selector.get("value"), "selector value")?;
        if identifier != selected && !identifier.ends_with(&format!("#{selected}")) {
            return Err("Exact Reference selector drift".into());
        }
    }
    let locator = child(value, "locator", "Exact Reference")?;
    fields(
        locator,
        &["uri", "mutable", "authentication"],
        &[],
        "locator",
    )?;
    if !locator.get("mutable").is_some_and(Value::is_boolean) {
        return Err("Exact Reference locator mutability is not boolean".into());
    }
    let uri = text(locator.get("uri"), "locator URI")?;
    text(locator.get("authentication"), "locator authentication")?;
    if uri == repository_identity
        && text(revision.get("value"), "revision value")? != repository_revision
    {
        return Err("Exact Reference revision drift".into());
    }
    if !uri.contains("://") {
        safe_path(uri).map_err(|_| "Exact Reference locator escapes repository root")?;
    }
    Ok(())
}

fn load(root: &Path, path: &str, schema: &str, root_field: &str) -> Result<Document, String> {
    let relative = safe_path(path)?;
    let bytes = crate::bounded_file::read_bounded_repository_file(
        root,
        &relative,
        LIMIT,
        "integration document",
    )
    .map_err(|error| error.to_string())?;
    let parsed = toml::from_str::<toml::Value>(
        std::str::from_utf8(&bytes)
            .map_err(|_| format!("integration document is not UTF-8: {path}"))?,
    )
    .map_err(|error| format!("parse integration document {path}: {error}"))?;
    let value = serde_json::to_value(parsed).map_err(|error| error.to_string())?;
    guards(&value, path)?;
    let body = object(&value, path)?;
    if text(body.get("schema"), "schema")? != schema {
        return Err(format!("unsupported schema at {path}"));
    }
    let claimed = text(body.get(root_field), root_field)?.to_string();
    if !digest(&claimed) || claimed != document_root(schema, root_field, &value)? {
        return Err(format!("{root_field} or root domain mismatch at {path}"));
    }
    if text(body.get("authority_effect"), "authority effect")? != "none" {
        return Err(format!(
            "integration document has authority effect at {path}"
        ));
    }
    Ok(Document {
        path: path.into(),
        value,
        root: claimed,
    })
}

fn document_root(schema: &str, root_field: &str, value: &Value) -> Result<String, String> {
    let mut value = value.clone();
    object_mut(&mut value, "document")?.insert(root_field.into(), Value::String(String::new()));
    let bytes = vela_protocol::canonical::to_canonical_bytes(&value)?;
    let mut hash = Sha256::new();
    hash.update(schema.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn guards(value: &Value, label: &str) -> Result<(), String> {
    match value {
        Value::Object(values) => {
            let unavailable = ["availability", "evidence_availability"]
                .iter()
                .filter_map(|key| values.get(*key).and_then(Value::as_str))
                .any(|value| value == "unavailable");
            if unavailable
                && ["outcome", "result"].iter().any(|key| {
                    values
                        .get(*key)
                        .is_some_and(|value| value.as_str() != Some("unavailable"))
                })
            {
                return Err(format!(
                    "{label} converts unavailable evidence into a result"
                ));
            }
            for (key, child) in values {
                if key != "authority_effect"
                    && AUTHORITY_KEYS.contains(&key.to_ascii_lowercase().as_str())
                {
                    return Err(format!("{label} contains forbidden authority field {key}"));
                }
                guards(child, label)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                guards(child, label)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn keyed(documents: &[Document], field: &str) -> Result<BTreeMap<String, String>, String> {
    documents
        .iter()
        .map(|document| {
            Ok((
                text(object(&document.value, "document")?.get(field), field)?.into(),
                document.root.clone(),
            ))
        })
        .collect()
}

fn views(documents: &[Document], kind: &'static str, field: &str) -> Result<Vec<View>, String> {
    documents
        .iter()
        .map(|document| {
            Ok(View {
                kind,
                id: text(object(&document.value, kind)?.get(field), field)?.into(),
                path: document.path.clone(),
                root: document.root.clone(),
            })
        })
        .collect()
}

fn fields(
    value: &Map<String, Value>,
    required: &[&str],
    optional: &[&str],
    label: &str,
) -> Result<(), String> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let required = required.iter().copied().collect::<BTreeSet<_>>();
    let allowed = required
        .iter()
        .copied()
        .chain(optional.iter().copied())
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&actual).copied().collect::<Vec<_>>();
    let unknown = actual.difference(&allowed).copied().collect::<Vec<_>>();
    if missing.is_empty() && unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{label} fields: missing={missing:?} unknown={unknown:?}"
        ))
    }
}

fn validate_outputs(raw: Option<&Value>, label: &str) -> Result<(), String> {
    let values = array(raw, &format!("{label} outputs"))?;
    if values.is_empty()
        || values
            .iter()
            .any(|value| !OUTPUTS.contains(&value.as_str().unwrap_or("")))
    {
        return Err(format!(
            "{label} declares an unsupported or authoritative output"
        ));
    }
    Ok(())
}

fn nonempty(value: &Map<String, Value>, fields: &[&str], label: &str) -> Result<(), String> {
    if fields.iter().all(|field| {
        value
            .get(*field)
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    }) {
        Ok(())
    } else {
        Err(format!("{label} has a missing or empty array"))
    }
}

fn source(raw: Option<&Value>, label: &str) -> Result<(), String> {
    if raw.is_none()
        || raw
            .and_then(Value::as_object)
            .is_some_and(|value| !value.is_empty())
    {
        Ok(())
    } else {
        Err(format!("{label}.source is empty or not an object"))
    }
}

fn file_digest(root: &Path, path: &str) -> Result<String, String> {
    let bytes = crate::bounded_file::read_bounded_repository_file(
        root,
        &safe_path(path)?,
        LIMIT,
        "Method implementation",
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn safe_path(raw: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(raw);
    if raw.is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        Err(format!("integration path escapes repository root: {raw}"))
    } else {
        Ok(path.into())
    }
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} is not an object"))
}
fn object_mut<'a>(value: &'a mut Value, label: &str) -> Result<&'a mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| format!("{label} is not an object"))
}
fn child<'a>(
    value: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a Map<String, Value>, String> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{label}.{field} is missing or not an object"))
}
fn array<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a Vec<Value>, String> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} is missing or not an array"))
}
fn text<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a str, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} is missing or empty"))
}
fn digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(value: &mut Value, path: &str, replacement: Value) {
        let parts = path.split('.').collect::<Vec<_>>();
        let mut cursor = value;
        for part in &parts[..parts.len() - 1] {
            cursor = match cursor {
                Value::Array(values) => &mut values[part.parse::<usize>().unwrap()],
                Value::Object(values) => values.get_mut(*part).unwrap(),
                _ => panic!("invalid fixture path"),
            };
        }
        let key = parts.last().unwrap();
        if replacement == json!({"$delete": true}) {
            cursor.as_object_mut().unwrap().remove(*key);
        } else if let Some(values) = cursor.as_array_mut() {
            values[key.parse::<usize>().unwrap()] = replacement;
        } else {
            cursor
                .as_object_mut()
                .unwrap()
                .insert((*key).into(), replacement);
        }
    }

    fn reroot(packet: &mut Value, kind: &str, preserve_wrong_domain: bool) {
        let (schema, field, inventory) = match kind {
            "profile" => (PROFILE, "profile_root", "profiles"),
            "binding" => (BINDING, "binding_root", "bindings"),
            "method" => (METHOD, "method_root", "methods"),
            "manifest" => (MANIFEST, "manifest_root", ""),
            _ => return,
        };
        if !preserve_wrong_domain {
            packet[kind][field] =
                Value::String(document_root(schema, field, &packet[kind]).unwrap());
        }
        if kind != "manifest" {
            packet["manifest"][inventory][0]["root"] = packet[kind][field].clone();
            packet["manifest"]["manifest_root"] = Value::String(
                document_root(MANIFEST, "manifest_root", &packet["manifest"]).unwrap(),
            );
        }
    }

    fn write(root: &Path, packet: &Value) {
        for directory in [".vela/profiles", ".vela/bindings", ".vela/methods", "tools"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(root.join("tools/check.py"), b"pass\n").unwrap();
        for (kind, path) in [
            ("manifest", "vela.toml"),
            ("profile", ".vela/profiles/proof.json"),
            ("binding", ".vela/bindings/proof.json"),
            ("method", ".vela/methods/native-check.json"),
        ] {
            let value = toml::Value::try_from(packet[kind].clone()).unwrap();
            fs::write(root.join(path), toml::to_string(&value).unwrap()).unwrap();
        }
    }

    #[test]
    fn fixture_packet_and_hostiles() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../conformance/integration-v0.1/fixtures.json"
        ))
        .unwrap();
        let positive = tempfile::tempdir().unwrap();
        write(positive.path(), &fixture["packet"]);
        inspect(positive.path()).unwrap();

        let mut duplicate = fixture["packet"].clone();
        let mut entry = duplicate["manifest"]["profiles"][0].clone();
        entry["path"] = json!(".vela/profiles/duplicate.json");
        duplicate["manifest"]["profiles"]
            .as_array_mut()
            .unwrap()
            .push(entry);
        reroot(&mut duplicate, "manifest", false);
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), &duplicate);
        let profiles = directory.path().join(".vela/profiles");
        fs::copy(profiles.join("proof.json"), profiles.join("duplicate.json")).unwrap();
        assert!(inspect(directory.path()).is_err());

        for path in [
            "references.0.native_identity.system",
            "references.0.content_fixity.media_type",
            "references.0.locator.authentication",
            "references.0.selector.kind",
            "mappings.0.source",
            "translations.0.target",
        ] {
            let mut packet = fixture["packet"].clone();
            set(&mut packet["binding"], path, json!(""));
            reroot(&mut packet, "binding", false);
            let directory = tempfile::tempdir().unwrap();
            write(directory.path(), &packet);
            assert!(inspect(directory.path()).is_err(), "{path}");
        }

        let mut omitted = fixture["packet"].clone();
        omitted["binding"]["references"][0]
            .as_object_mut()
            .unwrap()
            .remove("selector");
        reroot(&mut omitted, "binding", false);
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), &omitted);
        inspect(directory.path()).unwrap();
        let profile_root =
            document_root(PROFILE, "profile_root", &fixture["packet"]["profile"]).unwrap();
        assert_ne!(
            profile_root,
            document_root(METHOD, "profile_root", &fixture["packet"]["profile"]).unwrap()
        );

        for case in fixture["hostile"].as_array().unwrap() {
            let mut packet = fixture["packet"].clone();
            let name = case[0].as_str().unwrap();
            let target = case[1].as_str().unwrap();
            set(
                &mut packet[target],
                case[2].as_str().unwrap(),
                case[3].clone(),
            );
            if case.as_array().unwrap().len() == 5 {
                let second = case[4].as_array().unwrap();
                set(
                    &mut packet[target],
                    second[0].as_str().unwrap(),
                    second[1].clone(),
                );
            }
            if target == "check_output" {
                let output = &packet[target];
                let unavailable = output["evidence_availability"] == "unavailable";
                let valid = ["pass", "fail", "inconclusive", "error", "unavailable"]
                    .contains(&output["outcome"].as_str().unwrap_or(""));
                assert!(
                    guards(output, "synthetic output").is_err()
                        || output["authority_effect"] != "none"
                        || !valid
                        || (unavailable && output["outcome"] != "unavailable"),
                    "{name}"
                );
                continue;
            }
            let deliberate_root = matches!(name, "wrong_root" | "short_root");
            if !deliberate_root {
                reroot(&mut packet, target, name == "wrong_root_domain");
            }
            let directory = tempfile::tempdir().unwrap();
            write(directory.path(), &packet);
            assert!(inspect(directory.path()).is_err(), "{name}");
        }
    }
}
