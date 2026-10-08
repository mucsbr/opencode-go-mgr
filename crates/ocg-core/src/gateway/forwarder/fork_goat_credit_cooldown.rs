//! Fork-only monthly cooldown for the observed GOAT insufficient-credit 400.
//! Its deadline comes from the saved purchase date, not an upstream timestamp.

use super::{
    CoreState, ForwardAttemptContext, LiveSendSelection, RecoveryPermit, live_send,
    plan_window_admission,
};
use crate::gateway::failure::{Cause, FailureFacts, Scope};
use crate::models::{UsageWindowKind, purchase_expires_on};
use crate::provider::COMMAND_CODE_PROVIDER_ID;
use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::time::Instant;

pub(super) struct CreditRejection<'a> {
    pub status: u16,
    pub provider_id: &'a str,
    pub body: &'a str,
    pub retry_after: Option<&'a str>,
    pub observed_at: DateTime<Utc>,
    pub observed_mono: Instant,
}

pub(super) fn observe(
    state: &CoreState,
    selection: &LiveSendSelection,
    permit: &mut RecoveryPermit,
    context: &mut ForwardAttemptContext,
    rejection: CreditRejection<'_>,
) -> Result<()> {
    if !is_credit_error(rejection.status, rejection.provider_id, rejection.body) {
        return Ok(());
    }
    let db = state.db.lock();
    let purchase_date: Option<Option<String>> = db
        .conn
        .query_row(
            "SELECT purchase_date FROM credentials WHERE id = ?1 AND legacy_account_id = ?2",
            [
                selection.credential_id.as_deref().unwrap_or(""),
                selection.account_id.as_str(),
            ],
            |row| row.get(0),
        )
        .optional()?;
    let Some(purchase_date) = purchase_date.flatten() else {
        return Ok(());
    };
    let Some(reset) = monthly_deadline(&rejection, &purchase_date) else {
        return Ok(());
    };
    // Use the same receiving-Key and manual-reset fences as upstream 429s.
    // Do not label the locally derived renewal as an upstream reset fact.
    let facts = FailureFacts {
        cause: Cause::CreditsExhausted,
        scope: Scope::Credential,
        window: Some(UsageWindowKind::Month),
        upstream_reset_at: None,
        retry_not_before: None,
        rule_id: "fork.goat.monthly_credits",
        rule_version: 1,
    };
    let recorded = live_send::selection_identity_is_current(&db, selection)?
        && permit.permits_observation(&facts)
        && live_send::selection_allows_observation(&db, selection)?
        && crate::goat_plan_cooldowns::record_window_on(
            &db.conn,
            selection.credential_id.as_deref().unwrap_or(""),
            &selection.account_id,
            &selection.binding_id,
            selection.credential_version,
            &selection.key_cipher,
            UsageWindowKind::Month,
            reset,
        )?;
    if recorded {
        permit.observe_credential_retry(
            plan_window_admission(reset, rejection.retry_after, rejection.observed_at),
            rejection.observed_mono,
        );
    }
    context.restriction_details = Some(serde_json::json!({
        "facts": facts,
        "local_monthly_reset_at": reset,
        "reset_source": "saved_purchase_date",
        "recorded_for_current_generation": recorded,
    }));
    Ok(())
}

fn monthly_deadline(rejection: &CreditRejection<'_>, purchase_date: &str) -> Option<DateTime<Utc>> {
    if rejection.status != 400 || rejection.provider_id != COMMAND_CODE_PROVIDER_ID {
        return None;
    }
    if !is_credit_error(rejection.status, rejection.provider_id, rejection.body) {
        return None;
    }
    let expires = purchase_expires_on(purchase_date).ok()?;
    let midnight = NaiveDate::parse_from_str(&expires, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)?;
    let reset = Local
        .from_local_datetime(&midnight)
        .single()?
        .with_timezone(&Utc);
    let remaining = reset.signed_duration_since(rejection.observed_at);
    (remaining > Duration::zero() && remaining <= Duration::days(32)).then_some(reset)
}

pub(super) fn is_credit_error(status: u16, provider_id: &str, body: &str) -> bool {
    if status != 400 || provider_id != COMMAND_CODE_PROVIDER_ID {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    let Some(error) = value.get("error").and_then(Value::as_object) else {
        return false;
    };
    let matches = |field: &str, expected: &str| {
        error
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
    };
    let message_matches = error
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|message| {
            let message = message.to_ascii_lowercase();
            message.contains("insufficient credits") && message.contains("purchase more credits")
        });
    matches("code", "BAD_REQUEST") && matches("type", "invalid_request_error") && message_matches
}

#[cfg(test)]
mod tests;
