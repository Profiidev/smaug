//! Subset of the Laravel validation rules eggs use for their variables:
//! required, nullable, string, integer, numeric, boolean, min, max, between,
//! in and regex. Unknown rules are ignored.

use regex::{Regex, RegexBuilder};

/// Message for the first rule `value` breaks (`name` is the variable's display
/// name), or `None` when it passes.
pub fn check(name: &str, value: &str, rules: &[String]) -> Option<String> {
  let has = |r: &str| rules.iter().any(|x| x == r);
  // empty values only have to pass `required`, like Laravel's null handling
  if value.trim().is_empty() {
    return has("required").then(|| format!("The {name} field is required."));
  }

  let numeric = has("integer") || has("numeric");
  // ponytail: non-numeric values skip size rules when a numeric rule is set, that rule reports instead
  let size = if numeric {
    value.parse::<f64>().ok()
  } else {
    Some(value.chars().count() as f64)
  };
  let unit = if numeric { "" } else { " characters" };
  let num = |s: &str| s.trim().parse::<f64>().ok();

  for rule in rules {
    let (rule, arg) = rule.split_once(':').unwrap_or((rule, ""));
    let msg = match rule {
      "integer" if value.parse::<i64>().is_err() => format!("The {name} field must be an integer."),
      "numeric" if !value.parse::<f64>().is_ok_and(f64::is_finite) => {
        format!("The {name} field must be a number.")
      }
      "boolean" if !["true", "false", "1", "0"].contains(&value) => {
        format!("The {name} field must be true or false.")
      }
      "min" => match (size, num(arg)) {
        (Some(size), Some(min)) if size < min => {
          format!("The {name} field must be at least {}{unit}.", arg.trim())
        }
        _ => continue,
      },
      "max" => match (size, num(arg)) {
        (Some(size), Some(max)) if size > max => {
          format!(
            "The {name} field must not be greater than {}{unit}.",
            arg.trim()
          )
        }
        _ => continue,
      },
      "between" => {
        let Some((lo, hi)) = arg.split_once(',') else {
          continue;
        };
        match (size, num(lo), num(hi)) {
          (Some(size), Some(min), Some(max)) if size < min || size > max => format!(
            "The {name} field must be between {} and {}{unit}.",
            lo.trim(),
            hi.trim()
          ),
          _ => continue,
        }
      }
      "in" if !arg.split(',').any(|o| o == value) => format!("The selected {name} is invalid."),
      "regex" => match parse_regex(arg) {
        Some(re) if !re.is_match(value) => format!("The {name} field format is invalid."),
        _ => continue,
      },
      _ => continue,
    };
    return Some(msg);
  }
  None
}

/// PCRE style `/pattern/flags`; `None` (rule ignored) when the regex crate
/// can not express it.
// ponytail: lookarounds/backreferences are not supported by the regex crate, those rules are skipped
fn parse_regex(arg: &str) -> Option<Regex> {
  let delim = arg.chars().next()?;
  if delim.is_alphanumeric() || delim == '\\' || delim.is_whitespace() {
    return None;
  }
  let end = arg.rfind(delim)?;
  if end == 0 {
    return None;
  }
  let mut builder = RegexBuilder::new(&arg[delim.len_utf8()..end]);
  for flag in arg[end + delim.len_utf8()..].chars() {
    match flag {
      'i' => builder.case_insensitive(true),
      'm' => builder.multi_line(true),
      's' => builder.dot_matches_new_line(true),
      'x' => builder.ignore_whitespace(true),
      _ => &mut builder,
    };
  }
  builder.build().ok()
}

#[cfg(test)]
mod test {
  use super::*;

  fn run(value: &str, rules: &[&str]) -> Option<String> {
    let rules: Vec<String> = rules.iter().map(|r| r.to_string()).collect();
    check("Port", value, &rules)
  }

  fn ok(value: &str, rules: &[&str]) {
    assert_eq!(run(value, rules), None, "{value:?} {rules:?}");
  }

  fn bad(value: &str, rules: &[&str], msg: &str) {
    assert_eq!(
      run(value, rules).as_deref(),
      Some(msg),
      "{value:?} {rules:?}"
    );
  }

  #[test]
  fn required_and_nullable() {
    bad("", &["required"], "The Port field is required.");
    bad(
      "   ",
      &["required", "string"],
      "The Port field is required.",
    );
    bad("", &["nullable", "required"], "The Port field is required.");
    ok("x", &["required"]);
    // empty and not required skips every other rule
    ok(
      "",
      &["nullable", "integer", "min:5", "in:a,b", "regex:/^x$/"],
    );
    ok("", &[]);
    ok("", &["string", "max:0"]);
  }

  #[test]
  fn types() {
    ok("anything", &["string"]);
    ok("-12", &["integer"]);
    ok("+7", &["integer"]);
    for v in ["1.5", "abc", "1e3", "9999999999999999999999"] {
      bad(v, &["integer"], "The Port field must be an integer.");
    }
    for v in ["1.5", "-2", "1e3", "42"] {
      ok(v, &["numeric"]);
    }
    for v in ["abc", "inf", "NaN", "1,5"] {
      bad(v, &["numeric"], "The Port field must be a number.");
    }
    for v in ["true", "false", "1", "0"] {
      ok(v, &["boolean"]);
    }
    for v in ["yes", "TRUE", "2", "on"] {
      bad(v, &["boolean"], "The Port field must be true or false.");
    }
  }

  #[test]
  fn sizes_on_strings_count_chars() {
    ok("abc", &["min:3"]);
    ok("äöü", &["max:3"]);
    bad(
      "ab",
      &["string", "min:3"],
      "The Port field must be at least 3 characters.",
    );
    bad(
      "abcd",
      &["max:3"],
      "The Port field must not be greater than 3 characters.",
    );
    ok("abc", &["between:1,3"]);
    bad(
      "abcd",
      &["between:1,3"],
      "The Port field must be between 1 and 3 characters.",
    );
    bad(
      "a",
      &["between:2,3"],
      "The Port field must be between 2 and 3 characters.",
    );
    // numbers are strings unless a numeric rule is present
    bad(
      "100",
      &["max:2"],
      "The Port field must not be greater than 2 characters.",
    );
  }

  #[test]
  fn sizes_on_numbers_compare_values() {
    ok("1024", &["integer", "min:1024", "max:65535"]);
    bad(
      "80",
      &["integer", "min:1024"],
      "The Port field must be at least 1024.",
    );
    bad(
      "70000",
      &["numeric", "max:65535"],
      "The Port field must not be greater than 65535.",
    );
    ok("2.5", &["numeric", "between:1,3"]);
    bad(
      "-1",
      &["integer", "between:0,10"],
      "The Port field must be between 0 and 10.",
    );
    // the type rule reports, the size rule is skipped
    bad(
      "abc",
      &["max:5", "integer"],
      "The Port field must be an integer.",
    );
  }

  #[test]
  fn malformed_size_args_are_ignored() {
    ok("abcdef", &["max:x"]);
    ok("abcdef", &["min:"]);
    ok("abcdef", &["between:1"]);
    ok("abcdef", &["between:a,b"]);
  }

  #[test]
  fn in_list() {
    ok("b", &["in:a,b,c"]);
    bad("d", &["in:a,b,c"], "The selected Port is invalid.");
    bad("A", &["in:a"], "The selected Port is invalid.");
    bad("x", &["in:"], "The selected Port is invalid.");
  }

  #[test]
  fn regex_rule() {
    ok("1.20.4", &[r"regex:/^[0-9.]+$/"]);
    bad(
      "latest!",
      &[r"regex:/^[\w.-]+$/"],
      "The Port field format is invalid.",
    );
    ok("ABC", &["regex:/^abc$/i"]);
    bad(
      "ABC",
      &["regex:/^abc$/"],
      "The Port field format is invalid.",
    );
    ok("a|b", &["regex:#^a\\|b$#"]);
    ok("b", &["regex:/^(a|b)$/"]);
    // multi line and dot-all flags
    ok("x\nabc", &["regex:/^abc$/m"]);
    ok("a\nb", &["regex:/^a.b$/s"]);
    // unsupported or malformed patterns are ignored
    for rule in [
      "regex:/(?=a)/",
      "regex:abc",
      "regex:/",
      "regex:",
      "regex:/[/",
    ] {
      ok("zzz", &[rule]);
    }
    assert!(parse_regex("/a/").is_some());
    assert!(parse_regex(" a ").is_none());
  }

  #[test]
  fn unknown_rules_are_ignored_and_first_failure_wins() {
    ok("x", &["alpha_dash", "sometimes", "max_digits:1", "string"]);
    bad(
      "abc",
      &["integer", "in:1,2"],
      "The Port field must be an integer.",
    );
    bad("3", &["in:1,2", "integer"], "The selected Port is invalid.");
  }
}
