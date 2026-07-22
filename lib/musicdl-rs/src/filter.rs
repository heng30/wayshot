//! Filter rule engine for constructing source-specific search parameters.
//!
//! Each source defines its own filter rules. Rules validate user input
//! against declared choices and format the value into a URL parameter string.

use crate::{
    error::{MusicDlError, Result},
    types::FilterValue,
};
use std::collections::HashMap;

/// Type alias for the filter format function to reduce complexity.
type FormatFn = Box<dyn Fn(&FilterValue) -> Result<String> + Send + Sync>;

/// A rule for formatting a filter option into a URL parameter string.
#[allow(clippy::type_complexity)]
pub struct FilterRule {
    name: String,
    format_fn: FormatFn,
}

impl FilterRule {
    /// Create a new filter rule with a custom format function.
    pub fn new<F>(name: impl Into<String>, format_fn: F) -> Self
    where
        F: Fn(&FilterValue) -> Result<String> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            format_fn: Box::new(format_fn),
        }
    }

    /// Create a filter rule that validates the value against a list of string choices,
    /// then formats it using the provided function.
    pub fn with_string_choices<F>(name: impl Into<String>, choices: Vec<&str>, format_fn: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        let name_owned = name.into();
        let choices_owned: Vec<String> = choices.iter().map(|s| s.to_string()).collect();
        Self {
            name: name_owned.clone(),
            format_fn: Box::new(move |v| {
                let s = v.as_str().ok_or_else(|| {
                    MusicDlError::Filter(format!(
                        "Expected string value for filter '{}'",
                        name_owned
                    ))
                })?;
                if !choices_owned.iter().any(|c| c == s) {
                    return Err(MusicDlError::Filter(format!(
                        "Invalid choice '{}' for filter '{}'. Valid choices: {}",
                        s,
                        name_owned,
                        choices_owned.join(", ")
                    )));
                }
                Ok(format_fn(s))
            }),
        }
    }

    /// Create a filter rule that accepts any string value and formats it.
    pub fn with_any_string<F>(name: impl Into<String>, format_fn: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        let name_owned = name.into();
        Self {
            name: name_owned.clone(),
            format_fn: Box::new(move |v| {
                let s = v.as_str().ok_or_else(|| {
                    MusicDlError::Filter(format!(
                        "Expected string value for filter '{}'",
                        name_owned
                    ))
                })?;
                Ok(format_fn(s))
            }),
        }
    }

    /// Get the rule name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Format a filter value using this rule.
    pub fn format(&self, value: &FilterValue) -> Result<String> {
        (self.format_fn)(value)
    }
}

/// A collection of filter rules, applied to user-provided options.
pub struct Filter {
    rules: HashMap<String, FilterRule>,
}

impl Filter {
    /// Create an empty filter with no rules.
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Add a rule to this filter.
    pub fn add_rule(&mut self, rule: FilterRule) {
        self.rules.insert(rule.name().to_string(), rule);
    }

    /// Apply all matching rules to the given filter options, returning
    /// the concatenated filter string.
    ///
    /// Only rules whose names appear in `options` are applied.
    /// The results are joined with the given separator.
    pub fn apply(&self, options: &crate::types::Filters, separator: &str) -> Result<String> {
        let mut parts = Vec::new();
        for (name, value) in options {
            if let Some(rule) = self.rules.get(name) {
                parts.push(rule.format(value)?);
            }
        }
        Ok(parts.join(separator))
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}
