//! Build identity helpers.

pub const BASE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn channel() -> &'static str {
    non_empty(option_env!("HERDR_BUILD_CHANNEL")).unwrap_or("stable")
}

pub fn build_id() -> Option<&'static str> {
    non_empty(option_env!("HERDR_BUILD_ID"))
}

pub fn version() -> String {
    match channel() {
        "stable" => BASE_VERSION.to_string(),
        channel => match build_id() {
            Some(build_id) => format!("{BASE_VERSION}-{channel}.{build_id}"),
            None => format!("{BASE_VERSION}-{channel}"),
        },
    }
}

pub fn is_preview() -> bool {
    channel() == "preview"
}

/// Human-readable provenance without changing upstream release/channel identity
/// used by endpoint negotiation and remote asset selection.
pub fn display_version() -> String {
    let version = version();
    match non_empty(option_env!("HERDR_FORK_ID")) {
        Some(fork) => format!(
            "{version} (fork {fork}; commit {}; built {})",
            non_empty(option_env!("HERDR_FORK_REVISION")).unwrap_or("unknown"),
            fork_build_time().unwrap_or_else(|| "unknown".into())
        ),
        None => version,
    }
}

fn fork_build_time() -> Option<String> {
    let epoch = non_empty(option_env!("HERDR_FORK_BUILD_EPOCH"))?
        .parse()
        .ok()?;
    time::OffsetDateTime::from_unix_timestamp(epoch)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

fn non_empty(value: Option<&'static str>) -> Option<&'static str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn stable_version_defaults_to_cargo_version() {
        assert!(!super::version().is_empty());
    }
}
