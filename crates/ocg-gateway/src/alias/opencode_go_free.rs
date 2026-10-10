//! Go directory membership, not the `-free` suffix, authorizes the keyed route.
//!
//! These exact kebab public names can share an identical saved Zen row. Other
//! raw conflicts stay closed unless they already declare that public name.
//! The shorter Zen alias stays unchanged, and no paid ID is substituted.

use super::*;

pub(super) fn insert_catalog(registry: &mut Registry, catalogs: RuntimeCatalogs<'_>) {
    for id in catalogs.go {
        if !is_free_model(id) || looks_raw_shaped(id) || *id != id.to_ascii_lowercase() {
            continue;
        }
        insert_raw_mapping(registry, go_mapping(id));
        let unrelated_raw = registry.raw_exact[id].iter().any(|mapping| {
            !mapping.is_opencode_go()
                && !mapping.is_zen_free()
                && !registry
                    .aliases
                    .get(id)
                    .is_some_and(|entry| entry.mappings.contains(mapping))
        });
        if unrelated_raw {
            continue;
        }
        insert_mapping(registry, id, go_mapping(id));
        if catalogs.zen_free.contains(id) {
            insert_mapping(registry, id, zen_mapping(id));
        }
    }
}

#[cfg(test)]
mod tests;
