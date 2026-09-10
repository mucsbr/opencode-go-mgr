//! Administrator-confirmed cross-Provider model Alias bindings.
//!
//! Provider discovery supplies exact upstream IDs but never claims two IDs are
//! the same model. These rows are therefore manual control-plane data. Runtime
//! resolution consumes only validated bindings and keeps static mappings as
//! the immutable baseline.

use crate::alias::{ResolveError, ResolvedModel, RuntimeCatalogs, UserAliasBinding};
use crate::provider_contracts::{EffectiveContractSet, ollama_cloud_pinned_model_ids};
use std::collections::{HashMap, HashSet};

pub const MAX_USER_ALIAS_BINDINGS: usize = 512;
pub const MAX_USER_ALIASES: usize = 256;

pub fn validate_user_alias_bindings(
    bindings: &[UserAliasBinding],
    contracts: &EffectiveContractSet,
) -> Result<Vec<UserAliasBinding>, String> {
    if bindings.len() > MAX_USER_ALIAS_BINDINGS {
        return Err(format!(
            "at most {MAX_USER_ALIAS_BINDINGS} model Alias bindings are allowed"
        ));
    }

    let mut normalized = Vec::with_capacity(bindings.len());
    let mut keys = HashSet::new();
    let mut aliases: HashMap<String, Vec<UserAliasBinding>> = HashMap::new();
    for binding in bindings {
        let alias = normalize_alias(&binding.alias)?;
        let provider_id = binding.provider_id.trim();
        let upstream_model = binding.upstream_model.trim();
        if provider_id.is_empty() || upstream_model.is_empty() {
            return Err("providerId and upstreamModel are required".to_string());
        }
        let unchanged_existing = contracts.user_alias_bindings.iter().any(|existing| {
            existing.alias == alias
                && existing.provider_id == provider_id
                && existing.upstream_model == upstream_model
        });
        let contract = contracts
            .provider_offering(provider_id)
            .ok_or_else(|| format!("unknown or unsupported Provider `{provider_id}`"))?;
        let catalog_model = contract
            .catalog
            .models
            .iter()
            .find(|model| model.trim() == upstream_model)
            .cloned()
            .or_else(|| unchanged_existing.then(|| upstream_model.to_string()))
            .ok_or_else(|| {
                format!(
                    "upstream model `{upstream_model}` is not in the current `{provider_id}` catalog"
                )
            })?;
        if !unchanged_existing && !contract.model_has_enabled_protocol(&catalog_model) {
            return Err(format!(
                "upstream model `{catalog_model}` has no enabled protocol for `{provider_id}`"
            ));
        }
        let key = (alias.clone(), provider_id.to_string());
        if !keys.insert(key) {
            return Err(format!(
                "Alias `{alias}` has more than one binding for Provider `{provider_id}`"
            ));
        }
        let binding = UserAliasBinding {
            alias: alias.clone(),
            provider_id: provider_id.to_string(),
            upstream_model: catalog_model,
        };
        aliases.entry(alias).or_default().push(binding.clone());
        normalized.push(binding);
    }
    if aliases.len() > MAX_USER_ALIASES {
        return Err(format!(
            "at most {MAX_USER_ALIASES} model Aliases are allowed"
        ));
    }

    let models = |provider_id: &str| {
        contracts
            .provider_offering(provider_id)
            .map(|scope| scope.catalog.models.as_slice())
            .unwrap_or_default()
    };
    let ollama_pinned = ollama_cloud_pinned_model_ids(contracts);
    let catalogs = RuntimeCatalogs {
        go: models(crate::provider::OPENCODE_PROVIDER_ID),
        zen_free: models(crate::provider::OPENCODE_ZEN_FREE_PROVIDER_ID),
        command_code: models(crate::provider::COMMAND_CODE_PROVIDER_ID),
        minimax: models(crate::provider::MINIMAX_PROVIDER_ID),
        kimi: models(crate::provider::KIMI_PROVIDER_ID),
        ollama: models(crate::provider::OLLAMA_PROVIDER_ID),
        ollama_pinned: &ollama_pinned,
        ..RuntimeCatalogs::default()
    };
    for (alias, group) in &aliases {
        match crate::alias::resolve_with_runtime_catalogs(alias, catalogs) {
            Ok(ResolvedModel::Alias { mappings, .. }) => {
                for binding in group {
                    let unchanged_existing = contracts
                        .user_alias_bindings
                        .iter()
                        .any(|existing| existing == binding);
                    if let Some(existing) = mappings
                        .iter()
                        .find(|mapping| mapping.provider_id == binding.provider_id)
                    {
                        if existing.upstream_model == binding.upstream_model && unchanged_existing {
                            continue;
                        }
                        if existing.upstream_model == binding.upstream_model {
                            return Err(format!(
                                "Alias `{alias}` already contains the built-in Provider `{}` mapping",
                                binding.provider_id
                            ));
                        }
                        return Err(format!(
                            "Alias `{alias}` already maps Provider `{}` to `{}`",
                            binding.provider_id, existing.upstream_model
                        ));
                    }
                }
            }
            Ok(ResolvedModel::PinnedRaw { mapping, .. }) => {
                if !group.iter().any(|binding| {
                    binding.provider_id == mapping.provider_id
                        && binding.upstream_model == mapping.upstream_model
                }) {
                    return Err(format!(
                        "Alias `{alias}` is already the raw model `{}` for Provider `{}`; include that binding to preserve its existing route",
                        mapping.upstream_model, mapping.provider_id
                    ));
                }
            }
            Err(ResolveError::Ambiguous { .. }) => {
                return Err(format!(
                    "Alias `{alias}` conflicts with more than one existing raw Provider model"
                ));
            }
            Err(ResolveError::Unknown { .. }) => {}
        }
    }

    normalized.sort_by(|left, right| {
        left.alias
            .cmp(&right.alias)
            .then(left.provider_id.cmp(&right.provider_id))
    });
    Ok(normalized)
}

fn normalize_alias(value: &str) -> Result<String, String> {
    let alias = value.trim();
    if alias.is_empty() || alias.len() > 128 {
        return Err("Alias must contain between 1 and 128 bytes".to_string());
    }
    if alias != alias.to_ascii_lowercase()
        || !alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'))
        || !alias
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        || !alias
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_alphanumeric())
    {
        return Err(
            "Alias must be lowercase kebab/dot notation and start and end with a letter or digit"
                .to_string(),
        );
    }
    Ok(alias.to_string())
}

#[cfg(test)]
mod tests;
