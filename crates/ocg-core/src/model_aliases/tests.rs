use super::*;
use crate::provider::{ProviderAdapterKind, UpstreamProtocolKind};
use crate::provider_contracts::{
    ContractEvidenceSource, ContractScope, EffectiveCatalog, EffectiveModelContract,
    EffectiveProtocolEvidence, EffectiveScopeContract, ProtocolOverrideState,
};
use chrono::Utc;
use std::collections::BTreeMap;

fn scope(provider_id: &str, model_id: &str, enabled: bool) -> EffectiveScopeContract {
    let mut protocols = BTreeMap::new();
    protocols.insert(
        UpstreamProtocolKind::ChatCompletions.as_str().to_string(),
        EffectiveProtocolEvidence {
            protocol: UpstreamProtocolKind::ChatCompletions,
            available: true,
            enabled,
            source: ContractEvidenceSource::ProbeConfirmed,
            verified_at: Some(Utc::now()),
            observed_at: Some(Utc::now()),
            last_probe_result: None,
            last_probe_at: None,
            last_probe_error: None,
            r#override: ProtocolOverrideState::ForceOn,
        },
    );
    let mut models = BTreeMap::new();
    models.insert(
        model_id.to_string(),
        EffectiveModelContract {
            model_id: model_id.to_string(),
            preferred_protocol: UpstreamProtocolKind::ChatCompletions,
            protocols,
            routable: enabled,
            disabled_reasons: Vec::new(),
        },
    );
    EffectiveScopeContract {
        scope: ContractScope::provider(provider_id),
        provider_id: provider_id.to_string(),
        adapter_kind: if provider_id == crate::provider::OPENCODE_PROVIDER_ID {
            ProviderAdapterKind::OpenCodeGo
        } else {
            ProviderAdapterKind::CommandCodeGoat
        },
        catalog_routable: true,
        production_inference: true,
        catalog: EffectiveCatalog {
            source: "test".to_string(),
            source_url: String::new(),
            refreshed_at: Some(Utc::now()),
            models: vec![model_id.to_string()],
            refresh_supported: true,
        },
        models,
        revision: 1,
        fallback_priority: &[UpstreamProtocolKind::ChatCompletions],
        disabled_reasons: Vec::new(),
    }
}

fn contracts() -> EffectiveContractSet {
    let mut contracts = EffectiveContractSet::default();
    contracts.providers.insert(
        crate::provider::OPENCODE_PROVIDER_ID.to_string(),
        scope(
            crate::provider::OPENCODE_PROVIDER_ID,
            "deepseek-flash",
            true,
        ),
    );
    contracts.providers.insert(
        crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
        scope(
            crate::provider::COMMAND_CODE_PROVIDER_ID,
            "deepseek/deepseek-v4.1-flash",
            true,
        ),
    );
    contracts
}

#[test]
fn explicit_cross_provider_bindings_preserve_the_existing_raw_route() {
    let validated = validate_user_alias_bindings(
        &[
            UserAliasBinding {
                alias: "deepseek-flash".to_string(),
                provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
                upstream_model: "deepseek/deepseek-v4.1-flash".to_string(),
            },
            UserAliasBinding {
                alias: "deepseek-flash".to_string(),
                provider_id: crate::provider::OPENCODE_PROVIDER_ID.to_string(),
                upstream_model: "deepseek-flash".to_string(),
            },
        ],
        &contracts(),
    )
    .unwrap();
    assert_eq!(validated.len(), 2);
    assert_eq!(validated[0].provider_id, "command-code");
    assert_eq!(validated[1].provider_id, "opencode");
}

#[test]
fn alias_matching_a_raw_model_requires_its_existing_provider_binding() {
    let error = validate_user_alias_bindings(
        &[UserAliasBinding {
            alias: "deepseek-flash".to_string(),
            provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
            upstream_model: "deepseek/deepseek-v4.1-flash".to_string(),
        }],
        &contracts(),
    )
    .unwrap_err();
    assert!(error.contains("include that binding"));
}

#[test]
fn bindings_require_current_catalog_membership_and_an_enabled_protocol() {
    let mut contracts = contracts();
    contracts.providers.insert(
        crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
        scope(
            crate::provider::COMMAND_CODE_PROVIDER_ID,
            "deepseek/deepseek-v4.1-flash",
            false,
        ),
    );
    let error = validate_user_alias_bindings(
        &[UserAliasBinding {
            alias: "shared-flash".to_string(),
            provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
            upstream_model: "deepseek/deepseek-v4.1-flash".to_string(),
        }],
        &contracts,
    )
    .unwrap_err();
    assert!(error.contains("no enabled protocol"));
}

#[test]
fn unchanged_stale_bindings_can_be_retained_or_deleted() {
    let stale = UserAliasBinding {
        alias: "retired-model".to_string(),
        provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
        upstream_model: "vendor/retired".to_string(),
    };
    let mut contracts = contracts();
    contracts.user_alias_bindings = vec![stale.clone()];
    assert_eq!(
        validate_user_alias_bindings(std::slice::from_ref(&stale), &contracts).unwrap(),
        vec![stale]
    );
    assert!(
        validate_user_alias_bindings(&[], &contracts)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn stale_bindings_remain_persisted_but_do_not_enter_runtime_resolution() {
    let binding = UserAliasBinding {
        alias: "shared-flash".to_string(),
        provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
        upstream_model: "deepseek/deepseek-v4.1-flash".to_string(),
    };
    let mut contracts = contracts();
    contracts.user_alias_bindings = vec![binding.clone()];
    assert_eq!(
        contracts.routeable_user_alias_bindings(),
        vec![binding.clone()]
    );

    contracts.providers.insert(
        crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
        scope(
            crate::provider::COMMAND_CODE_PROVIDER_ID,
            "deepseek/deepseek-v4.1-flash",
            false,
        ),
    );
    assert!(contracts.routeable_user_alias_bindings().is_empty());
    assert_eq!(contracts.user_alias_bindings, vec![binding]);
}

#[test]
fn aliases_use_bounded_lowercase_public_names() {
    for alias in ["DeepSeek", "vendor/model", "-leading", "trailing-"] {
        let error = validate_user_alias_bindings(
            &[UserAliasBinding {
                alias: alias.to_string(),
                provider_id: crate::provider::COMMAND_CODE_PROVIDER_ID.to_string(),
                upstream_model: "deepseek/deepseek-v4.1-flash".to_string(),
            }],
            &contracts(),
        )
        .unwrap_err();
        assert!(error.contains("lowercase kebab/dot"), "{alias}: {error}");
    }
}
