//! CAS-protected user model Alias bindings.

use super::{
    V3ApiError, check_expectation, parse_mutation_json, providers::provider_contracts_response,
    types::ModelAliasBindingsUpdate,
};
use crate::alias::UserAliasBinding;
use crate::state::CoreState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use chrono::Utc;

pub(super) async fn put_model_alias_bindings(
    State(state): State<CoreState>,
    body: Bytes,
) -> Result<Json<super::types::ProviderContracts>, V3ApiError> {
    let input = parse_mutation_json::<ModelAliasBindingsUpdate>(&body)?;
    let _settings_update = state.settings_update.lock();
    check_expectation(&state, &input.expectation)?;
    let requested = input
        .bindings
        .into_iter()
        .map(|binding| UserAliasBinding {
            alias: binding.alias,
            provider_id: binding.provider_id,
            upstream_model: binding.upstream_model,
        })
        .collect::<Vec<_>>();
    let contracts = state.provider_contracts();
    let validated = crate::model_aliases::validate_user_alias_bindings(&requested, &contracts)
        .map_err(|message| V3ApiError::invalid_request_at(&state, message))?;
    if validated == contracts.user_alias_bindings {
        return provider_contracts_response(&state);
    }
    {
        let db = state.db.lock();
        db.replace_user_model_alias_bindings(&validated, Utc::now())
            .map_err(V3ApiError::internal)?;
        state
            .reload_provider_contracts_locked(&db)
            .map_err(V3ApiError::internal)?;
    }
    state.routing.reset();
    state.bump_settings_revision();
    provider_contracts_response(&state)
}
