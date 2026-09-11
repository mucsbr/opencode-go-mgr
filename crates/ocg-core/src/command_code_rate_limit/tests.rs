use super::*;

fn at(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn parses_observed_five_hour_and_weekly_provider_errors() {
    let five_hour = r#"{"error":{"code":"RATE_LIMITED","message":"You've reached your 5-hour usage limit for your plan. Your limit resets at 2026-09-03T05:25:42.353Z. Please wait for the window to reset or upgrade your plan to continue.","type":"rate_limit_error"}}"#;
    assert_eq!(
        parse_command_code_rate_limit(five_hour, at("2026-09-03T02:59:17.598Z")),
        Some(CommandCodeRateLimit {
            window: UsageWindowKind::FiveHours,
            resets_at: at("2026-09-03T05:25:42.353Z"),
        })
    );

    let weekly = r#"{"error":{"code":"RATE_LIMITED","message":"You've reached your weekly usage limit for your plan. Your limit resets at 2026-09-08T09:56:18.379Z. Please wait for the window to reset or upgrade your plan to continue.","type":"rate_limit_error"}}"#;
    assert_eq!(
        parse_command_code_rate_limit(weekly, at("2026-09-03T08:18:11.018Z")),
        Some(CommandCodeRateLimit {
            window: UsageWindowKind::Week,
            resets_at: at("2026-09-08T09:56:18.379Z"),
        })
    );
}

#[test]
fn accepts_anthropic_shaped_error_without_a_code() {
    let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"You've reached your weekly usage limit for your plan. Your limit resets at 2026-09-08T09:56:18.379Z. Please wait for the window to reset."}}"#;
    assert_eq!(
        parse_command_code_rate_limit(body, at("2026-09-08T06:27:42.402Z"))
            .map(|limit| limit.window),
        Some(UsageWindowKind::Week)
    );
}

#[test]
fn rejects_transient_malformed_expired_and_unbounded_rate_limits() {
    let transient = r#"{"error":{"code":"RATE_LIMITED","message":"Upstream provider is rate limited. Please retry.","type":"rate_limit_error"}}"#;
    assert!(parse_command_code_rate_limit(transient, at("2026-09-03T02:59:17.598Z")).is_none());

    let wrong_kind = r#"{"error":{"code":"OTHER","message":"You've reached your weekly usage limit for your plan. Your limit resets at 2026-09-08T09:56:18.379Z.","type":"server_error"}}"#;
    assert!(parse_command_code_rate_limit(wrong_kind, at("2026-09-08T06:27:42.402Z")).is_none());

    let expired = r#"{"error":{"code":"RATE_LIMITED","message":"You've reached your 5-hour usage limit for your plan. Your limit resets at 2026-09-03T05:25:42.353Z.","type":"rate_limit_error"}}"#;
    assert!(parse_command_code_rate_limit(expired, at("2026-09-03T05:25:42.354Z")).is_none());

    let beyond_week = r#"{"error":{"code":"RATE_LIMITED","message":"You've reached your weekly usage limit for your plan. Your limit resets at 2026-09-10T00:00:00Z.","type":"rate_limit_error"}}"#;
    assert!(parse_command_code_rate_limit(beyond_week, at("2026-09-02T00:00:00Z")).is_none());
}

#[test]
fn recognizes_only_the_observed_insufficient_credits_envelope() {
    let observed = r#"{"error":{"code":"BAD_REQUEST","message":"You have insufficient credits to make this request. Please purchase more credits to continue using the service.","type":"invalid_request_error"}}"#;
    assert!(is_command_code_insufficient_credits(observed));

    for rejected in [
        r#"{"error":{"code":"BAD_REQUEST","message":"Invalid base64 data","type":"invalid_request_error"}}"#,
        r#"{"error":{"code":"OTHER","message":"You have insufficient credits to make this request. Please purchase more credits.","type":"invalid_request_error"}}"#,
        r#"{"error":{"code":"BAD_REQUEST","message":"You have insufficient credits to make this request. Please purchase more credits.","type":"rate_limit_error"}}"#,
        r#"{"error":{"code":"BAD_REQUEST","message":"You have insufficient credits to make this request.","type":"invalid_request_error"}}"#,
        "not json",
    ] {
        assert!(
            !is_command_code_insufficient_credits(rejected),
            "{rejected}"
        );
    }
}

#[test]
fn derives_a_bounded_monthly_reset_from_the_saved_purchase_date() {
    let observed_at = at("2026-09-11T02:51:52Z");
    let resets_at = command_code_monthly_reset("2026-09-01", observed_at)
        .expect("the current billing month should have a future renewal");
    let local_reset = resets_at.with_timezone(&Local);
    assert_eq!(local_reset.date_naive().to_string(), "2026-10-01");
    assert_eq!(local_reset.time().to_string(), "00:00:00");

    assert!(command_code_monthly_reset("", observed_at).is_none());
    assert!(command_code_monthly_reset("2026-08-01", observed_at).is_none());
    assert!(command_code_monthly_reset("2026-10-01", observed_at).is_none());
}
