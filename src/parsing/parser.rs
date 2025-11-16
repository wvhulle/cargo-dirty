//! Parser combinators for cargo fingerprint log analysis
//!
//! This module uses the nom parser combinator library to parse cargo's
//! fingerprint log output and extract structured rebuild reasons.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{char, digit1, space0},
    combinator::map,
    sequence::{delimited, preceded, terminated, tuple},
};

use super::RebuildReason;
// Parse a quoted string: "hello world"
fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        map(take_until("\""), |s: &str| s.to_string()),
        char('"'),
    )(input)
}

// Parse a number (used for fingerprints)
fn parse_number(input: &str) -> IResult<&str, String> {
    map(digit1, |s: &str| s.to_string())(input)
}

// Parse Option<T>: "Some(value)" or "None"
fn parse_option_string(input: &str) -> IResult<&str, Option<String>> {
    alt((
        map(tag("None"), |_| None),
        map(
            preceded(tag("Some("), terminated(parse_quoted_string, char(')'))),
            Some,
        ),
    ))(input)
}

// Parse field assignment: field_name: value (simplified helper functions)

// Parse comma separator with optional whitespace
fn parse_comma(input: &str) -> IResult<&str, ()> {
    map(tuple((space0, char(','), space0)), |_| ())(input)
}

// Parse EnvVarChanged { name: "CC", old_value: Some("gcc"), new_value: None }
fn parse_env_var_changed(input: &str) -> IResult<&str, RebuildReason> {
    let (input, _) = tag("EnvVarChanged")(input)?;
    let (input, _) = tuple((space0, char('{'), space0))(input)?;

    // Parse name: "value"
    let (input, _) = tuple((tag("name"), space0, char(':'), space0))(input)?;
    let (input, name) = parse_quoted_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse old_value: Option<String>
    let (input, _) = tuple((tag("old_value"), space0, char(':'), space0))(input)?;
    let (input, old_value) = parse_option_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse new_value: Option<String>
    let (input, _) = tuple((tag("new_value"), space0, char(':'), space0))(input)?;
    let (input, new_value) = parse_option_string(input)?;

    let (input, _) = tuple((space0, char('}')))(input)?;

    Ok((
        input,
        RebuildReason::EnvVarChanged {
            name,
            old_value,
            new_value,
        },
    ))
}

// Parse UnitDependencyInfoChanged { old_name: "rusqlite", old_fingerprint: 123,
// new_name: "rusqlite", new_fingerprint: 456 }
fn parse_unit_dependency_info_changed(input: &str) -> IResult<&str, RebuildReason> {
    let (input, _) = tag("UnitDependencyInfoChanged")(input)?;
    let (input, _) = tuple((space0, char('{'), space0))(input)?;

    // Parse old_name: "value"
    let (input, _) = tuple((tag("old_name"), space0, char(':'), space0))(input)?;
    let (input, old_name) = parse_quoted_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse old_fingerprint: number
    let (input, _) = tuple((tag("old_fingerprint"), space0, char(':'), space0))(input)?;
    let (input, old_fingerprint) = parse_number(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse new_name: "value" (but we don't need to store it)
    let (input, _) = tuple((tag("new_name"), space0, char(':'), space0))(input)?;
    let (input, _) = parse_quoted_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse new_fingerprint: number
    let (input, _) = tuple((tag("new_fingerprint"), space0, char(':'), space0))(input)?;
    let (input, new_fingerprint) = parse_number(input)?;

    let (input, _) = tuple((space0, char('}')))(input)?;

    Ok((
        input,
        RebuildReason::UnitDependencyInfoChanged {
            name: old_name,
            old_fingerprint,
            new_fingerprint,
            context: None, // We'll enhance this with context parsing later
        },
    ))
}

// Parse TargetConfigurationChanged
fn parse_target_configuration_changed(input: &str) -> IResult<&str, RebuildReason> {
    let (input, _) = tag("TargetConfigurationChanged")(input)?;
    Ok((input, RebuildReason::TargetConfigurationChanged))
}

// Parse FileTime { seconds: 123, nanos: 456 }
fn parse_file_time(input: &str) -> IResult<&str, (String, String)> {
    let (input, _) = tag("FileTime")(input)?;
    let (input, _) = tuple((space0, char('{'), space0))(input)?;

    // Parse seconds: number
    let (input, _) = tuple((tag("seconds"), space0, char(':'), space0))(input)?;
    let (input, seconds) = parse_number(input)?;
    let (input, ()) = parse_comma(input)?;

    // Parse nanos: number
    let (input, _) = tuple((tag("nanos"), space0, char(':'), space0))(input)?;
    let (input, nanos) = parse_number(input)?;

    let (input, _) = tuple((space0, char('}')))(input)?;

    Ok((input, (seconds, nanos)))
}

// Parse ChangedFile { reference: "...", reference_mtime: FileTime { ... },
// stale: "...", stale_mtime: FileTime { ... } }
fn parse_changed_file(input: &str) -> IResult<&str, String> {
    let (input, _) = tag("ChangedFile")(input)?;
    let (input, _) = tuple((space0, char('{'), space0))(input)?;

    // Skip reference field
    let (input, _) = tuple((tag("reference"), space0, char(':'), space0))(input)?;
    let (input, _) = parse_quoted_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Skip reference_mtime field
    let (input, _) = tuple((tag("reference_mtime"), space0, char(':'), space0))(input)?;
    let (input, _) = parse_file_time(input)?;
    let (input, ()) = parse_comma(input)?;

    // Extract stale path
    let (input, _) = tuple((tag("stale"), space0, char(':'), space0))(input)?;
    let (input, stale_path) = parse_quoted_string(input)?;
    let (input, ()) = parse_comma(input)?;

    // Skip stale_mtime field
    let (input, _) = tuple((tag("stale_mtime"), space0, char(':'), space0))(input)?;
    let (input, _) = parse_file_time(input)?;

    let (input, _) = tuple((space0, char('}')))(input)?;

    Ok((input, stale_path))
}

// Parse FsStatusOutdated(StaleItem(ChangedFile { ... }))
fn parse_fs_status_outdated(input: &str) -> IResult<&str, RebuildReason> {
    let (input, _) = tag("FsStatusOutdated")(input)?;
    let (input, _) = tuple((char('('), tag("StaleItem"), char('(')))(input)?;

    let (input, path) = parse_changed_file(input)?;

    let (input, _) = tuple((char(')'), char(')')))(input)?;

    Ok((input, RebuildReason::FileChanged { path }))
}

// Main parser for dirty reasons
fn parse_dirty_reason_content(input: &str) -> IResult<&str, RebuildReason> {
    alt((
        parse_env_var_changed,
        parse_unit_dependency_info_changed,
        parse_target_configuration_changed,
        parse_fs_status_outdated,
    ))(input)
}

// Parse the simple "stale: changed <path>" pattern using nom
fn parse_stale_changed_content(input: &str) -> IResult<&str, RebuildReason> {
    let (input, _) = tag("stale: changed ")(input)?;
    let (input, path) = parse_quoted_string(input)?;

    Ok((input, RebuildReason::FileChanged { path }))
}

// Parse the full "dirty: <reason>" pattern
#[must_use]
pub fn parse_rebuild_reason(input: &str) -> Option<RebuildReason> {
    // First try to parse "stale: changed" pattern
    if let Some(stale_start) = input.find("stale: changed ") {
        let stale_content = &input[stale_start..];
        if let Ok((_, reason)) = parse_stale_changed_content(stale_content) {
            return Some(reason);
        }
    }

    // Fall back to parsing "dirty:" pattern
    input.find("dirty:").and_then(|dirty_start| {
        let dirty_content = &input[dirty_start + 6..].trim_start();

        match parse_dirty_reason_content(dirty_content) {
            Ok((_, reason)) => Some(reason),
            Err(_) => None,
        }
    })
}
