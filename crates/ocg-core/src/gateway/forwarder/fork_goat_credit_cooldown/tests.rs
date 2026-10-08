use super::*;

const CREDIT_ERROR: &str = r#"{"error":{"code":"BAD_REQUEST","message":"You have insufficient credits to make this request. Please purchase more credits to continue using the service.","type":"invalid_request_error"}}"#;

fn rejection() -> CreditRejection<'static> {
    CreditRejection {
        status: 400,
        provider_id: COMMAND_CODE_PROVIDER_ID,
        body: CREDIT_ERROR,
        retry_after: None,
        observed_at: Local
            .with_ymd_and_hms(2026, 9, 11, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc),
        observed_mono: Instant::now(),
    }
}

#[test]
fn observed_credit_error_uses_the_saved_natural_month_renewal() {
    let reset = monthly_deadline(&rejection(), "2026-09-01")
        .unwrap()
        .with_timezone(&Local);
    assert_eq!(reset.date_naive().to_string(), "2026-10-01");
    assert_eq!(reset.time().to_string(), "00:00:00");
    let mut r = rejection();
    r.observed_at = Local
        .with_ymd_and_hms(2026, 9, 11, 12, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        monthly_deadline(&r, "2026-08-31")
            .unwrap()
            .with_timezone(&Local)
            .date_naive()
            .to_string(),
        "2026-09-30"
    );
}

#[test]
fn ordinary_errors_other_providers_and_invalid_renewals_do_not_create_a_month_cooldown() {
    for date in ["", "invalid", "2026-08-01", "2026-10-01"] {
        assert!(monthly_deadline(&rejection(), date).is_none());
    }
    let mut r = rejection();
    r.status = 429;
    assert!(monthly_deadline(&r, "2026-09-01").is_none());
    r.status = 400;
    r.provider_id = "opencode";
    assert!(monthly_deadline(&r, "2026-09-01").is_none());
    r.provider_id = COMMAND_CODE_PROVIDER_ID;
    for body in [
        "not json",
        r#"{"error":{"code":"BAD_REQUEST","message":"Invalid base64 data","type":"invalid_request_error"}}"#,
        r#"{"error":{"code":"BAD_REQUEST","message":"You have insufficient credits to make this request.","type":"invalid_request_error"}}"#,
        r#"{"error":{"code":"OTHER","message":"insufficient credits, purchase more credits","type":"invalid_request_error"}}"#,
    ] {
        r.body = body;
        assert!(monthly_deadline(&r, "2026-09-01").is_none());
    }
}
