//! Command Code GOAT inference quota-exhaustion parsing.
//!
//! The Provider API reports plan-window exhaustion as a standard 429 error
//! envelope. Unlike OpenCode Go, the exact reset is an RFC3339 timestamp in
//! the message. Keep this parser provider-specific so unrelated upstream 429s
//! cannot extend an account cooldown from lookalike text.
//!
//! Exhausted monthly credits are different: Command Code returns a structured
//! 400 without a reset timestamp. That response is strong enough to rotate the
//! account, while the saved purchase date supplies the local renewal boundary.

use crate::models::{UsageWindowKind, purchase_expires_on};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use serde_json::Value;

const CLOCK_SKEW_ALLOWANCE: Duration = Duration::minutes(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandCodeRateLimit {
    pub(crate) window: UsageWindowKind,
    pub(crate) resets_at: DateTime<Utc>,
}

pub(crate) fn parse_command_code_rate_limit(
    text: &str,
    observed_at: DateTime<Utc>,
) -> Option<CommandCodeRateLimit> {
    let value: Value = serde_json::from_str(text).ok()?;
    let error = value.get("error")?.as_object()?;
    let code_matches = error
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| code.eq_ignore_ascii_case("RATE_LIMITED"));
    let type_matches = error
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("rate_limit_error"));
    if !code_matches && !type_matches {
        return None;
    }

    let message = error.get("message")?.as_str()?;
    let message_lower = message.to_ascii_lowercase();
    let (window, maximum) = if message_lower.contains("5-hour usage limit for your plan")
        || message_lower.contains("5 hour usage limit for your plan")
    {
        (UsageWindowKind::FiveHours, Duration::hours(5))
    } else if message_lower.contains("weekly usage limit for your plan") {
        (UsageWindowKind::Week, Duration::days(7))
    } else {
        return None;
    };

    let resets_at = parse_reset_timestamp(message, &message_lower)?;
    let remaining = resets_at.signed_duration_since(observed_at);
    if remaining <= Duration::zero() || remaining > maximum + CLOCK_SKEW_ALLOWANCE {
        return None;
    }

    Some(CommandCodeRateLimit { window, resets_at })
}

pub(crate) fn is_command_code_insufficient_credits(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let Some(error) = value.get("error").and_then(Value::as_object) else {
        return false;
    };
    let code_matches = error
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| code.eq_ignore_ascii_case("BAD_REQUEST"));
    let type_matches = error
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("invalid_request_error"));
    let message_matches = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .is_some_and(|message| {
            message.contains("insufficient credits") && message.contains("purchase more credits")
        });
    code_matches && type_matches && message_matches
}

pub(crate) fn command_code_monthly_reset(
    purchase_date: &str,
    observed_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let expires_on = purchase_expires_on(purchase_date).ok()?;
    let local_midnight = NaiveDate::parse_from_str(&expires_on, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)?;
    let resets_at = Local
        .from_local_datetime(&local_midnight)
        .single()?
        .with_timezone(&Utc);
    let remaining = resets_at.signed_duration_since(observed_at);
    (remaining > Duration::zero() && remaining <= Duration::days(32)).then_some(resets_at)
}

fn parse_reset_timestamp(message: &str, message_lower: &str) -> Option<DateTime<Utc>> {
    const MARKER: &str = "your limit resets at ";
    let start = message_lower.find(MARKER)? + MARKER.len();
    let candidate = message.get(start..)?.split_whitespace().next()?;
    let candidate = candidate.trim_end_matches(['.', ',', ';']);
    DateTime::parse_from_rfc3339(candidate)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests;
