//! Config-file rewriting, ported from Pterodactyl Wings' `parser` package so
//! eggs behave the same. Quirks are kept on purpose and marked "like Wings".

use std::{
  collections::BTreeMap,
  path::{Component, Path, PathBuf},
  sync::LazyLock,
};

use centaurus::{
  bail,
  error::Result,
  eyre::{Context, OptionExt},
};
use regex::Regex;
use serde_json::{Map, Value};
use shared::msg::{ConfigFile, ConfigParser, ConfigReplace};
use tracing::warn;

const MAX_SIZE: u64 = 64 * 1024 * 1024;

static CONFIG: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\{\{\s?config\.([\w.-]+)\s?\}\}").expect("valid regex"));
static ARRAY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^([^\[\]]+)\[(\d+)](\..+)?$").expect("valid regex"));
static XML_ATTR: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^\[(\w+)='(.*)'\]$").expect("valid regex"));

/// Rewrites every config file below `root`. A file that fails is logged and
/// left as is, the server still starts, like Wings.
pub fn apply(root: &Path, files: &[ConfigFile], interface: &str) {
  for file in files {
    if let Err(err) = apply_one(root, file, interface) {
      warn!("Failed to update config file {}: {:?}", file.file, err);
    }
  }
}

fn apply_one(root: &Path, file: &ConfigFile, interface: &str) -> Result<()> {
  let path = jail(root, &file.file)?;
  let content = match std::fs::symlink_metadata(&path) {
    Ok(meta) if meta.len() > MAX_SIZE => {
      bail!("Config file is larger than 64 MiB");
    }
    Ok(_) => std::fs::read_to_string(&path).context("Failed to read config file")?,
    // like Wings: missing files are created, except for plain text ones
    Err(_) if file.parser == ConfigParser::File => return Ok(()),
    Err(_) => {
      if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config file directory")?;
      }
      String::new()
    }
  };
  let out = rewrite(file.parser, &content, &file.replace, interface)?;
  std::fs::write(&path, out).context("Failed to write config file")?;
  Ok(())
}

/// Resolves `file` inside `root`. Rejects `..` and any symlink on the way, so a
/// server can't point its config at a host file. The container is not running
/// while this runs, so nothing can swap a component in between.
fn jail(root: &Path, file: &str) -> Result<PathBuf> {
  let mut path = root.to_path_buf();
  for component in Path::new(file).components() {
    match component {
      Component::Normal(part) => path.push(part),
      Component::RootDir | Component::CurDir => (),
      _ => {
        bail!("Config file path escapes the server root");
      }
    }
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
      bail!("Config file path contains a symlink");
    }
  }
  Ok(path)
}

pub fn rewrite(
  parser: ConfigParser,
  content: &str,
  replace: &[ConfigReplace],
  interface: &str,
) -> Result<String> {
  Ok(match parser {
    ConfigParser::File => text(content, replace),
    ConfigParser::Properties => properties(content, replace, interface),
    ConfigParser::Ini => ini(content, replace, interface),
    ConfigParser::Json => {
      let mut doc: Value = if content.trim().is_empty() {
        Value::Object(Map::new())
      } else {
        serde_json::from_str(content).context("Invalid json")?
      };
      replace_json(&mut doc, replace, interface);
      let mut out = Vec::new();
      let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
      serde::Serialize::serialize(
        &doc,
        &mut serde_json::Serializer::with_formatter(&mut out, fmt),
      )?;
      String::from_utf8(out).context("Invalid utf-8")?
    }
    ConfigParser::Yaml => {
      let doc: serde_norway::Value = serde_norway::from_str(content).context("Invalid yaml")?;
      let mut doc = match yaml_to_json(doc) {
        Value::Null => Value::Object(Map::new()),
        doc => doc,
      };
      replace_json(&mut doc, replace, interface);
      serde_norway::to_string(&doc).context("Failed to write yaml")?
    }
    ConfigParser::Xml => xml(content, replace, interface)?,
  })
}

/// The replacement as text, like Wings' `ReplaceValue.String()`.
fn scalar(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    Value::Bool(_) | Value::Number(_) => value.to_string(),
    Value::Null => "<nil>".into(),
    _ => "<invalid>".into(),
  }
}

/// Fills `{{config.docker.interface}}`, the only Wings config value eggs can
/// read. Like Wings, every placeholder gets the value of the first one.
fn lookup(r: &ConfigReplace, interface: &str) -> String {
  let raw = scalar(&r.replace_with);
  if !r.replace_with.is_string() {
    return raw;
  }
  let Some(caps) = CONFIG.captures(&raw) else {
    return raw;
  };
  match caps[1].to_lowercase().as_str() {
    "docker.interface" | "docker.network.interface" => {
      CONFIG.replace_all(&raw, interface).into_owned()
    }
    _ => raw,
  }
}

/// Like Wings' `getKeyValue`: bools stay bools, integers become numbers.
fn typed(r: &ConfigReplace, value: String) -> Value {
  if r.replace_with.is_boolean() {
    return Value::Bool(matches!(
      value.as_str(),
      "1" | "t" | "T" | "TRUE" | "true" | "True"
    ));
  }
  match value.parse::<i64>() {
    Ok(n) => n.into(),
    Err(_) => Value::String(value),
  }
}

fn text(content: &str, replace: &[ConfigReplace]) -> String {
  let mut out = String::new();
  for line in content.lines() {
    let mut replaced = false;
    for r in replace.iter().filter(|r| line.starts_with(&r.matcher)) {
      out.push_str(&scalar(&r.replace_with));
      replaced = true;
    }
    if !replaced {
      out.push_str(line);
    }
    out.push('\n');
  }
  out
}

// --- json / yaml -----------------------------------------------------------

fn replace_json(doc: &mut Value, replace: &[ConfigReplace], interface: &str) {
  for r in replace {
    let value = lookup(r, interface);
    // like Wings: one wildcard, applied to every existing child
    if let Some((prefix, rest)) = r.matcher.split_once(".*") {
      let rest = rest.trim_matches('.');
      let children: Vec<&mut Value> = match get_mut(doc, prefix.trim_matches('.')) {
        Some(Value::Object(map)) => map.values_mut().collect(),
        Some(Value::Array(items)) => items.iter_mut().collect(),
        _ => Vec::new(),
      };
      for child in children {
        set_at_pathway(child, rest, r, &value);
      }
      continue;
    }
    set_at_pathway(doc, &r.matcher, r, &value);
  }
}

fn set_at_pathway(doc: &mut Value, path: &str, r: &ConfigReplace, value: &str) {
  let if_value = r.if_value.as_deref().unwrap_or_default();
  if if_value.is_empty() {
    return set_value_at_path(doc, path, typed(r, value.into()));
  }
  if let Some(pattern) = if_value.strip_prefix("regex:") {
    // a regex replace needs an existing value
    let Some(current) = get_mut(doc, path).map(|v| as_text(v)) else {
      return;
    };
    match Regex::new(pattern) {
      Ok(re) if re.is_match(&current) => {
        let new = re.replace_all(&current, value).into_owned();
        set_value_at_path(doc, path, Value::String(new));
      }
      Ok(_) => (),
      Err(err) => warn!("Invalid if_value regex {pattern}: {err}"),
    }
    return;
  }
  if get_mut(doc, path).is_some_and(|v| as_text(v) != if_value) {
    return;
  }
  set_value_at_path(doc, path, typed(r, value.into()));
}

/// Strings without quotes, everything else as json, so it compares against `if_value`.
fn as_text(value: &Value) -> String {
  match value {
    Value::String(s) => s.clone(),
    other => other.to_string(),
  }
}

/// `name[n].rest` addresses an array element; index 0 is created when missing.
fn set_value_at_path(doc: &mut Value, path: &str, value: Value) {
  let Some(caps) = ARRAY.captures(path) else {
    set_p(doc, path, value);
    return;
  };
  let (name, rest) = (
    &caps[1],
    caps.get(3).map(|m| m.as_str().trim_start_matches('.')),
  );
  let Ok(i) = caps[2].parse::<usize>() else {
    return;
  };
  let exists = matches!(get_mut(doc, name), Some(Value::Array(items)) if items.len() > i);
  if !exists {
    let is_array = matches!(get_mut(doc, name), Some(Value::Array(_)));
    if i != 0 || is_array {
      warn!("Config path {path} points at a missing array element");
      return;
    }
    let first = if rest.is_some() {
      Value::Object(Map::new())
    } else {
      Value::Null
    };
    set_p(doc, name, Value::Array(vec![first]));
  }
  let Some(Value::Array(items)) = get_mut(doc, name) else {
    return;
  };
  match rest {
    Some(rest) => set_p(&mut items[i], rest, value),
    None => items[i] = value,
  }
}

fn get_mut<'a>(mut doc: &'a mut Value, path: &str) -> Option<&'a mut Value> {
  if path.is_empty() {
    return Some(doc);
  }
  for part in path.split('.') {
    doc = match doc {
      Value::Object(map) => map.get_mut(part)?,
      Value::Array(items) => items.get_mut(part.parse::<usize>().ok()?)?,
      _ => return None,
    };
  }
  Some(doc)
}

/// Sets a dotted path, creating missing objects like gabs' `SetP`.
fn set_p(mut doc: &mut Value, path: &str, value: Value) {
  let parts: Vec<&str> = path.split('.').collect();
  let (last, parents) = parts.split_last().expect("split yields one part");
  for part in parents {
    if doc.is_null() {
      *doc = Value::Object(Map::new());
    }
    doc = match doc {
      Value::Object(map) => map
        .entry(*part)
        .or_insert_with(|| Value::Object(Map::new())),
      Value::Array(items) => match part.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
        Some(item) => item,
        None => return warn!("Config path {path} collides with an array"),
      },
      _ => return warn!("Config path {path} collides with a value"),
    };
  }
  if doc.is_null() {
    *doc = Value::Object(Map::new());
  }
  match doc {
    Value::Object(map) => {
      map.insert(last.to_string(), value);
    }
    Value::Array(items) => match last.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
      Some(item) => *item = value,
      None => warn!("Config path {path} collides with an array"),
    },
    _ => warn!("Config path {path} collides with a value"),
  }
}

/// Map keys become strings, like Wings' `ConvertMapI2MapS`.
fn yaml_to_json(value: serde_norway::Value) -> Value {
  use serde_norway::Value as Y;
  match value {
    Y::Null => Value::Null,
    Y::Bool(b) => Value::Bool(b),
    Y::Number(n) => serde_json::to_value(n).unwrap_or(Value::Null),
    Y::String(s) => Value::String(s),
    Y::Sequence(items) => Value::Array(items.into_iter().map(yaml_to_json).collect()),
    Y::Mapping(map) => Value::Object(
      map
        .into_iter()
        .map(|(k, v)| {
          let key = match yaml_to_json(k) {
            Value::String(s) => s,
            other => other.to_string(),
          };
          (key, yaml_to_json(v))
        })
        .collect(),
    ),
    Y::Tagged(tagged) => yaml_to_json(tagged.value),
  }
}

// --- properties ------------------------------------------------------------

/// Like Wings: keeps the leading comment block, then writes every key as
/// `key=value` with the value ASCII-escaped. Later comments are dropped.
fn properties(content: &str, replace: &[ConfigReplace], interface: &str) -> String {
  let mut out = String::new();
  for line in content.lines() {
    if !line.is_empty() && !line.starts_with('#') {
      break;
    }
    out.push_str(line);
    out.push('\n');
  }

  let mut keys: Vec<String> = Vec::new();
  let mut values: BTreeMap<String, String> = BTreeMap::new();
  for (key, value) in parse_properties(content) {
    if values.insert(key.clone(), value).is_none() {
      keys.push(key);
    }
  }

  for r in replace {
    let value = lookup(r, interface);
    let current = values.get(&r.matcher);
    if let Some(if_value) = r.if_value.as_deref().filter(|v| !v.is_empty())
      && current.is_none_or(|v| v != if_value)
    {
      continue;
    }
    if current.is_none() {
      keys.push(r.matcher.clone());
    }
    values.insert(r.matcher.clone(), value);
  }

  for key in keys {
    let value = go_quote_ascii(&values[&key]);
    out.push_str(&format!("{key}={}\n", value.trim_matches('"')));
  }
  out
}

/// Java properties: `#`/`!` comments, `=`, `:` or whitespace separators,
/// backslash escapes and line continuations.
fn parse_properties(content: &str) -> Vec<(String, String)> {
  let mut logical = Vec::new();
  let mut current = String::new();
  for line in content.lines() {
    let line = line.trim_start();
    if current.is_empty() && (line.is_empty() || line.starts_with('#') || line.starts_with('!')) {
      continue;
    }
    let trailing = line.len() - line.trim_end_matches('\\').len();
    if trailing % 2 == 1 {
      current.push_str(&line[..line.len() - 1]);
      continue;
    }
    current.push_str(line);
    logical.push(std::mem::take(&mut current));
  }
  if !current.is_empty() {
    logical.push(current);
  }

  logical
    .into_iter()
    .map(|line| {
      let mut key = String::new();
      let mut chars = line.chars().peekable();
      while let Some(c) = chars.next() {
        match c {
          '\\' => key.push(unescape(chars.next(), &mut chars)),
          '=' | ':' => break,
          c if c.is_whitespace() => {
            while chars.next_if(|c| c.is_whitespace()).is_some() {}
            chars.next_if(|c| *c == '=' || *c == ':');
            break;
          }
          c => key.push(c),
        }
      }
      while chars.next_if(|c| c.is_whitespace()).is_some() {}
      let mut value = String::new();
      while let Some(c) = chars.next() {
        match c {
          '\\' => value.push(unescape(chars.next(), &mut chars)),
          c => value.push(c),
        }
      }
      (key, value)
    })
    .collect()
}

fn unescape(c: Option<char>, rest: &mut impl Iterator<Item = char>) -> char {
  match c {
    Some('t') => '\t',
    Some('n') => '\n',
    Some('r') => '\r',
    Some('f') => '\x0c',
    Some('u') => {
      let hex: String = rest.take(4).collect();
      u32::from_str_radix(&hex, 16)
        .ok()
        .and_then(char::from_u32)
        .unwrap_or('\u{fffd}')
    }
    Some(c) => c,
    None => '\\',
  }
}

/// Go's `strconv.QuoteToASCII`.
fn go_quote_ascii(value: &str) -> String {
  let mut out = String::from("\"");
  for c in value.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\x07' => out.push_str("\\a"),
      '\x08' => out.push_str("\\b"),
      '\x0c' => out.push_str("\\f"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '\x0b' => out.push_str("\\v"),
      ' '..='~' => out.push(c),
      c if (c as u32) < 0x80 => out.push_str(&format!("\\x{:02x}", c as u32)),
      c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
      c => out.push_str(&format!("\\U{:08x}", c as u32)),
    }
  }
  out.push('"');
  out
}

// --- ini -------------------------------------------------------------------

/// `section.key`, or a bare key in the default section. Brackets protect dots
/// and are dropped, like Wings. Edits lines in place, new keys are `key=value`.
fn ini(content: &str, replace: &[ConfigReplace], interface: &str) -> String {
  let mut lines: Vec<String> = content.lines().map(String::from).collect();
  for r in replace {
    let value = lookup(r, interface);
    let (section, key) = ini_path(&r.matcher);
    set_ini(&mut lines, &section, &key, &value);
  }
  let mut out = lines.join("\n");
  out.push('\n');
  out
}

fn ini_path(matcher: &str) -> (String, String) {
  let mut path: Vec<String> = Vec::new();
  let mut part = String::new();
  let mut depth = 0;
  for c in matcher.chars() {
    match c {
      '[' => depth += 1,
      ']' => depth -= 1,
      '.' if depth == 0 && path.is_empty() => path.push(std::mem::take(&mut part)),
      c => part.push(c),
    }
  }
  path.push(part);
  match path.as_slice() {
    [section, key] => (section.clone(), key.clone()),
    _ => (String::new(), path.remove(0)),
  }
}

fn ini_section(line: &str) -> Option<&str> {
  let line = line.trim();
  line.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

fn set_ini(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
  let mut current = "";
  let mut section_end = None;
  for (i, line) in lines.iter_mut().enumerate() {
    if let Some(name) = ini_section(line) {
      if current == section && section_end.is_none() {
        section_end = Some(i);
      }
      current = name;
      continue;
    }
    if current != section {
      continue;
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with(';') || trimmed.starts_with('#') {
      continue;
    }
    let Some(sep) = line.find(['=', ':']) else {
      continue;
    };
    if line[..sep].trim() == key {
      let spacing = if line[sep + 1..].starts_with(' ') {
        " "
      } else {
        ""
      };
      *line = format!("{}{spacing}{value}", &line[..=sep]);
      return;
    }
  }
  let new = format!("{key}={value}");
  match section_end {
    Some(end) => {
      // before the blank lines that separate it from the next section
      let mut at = end;
      while at > 0 && lines[at - 1].trim().is_empty() {
        at -= 1;
      }
      lines.insert(at, new);
    }
    None if current == section => lines.push(new),
    None => {
      if lines.last().is_some_and(|l| !l.trim().is_empty()) {
        lines.push(String::new());
      }
      lines.push(format!("[{section}]"));
      lines.push(new);
    }
  }
}

// --- xml -------------------------------------------------------------------

/// Dots become the element path, the first part is the root. Missing elements
/// are created unless the path has `*`. `[attr='v']` sets an attribute.
fn xml(content: &str, replace: &[ConfigReplace], interface: &str) -> Result<String> {
  use xmltree::{Element, EmitterConfig, XMLNode};

  let mut root = if content.trim().is_empty() {
    None
  } else {
    let nodes = Element::parse_all(content.as_bytes()).context("Invalid xml")?;
    nodes.into_iter().find_map(|n| match n {
      XMLNode::Element(e) => Some(e),
      _ => None,
    })
  };

  for (i, r) in replace.iter().enumerate() {
    let value = lookup(r, interface);
    let parts: Vec<&str> = r.matcher.split('.').collect();
    if i == 0 && root.is_none() {
      root = Some(Element::new(parts[0]));
    }
    let Some(root) = root.as_mut() else {
      continue;
    };
    if !r.matcher.contains('*') {
      let mut element = &mut *root;
      for tag in &parts[1..] {
        let pos = element
          .children
          .iter()
          .position(|c| matches!(c, XMLNode::Element(e) if e.name == *tag));
        let pos = pos.unwrap_or_else(|| {
          element.children.push(XMLNode::Element(Element::new(tag)));
          element.children.len() - 1
        });
        let XMLNode::Element(child) = &mut element.children[pos] else {
          unreachable!("position matched an element");
        };
        element = child;
      }
    }
    for element in find_xml(root, &parts) {
      if let Some(caps) = XML_ATTR.captures(&value) {
        element.attributes.insert(caps[1].into(), caps[2].into());
      } else {
        match element
          .children
          .iter_mut()
          .find(|c| matches!(c, XMLNode::Text(_)))
        {
          Some(text) => *text = XMLNode::Text(value.clone()),
          None => element.children.insert(0, XMLNode::Text(value.clone())),
        }
      }
    }
  }

  let root = root.ok_or_eyre("Xml file has no root element")?;
  let mut out = Vec::new();
  root
    .write_with_config(
      &mut out,
      EmitterConfig::new()
        .perform_indent(true)
        .indent_string("  "),
    )
    .context("Failed to write xml")?;
  Ok(String::from_utf8(out).context("Invalid utf-8")?)
}

fn find_xml<'a>(root: &'a mut xmltree::Element, parts: &[&str]) -> Vec<&'a mut xmltree::Element> {
  if parts[0] != "*" && root.name != parts[0] {
    return Vec::new();
  }
  let mut found = vec![root];
  for part in &parts[1..] {
    found = found
      .into_iter()
      .flat_map(|e| {
        e.children.iter_mut().filter_map(|c| match c {
          xmltree::XMLNode::Element(e) if *part == "*" || e.name == *part => Some(e),
          _ => None,
        })
      })
      .collect();
  }
  found
}

#[cfg(test)]
mod test {
  use serde_json::json;

  use super::*;

  fn r(matcher: &str, if_value: Option<&str>, with: Value) -> ConfigReplace {
    ConfigReplace {
      matcher: matcher.into(),
      if_value: if_value.map(String::from),
      replace_with: with,
    }
  }

  fn run(parser: ConfigParser, content: &str, replace: &[ConfigReplace]) -> String {
    rewrite(parser, content, replace, "172.18.0.1").unwrap()
  }

  #[test]
  fn text_replaces_whole_lines() {
    let out = run(
      ConfigParser::File,
      "port 1\r\nname a\n",
      &[r("port", None, json!("port 25565"))],
    );
    assert_eq!(out, "port 25565\nname a\n");
  }

  #[test]
  fn properties_like_wings() {
    let content = "#Minecraft server properties\n#date\nserver-port=1\nonline-mode=true\n# dropped\nmotd=caf\\u00e9\n";
    let out = run(
      ConfigParser::Properties,
      content,
      &[
        r("server-port", None, json!("25565")),
        r("online-mode", Some("true"), json!("false")),
        r("pvp", Some("true"), json!("false")),
        r("server-ip", None, json!("{{config.docker.interface}}")),
      ],
    );
    assert_eq!(
      out,
      "#Minecraft server properties\n#date\nserver-port=25565\nonline-mode=false\nmotd=caf\\u00e9\nserver-ip=172.18.0.1\n"
    );
    // created from an empty file
    assert_eq!(
      run(ConfigParser::Properties, "", &[r("a", None, json!(1))]),
      "a=1\n"
    );
  }

  #[test]
  fn json_paths_values_and_if_value() {
    let content = r#"{"listeners":[{"host":"0.0.0.0:1","query_port":1}],"servers":{"a":{"address":"x"},"b":{"address":"y"}},"debug":"no"}"#;
    let out = run(
      ConfigParser::Json,
      content,
      &[
        r("listeners[0].host", None, json!("0.0.0.0:25565")),
        r("listeners[0].query_port", None, json!("25565")),
        r(
          "servers.*.address",
          None,
          json!("{{config.docker.interface}}:1"),
        ),
        r("debug", Some("yes"), json!("ignored")),
        r("online", None, json!(true)),
        r("new.nested", None, json!("v")),
        r("motd", Some("regex:^(.*)$"), json!("never")),
      ],
    );
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
      doc,
      json!({
        "listeners": [{"host": "0.0.0.0:25565", "query_port": 25565}],
        "servers": {"a": {"address": "172.18.0.1:1"}, "b": {"address": "172.18.0.1:1"}},
        "debug": "no",
        "online": true,
        "new": {"nested": "v"},
      })
    );
    assert!(out.contains("\n    \"debug\""));
  }

  #[test]
  fn json_regex_if_value_and_array_creation() {
    let out = run(
      ConfigParser::Json,
      "",
      &[
        r("items[0].name", None, json!("a")),
        // like Wings: the existence check does not understand `[0]`, so this is skipped
        r("items[0].name", Some("regex:^(a)$"), json!("${1}b")),
        r("motd", None, json!("hi")),
        r("motd", Some("regex:^(h)i$"), json!("${1}ey")),
      ],
    );
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc, json!({"items": [{"name": "a"}], "motd": "hey"}));
  }

  #[test]
  fn yaml_like_json() {
    let out = run(
      ConfigParser::Yaml,
      "listeners:\n- host: 0.0.0.0:1\n  query_port: 1\n1: numeric key\n",
      &[
        r("listeners[0].host", None, json!("0.0.0.0:25565")),
        r("listeners[0].query_port", None, json!(25565)),
      ],
    );
    let doc: Value = serde_norway::from_str(&out).unwrap();
    assert_eq!(
      doc,
      json!({"1": "numeric key", "listeners": [{"host": "0.0.0.0:25565", "query_port": 25565}]})
    );
  }

  #[test]
  fn ini_sections_and_keys() {
    let content = "top=1\n\n[server]\nport = 1\n; comment\n\n[other]\nx=1\n";
    let out = run(
      ConfigParser::Ini,
      content,
      &[
        r("server.port", None, json!(25565)),
        r("server.name", None, json!("a")),
        r("top", None, json!(2)),
        r("new", None, json!(3)),
        r("[a.b].c", None, json!(4)),
      ],
    );
    assert_eq!(
      out,
      "top=2\nnew=3\n\n[server]\nport = 25565\n; comment\nname=a\n\n[other]\nx=1\n\n[a.b]\nc=4\n"
    );
  }

  #[test]
  fn xml_creates_elements_and_sets_attributes() {
    let out = run(
      ConfigParser::Xml,
      "<Config><Port>1</Port><Ip/></Config>",
      &[
        r("Config.Port", None, json!("25565")),
        r("Config.Ip", None, json!("[value='0.0.0.0']")),
        r("Config.New.Deep", None, json!("x")),
      ],
    );
    let root = xmltree::Element::parse(out.as_bytes()).unwrap();
    assert_eq!(root.get_child("Port").unwrap().get_text().unwrap(), "25565");
    assert_eq!(root.get_child("Ip").unwrap().attributes["value"], "0.0.0.0");
    let deep = root.get_child("New").unwrap().get_child("Deep").unwrap();
    assert_eq!(deep.get_text().unwrap(), "x");

    let created = run(ConfigParser::Xml, "", &[r("Root.Port", None, json!(1))]);
    assert!(created.contains("<Root>"));
  }

  #[test]
  fn files_are_jailed() {
    let root = std::env::temp_dir().join(format!("smaug-jail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink("/etc", root.join("link")).unwrap();

    assert!(jail(&root, "../x").is_err());
    assert!(jail(&root, "link/passwd").is_err());
    assert_eq!(jail(&root, "/a/b.yml").unwrap(), root.join("a/b.yml"));

    let file = ConfigFile {
      file: "config/server.properties".into(),
      parser: ConfigParser::Properties,
      replace: vec![r("port", None, json!(1))],
    };
    let text = ConfigFile {
      file: "missing.txt".into(),
      parser: ConfigParser::File,
      replace: vec![r("a", None, json!("b"))],
    };
    apply(&root, &[file, text], "172.18.0.1");
    assert_eq!(
      std::fs::read_to_string(root.join("config/server.properties")).unwrap(),
      "port=1\n"
    );
    assert!(!root.join("missing.txt").exists());
    std::fs::remove_dir_all(root).unwrap();
  }
}
