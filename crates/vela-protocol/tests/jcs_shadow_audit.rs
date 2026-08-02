use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE as BASE64_URL_SAFE};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use vela_protocol::canonical::to_canonical_bytes;

const MAX_JCS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditFixture {
    schema: String,
    result_root: String,
    result: AuditResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditResult {
    schema: String,
    canonicalizer: Canonicalizer,
    repositories: Vec<RepositoryResult>,
    exceptions: Vec<ExceptionResult>,
    totals: AuditTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Canonicalizer {
    package: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryResult {
    name: String,
    remote: String,
    commit: String,
    tree: String,
    files: usize,
    matches: usize,
    authority_payloads: usize,
    authority_payload_matches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExceptionResult {
    repository: String,
    path: String,
    raw_sha256: String,
    git_blob: String,
    first_difference: usize,
    unsafe_integer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditTotals {
    repositories: usize,
    files: usize,
    matches: usize,
    authority_payloads: usize,
    authority_payload_matches: usize,
    exceptions: usize,
    unsafe_integers: usize,
}

#[derive(Default)]
struct CheckoutCounts {
    files: usize,
    matches: usize,
    authority_payloads: usize,
    authority_payload_matches: usize,
    exceptions: Vec<ExceptionResult>,
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object properties")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| UniqueJson(Value::Number(number)))
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = HashSet::new();
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON property `{key}`"
                )));
            }
            values.insert(key, object.next_value::<UniqueJson>()?.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

#[test]
fn pinned_jcs_implementation_matches_rfc_8785_samples() {
    let input = br#"{
        "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
        "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
        "literals": [null, true, false]
    }"#;
    let value = parse_unique_json(input).unwrap();
    assert_eq!(
        jcs_string(&value),
        r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
    );

    let number_cases = [
        (0x0000_0000_0000_0000, "0"),
        (0x8000_0000_0000_0000, "0"),
        (0x0000_0000_0000_0001, "5e-324"),
        (0x8000_0000_0000_0001, "-5e-324"),
        (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
        (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
        (0x4340_0000_0000_0000, "9007199254740992"),
        (0xc340_0000_0000_0000, "-9007199254740992"),
        (0x4430_0000_0000_0000, "295147905179352830000"),
        (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
        (0x44b5_2d02_c7e1_4af6, "1e+23"),
        (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
        (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
        (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
        (0x444b_1ae4_d6e2_ef50, "1e+21"),
        (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
        (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
        (0x41b3_de43_5555_5553, "333333333.3333332"),
        (0x41b3_de43_5555_5554, "333333333.33333325"),
        (0x41b3_de43_5555_5555, "333333333.3333333"),
        (0x41b3_de43_5555_5556, "333333333.3333334"),
        (0x41b3_de43_5555_5557, "333333333.33333343"),
        (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
    ];
    for (bits, expected) in number_cases {
        assert_eq!(
            jcs_string(&f64::from_bits(bits)),
            expected,
            "bits={bits:016x}"
        );
    }
    assert!(serde_json_canonicalizer::to_vec(&f64::NAN).is_err());
    assert!(serde_json_canonicalizer::to_vec(&f64::INFINITY).is_err());
    assert!(serde_json_canonicalizer::to_vec(&f64::NEG_INFINITY).is_err());
}

#[test]
fn pinned_jcs_implementation_sorts_property_names_by_utf16() {
    let value = serde_json::json!({
        "\u{20ac}": "Euro Sign",
        "\r": "Carriage Return",
        "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "\u{1f600}": "Emoji: Grinning Face",
        "\u{0080}": "Control",
        "\u{00f6}": "Latin Small Letter O With Diaeresis"
    });
    let canonical = jcs_string(&value);
    let values = [
        "Carriage Return",
        "One",
        "Control",
        "Latin Small Letter O With Diaeresis",
        "Euro Sign",
        "Emoji: Grinning Face",
        "Hebrew Letter Dalet With Dagesh",
    ];
    let positions = values.map(|value| canonical.find(value).expect("RFC 8785 sample value"));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn raw_json_rejects_duplicate_properties_and_invalid_inputs() {
    assert!(parse_unique_json(br#"{"a":1,"a":2}"#).is_err());
    assert!(parse_unique_json(br#"{"outer":{"a":1,"a":2}}"#).is_err());
    assert!(parse_unique_json(br#"{"value":"\ud800"}"#).is_err());
    assert!(parse_unique_json(br#"{"value":NaN}"#).is_err());
    assert!(parse_unique_json(br#"{"value":Infinity}"#).is_err());
    assert!(parse_unique_json(br#"{"value":-Infinity}"#).is_err());
    assert!(parse_unique_json(br#"{"value":1e400}"#).is_err());
}

#[test]
#[ignore = "requires the exact four local Frontier checkouts"]
fn retained_frontier_json_is_audited_against_rfc_8785() {
    let fixture = read_fixture();
    assert_eq!(fixture.schema, "vela.jcs-shadow-audit-fixture.v1");
    assert_eq!(fixture.result.repositories.len(), 4);
    assert_eq!(fixture.result.totals.repositories, 4);
    assert_eq!(root(&fixture.result), fixture.result_root);

    let roots = env::var_os("VELA_JCS_AUDIT_FRONTIERS")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .filter(|paths| !paths.is_empty())
        .expect("VELA_JCS_AUDIT_FRONTIERS must contain the four Frontier paths");
    assert_eq!(
        roots.len(),
        4,
        "the frozen audit requires exactly four Frontiers"
    );

    let mut remaining = roots;
    let mut repositories = Vec::new();
    let mut exceptions = Vec::new();
    for expected in &fixture.result.repositories {
        let index = remaining
            .iter()
            .position(|path| normalized_remote(path) == expected.remote)
            .unwrap_or_else(|| panic!("missing checkout for {}", expected.remote));
        let checkout = remaining.remove(index);
        assert_clean_checkout(&checkout);
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), expected.commit);
        assert_eq!(git(&checkout, &["rev-parse", "HEAD^{tree}"]), expected.tree);
        let counts = audit_checkout(&checkout, &expected.name);
        repositories.push(RepositoryResult {
            name: expected.name.clone(),
            remote: expected.remote.clone(),
            commit: expected.commit.clone(),
            tree: expected.tree.clone(),
            files: counts.files,
            matches: counts.matches,
            authority_payloads: counts.authority_payloads,
            authority_payload_matches: counts.authority_payload_matches,
        });
        exceptions.extend(counts.exceptions);
    }
    assert!(remaining.is_empty());
    exceptions.sort_by(|left, right| {
        (&left.repository, &left.path).cmp(&(&right.repository, &right.path))
    });

    let actual = AuditResult {
        schema: "vela.jcs-shadow-audit-result.v1".into(),
        canonicalizer: Canonicalizer {
            package: "serde_json_canonicalizer".into(),
            version: "0.3.2".into(),
        },
        totals: AuditTotals {
            repositories: repositories.len(),
            files: repositories.iter().map(|value| value.files).sum(),
            matches: repositories.iter().map(|value| value.matches).sum(),
            authority_payloads: repositories
                .iter()
                .map(|value| value.authority_payloads)
                .sum(),
            authority_payload_matches: repositories
                .iter()
                .map(|value| value.authority_payload_matches)
                .sum(),
            exceptions: exceptions.len(),
            unsafe_integers: exceptions
                .iter()
                .map(|value| value.unsafe_integer_count)
                .sum(),
        },
        repositories,
        exceptions,
    };

    assert_eq!(actual, fixture.result, "frozen JCS audit result drifted");
    assert_eq!(root(&actual), fixture.result_root);
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("conformance/jcs-shadow-audit.v1.json")
}

fn read_fixture() -> AuditFixture {
    let bytes = fs::read(fixture_path()).expect("read frozen JCS audit fixture");
    serde_json::from_slice(&bytes).expect("parse frozen JCS audit fixture")
}

fn parse_unique_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueJson::deserialize(&mut deserializer)?.0;
    deserializer.end()?;
    Ok(value)
}

fn jcs_string<T: Serialize>(value: &T) -> String {
    String::from_utf8(serde_json_canonicalizer::to_vec(value).unwrap()).unwrap()
}

fn root<T: Serialize>(value: &T) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json_canonicalizer::to_vec(value).unwrap()
        ))
    )
}

fn normalized_remote(root: &Path) -> String {
    git(root, &["remote", "get-url", "origin"])
        .trim_end_matches(".git")
        .to_string()
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run git in {}: {error}", root.display()));
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        root.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn assert_clean_checkout(root: &Path) {
    assert!(
        git(root, &["status", "--porcelain", "--untracked-files=no"]).is_empty(),
        "JCS audit requires a clean tracked checkout at {}",
        root.display()
    );
}

fn audit_checkout(root: &Path, repository: &str) -> CheckoutCounts {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--", "*.json"])
        .output()
        .expect("list tracked JSON");
    assert!(output.status.success());

    let mut counts = CheckoutCounts::default();
    for relative in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let relative = PathBuf::from(std::str::from_utf8(relative).expect("UTF-8 JSON path"));
        let bytes = fs::read(root.join(&relative)).expect("read tracked JSON");
        let value = parse_unique_json(&bytes)
            .unwrap_or_else(|error| panic!("parse {}: {error}", relative.display()));
        counts.files += 1;
        let current = to_canonical_bytes(&value).expect("current canonicalization");
        let jcs = serde_json_canonicalizer::to_vec(&value).expect("JCS canonicalization");
        if current == jcs {
            counts.matches += 1;
        } else {
            let path = relative.to_string_lossy().into_owned();
            counts.exceptions.push(ExceptionResult {
                repository: repository.into(),
                path: path.clone(),
                raw_sha256: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
                git_blob: git(root, &["rev-parse", &format!("HEAD:{path}")]),
                first_difference: first_difference(&current, &jcs),
                unsafe_integer_count: count_unsafe_integers(&value),
            });
        }

        if relative.to_string_lossy().ends_with(".dsse.json") {
            audit_authority_payload(&value, &relative, &mut counts);
        }
    }
    counts
}

fn audit_authority_payload(value: &Value, relative: &Path, counts: &mut CheckoutCounts) {
    let encoded = value["payload"]
        .as_str()
        .unwrap_or_else(|| panic!("DSSE envelope {} has no payload", relative.display()));
    let payload = BASE64_STANDARD
        .decode(encoded)
        .or_else(|_| BASE64_URL_SAFE.decode(encoded))
        .expect("DSSE payload base64");
    let value = parse_unique_json(&payload).expect("unique DSSE payload JSON");
    let current = to_canonical_bytes(&value).expect("current DSSE canonicalization");
    assert_eq!(
        current, payload,
        "DSSE payload is not retained canonical JSON"
    );
    let jcs = serde_json_canonicalizer::to_vec(&value).expect("DSSE JCS canonicalization");
    counts.authority_payloads += 1;
    if current == jcs {
        counts.authority_payload_matches += 1;
    } else {
        panic!("DSSE authority payload differs under JCS");
    }
}

fn count_unsafe_integers(value: &Value) -> usize {
    match value {
        Value::Array(values) => values.iter().map(count_unsafe_integers).sum(),
        Value::Object(values) => values.values().map(count_unsafe_integers).sum(),
        Value::Number(number) => usize::from(
            number
                .as_u64()
                .is_some_and(|value| value > MAX_JCS_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value.unsigned_abs() > MAX_JCS_SAFE_INTEGER),
        ),
        Value::Null | Value::Bool(_) | Value::String(_) => 0,
    }
}

fn first_difference(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| left.len().min(right.len()))
}
