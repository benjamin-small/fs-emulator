//! Absolute path parsing shared by every filesystem. Case handling is left to
//! the filesystem; components are returned exactly as written.

use crate::{Error, Result};

fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Split an absolute path into components. `/` parses to an empty vector.
/// Repeated separators collapse; `.` and `..` components are rejected.
pub fn parse(path: &str) -> Result<Vec<String>> {
    if !path.starts_with(is_separator) {
        return Err(Error::InvalidPath);
    }
    let mut parts = Vec::new();
    for component in path.split(is_separator) {
        if component.is_empty() {
            continue;
        }
        if component == "." || component == ".." {
            return Err(Error::InvalidPath);
        }
        parts.push(component.to_string());
    }
    Ok(parts)
}

/// Split into (parent components, final name). The root has no name, so
/// `/` is `InvalidPath`.
pub fn split_parent(path: &str) -> Result<(Vec<String>, String)> {
    let mut parts = parse(path)?;
    let name = parts.pop().ok_or(Error::InvalidPath)?;
    Ok((parts, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn root_parses_to_empty() {
        assert_eq!(parse("/").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn components_are_split_on_either_separator() {
        assert_eq!(parse("/A/B.TXT").unwrap(), vec!["A", "B.TXT"]);
        assert_eq!(parse("\\A\\B.TXT").unwrap(), vec!["A", "B.TXT"]);
        assert_eq!(parse("//A//").unwrap(), vec!["A"]);
    }

    #[test]
    fn rejects_relative_dot_and_empty() {
        assert_eq!(parse("A"), Err(Error::InvalidPath));
        assert_eq!(parse(""), Err(Error::InvalidPath));
        assert_eq!(parse("/./A"), Err(Error::InvalidPath));
        assert_eq!(parse("/../A"), Err(Error::InvalidPath));
    }

    #[test]
    fn split_parent_returns_parent_components_and_name() {
        assert_eq!(
            split_parent("/A/B").unwrap(),
            (vec!["A".to_string()], "B".to_string())
        );
        assert_eq!(split_parent("/B").unwrap(), (vec![], "B".to_string()));
        assert_eq!(split_parent("/"), Err(Error::InvalidPath));
    }
}
