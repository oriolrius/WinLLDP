//! .env files, parsed like python-dotenv (v1) so existing files keep working:
//! unquoted values may contain spaces, `# comments`, `export KEY=...`,
//! 'single' and "double" quotes (with escapes) and ${VAR} / ${VAR:-default}

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The first `.env` in `start` or one of its parents
pub fn find(start: &Path) -> Option<PathBuf> {
    start.ancestors().map(|dir| dir.join(".env")).find(|f| f.is_file())
}

/// Set the variables of `path` that are not already set in the environment
pub fn load(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    for (key, value) in parse(&text, |name| std::env::var(name).ok()) {
        if std::env::var_os(&key).is_none() {
            std::env::set_var(key, value);
        }
    }
    true
}

/// Parse a .env file; `lookup` resolves ${VAR} references not defined earlier in the file
pub fn parse(text: &str, lookup: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut defined: HashMap<String, String> = HashMap::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    let skip_line = |i: &mut usize| {
        while *i < chars.len() && chars[*i] != '\n' {
            *i += 1;
        }
        *i += 1;
    };

    while i < chars.len() {
        // Leading whitespace, blank lines and comments
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '\r') {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        if chars[i] == '\n' || chars[i] == '#' {
            skip_line(&mut i);
            continue;
        }
        let rest: String = chars[i..chars.len().min(i + 7)].iter().collect();
        if rest.starts_with("export") && rest.chars().nth(6).is_some_and(|c| c == ' ' || c == '\t') {
            i += 7;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
        }

        // Key: quoted or up to '=' / whitespace
        let mut key = String::new();
        if i < chars.len() && (chars[i] == '\'' || chars[i] == '"') {
            let quote = chars[i];
            i += 1;
            while i < chars.len() && chars[i] != quote && chars[i] != '\n' {
                key.push(chars[i]);
                i += 1;
            }
            i += 1;
        } else {
            while i < chars.len() && !matches!(chars[i], '=' | '\n' | '\r' | ' ' | '\t' | '#') {
                key.push(chars[i]);
                i += 1;
            }
        }
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if key.is_empty() || i >= chars.len() || chars[i] != '=' {
            // A line without '=' (or malformed): ignored, like python-dotenv
            skip_line(&mut i);
            continue;
        }
        i += 1; // '='
        let value_start = i;
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        let spaced = i > value_start;

        let value;
        let expand;
        if i < chars.len() && chars[i] == '\'' {
            // Single quotes: literal (only \\ and \' are escapes)
            let mut v = String::new();
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                if chars[i] == '\\' && i + 1 < chars.len() && (chars[i + 1] == '\'' || chars[i + 1] == '\\') {
                    i += 1;
                }
                v.push(chars[i]);
                i += 1;
            }
            i += 1;
            value = v;
            expand = false;
            skip_line(&mut i);
        } else if i < chars.len() && chars[i] == '"' {
            let mut v = String::new();
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    match chars[i] {
                        'n' => v.push('\n'),
                        't' => v.push('\t'),
                        'r' => v.push('\r'),
                        'a' => v.push('\x07'),
                        'b' => v.push('\x08'),
                        'f' => v.push('\x0c'),
                        'v' => v.push('\x0b'),
                        '\\' => v.push('\\'),
                        '"' => v.push('"'),
                        '\'' => v.push('\''),
                        other => {
                            v.push('\\');
                            v.push(other);
                        }
                    }
                } else {
                    v.push(chars[i]);
                }
                i += 1;
            }
            i += 1;
            value = v;
            expand = true;
            skip_line(&mut i);
        } else {
            // Unquoted: up to end of line or a " #" comment, trimmed
            let mut v = String::new();
            while i < chars.len() && chars[i] != '\n' {
                if chars[i] == '#' && (v.ends_with([' ', '\t']) || (v.is_empty() && spaced)) {
                    skip_line(&mut i);
                    i -= 1;
                    break;
                }
                v.push(chars[i]);
                i += 1;
            }
            i += 1;
            value = v.trim().to_string();
            expand = true;
        }

        let value = if expand { expand_vars(&value, &defined, &lookup) } else { value };
        defined.insert(key.clone(), value.clone());
        result.retain(|(k, _)| *k != key);
        result.push((key, value));
    }
    result
}

/// ${VAR} and ${VAR:-default}: the environment wins over earlier definitions in the file
fn expand_vars(value: &str, defined: &HashMap<String, String>, lookup: &impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        out.push_str(&rest[..start]);
        let inner = &rest[start + 2..start + end];
        let (name, default) = match inner.split_once(":-") {
            Some((name, default)) => (name, default),
            None => (inner, ""),
        };
        let resolved = lookup(name).or_else(|| defined.get(name).cloned()).unwrap_or_else(|| default.to_string());
        out.push_str(&resolved);
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Vec<(String, String)> {
        parse(text, |name| (name == "FROM_ENV").then(|| "env-value".to_string()))
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn v1_env_example() {
        let text = "# LLDP Configuration\nLLDP_INTERVAL=30\nLLDP_SYSTEM_DESCRIPTION=Windows LLDP Service\n\n\
                    # File Configuration\nLLDP_NEIGHBORS_FILE=neighbors.json\n";
        assert_eq!(
            parsed(text),
            pairs(&[
                ("LLDP_INTERVAL", "30"),
                ("LLDP_SYSTEM_DESCRIPTION", "Windows LLDP Service"),
                ("LLDP_NEIGHBORS_FILE", "neighbors.json")
            ])
        );
    }

    #[test]
    fn quotes_comments_and_export() {
        let text = "export A=1\r\nB = spaced value   # comment\nC='single # not comment'\nD=\"double\\nline\"\n\
                    E=a#b\nF=\nG\nK= #x\nL=#x\n  H=indented\nI=\"multi\nline\"\nJ='it\\'s'\n";
        assert_eq!(
            parsed(text),
            pairs(&[
                ("A", "1"),
                ("B", "spaced value"),
                ("C", "single # not comment"),
                ("D", "double\nline"),
                ("E", "a#b"),
                ("F", ""),
                ("K", ""),
                ("L", "#x"),
                ("H", "indented"),
                ("I", "multi\nline"),
                ("J", "it's"),
            ])
        );
    }

    #[test]
    fn variable_expansion() {
        let text = "BASE=C:\\data\nFILE=${BASE}\\n.json\nQ=\"${FROM_ENV}\"\nS='${BASE}'\nD=${MISSING:-fallback}\nM=${MISSING}x\n";
        assert_eq!(
            parsed(text),
            pairs(&[
                ("BASE", "C:\\data"),
                ("FILE", "C:\\data\\n.json"),
                ("Q", "env-value"),
                ("S", "${BASE}"),
                ("D", "fallback"),
                ("M", "x"),
            ])
        );
    }

    #[test]
    fn later_definitions_win() {
        assert_eq!(parsed("A=1\nA=2\n"), pairs(&[("A", "2")]));
    }
}
