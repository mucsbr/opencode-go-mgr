use super::*;

const FREE: &str = "step-5-preview-free";

#[test]
fn go_free_public_name_keeps_exact_upstream_with_or_without_zen() {
    let go = [FREE.into()];
    for zen in [vec![], vec![FREE.into()]] {
        let catalogs = RuntimeCatalogs {
            go: &go,
            zen_free: &zen,
            ..Default::default()
        };
        let index = RuntimeCatalogIndex::from_catalogs(catalogs);
        let resolved = index.resolve(FREE).unwrap();
        assert!(!resolved.is_pinned());
        let mappings = resolved.routeable_mappings();
        assert_eq!(mappings.len(), 1 + zen.len());
        assert!(mappings.iter().any(|mapping| mapping.is_opencode_go()));
        assert!(
            mappings
                .iter()
                .all(|mapping| mapping.upstream_model == FREE)
        );
        assert!(index.published_models().iter().any(|row| row.alias == FREE));
        assert_eq!(
            catalog_aliases(OPENCODE_PROVIDER_ID, catalogs).get(FREE),
            Some(&FREE.into())
        );
    }
}

#[test]
fn zen_directory_alone_never_authorizes_go_or_changes_its_raw_pin() {
    let zen = [FREE.into()];
    let index = RuntimeCatalogIndex::from_catalogs(RuntimeCatalogs {
        zen_free: &zen,
        ..Default::default()
    });
    let resolved = index.resolve(FREE).unwrap();
    assert!(resolved.is_pinned());
    assert!(
        resolved
            .routeable_mappings()
            .iter()
            .all(|mapping| mapping.is_zen_free())
    );
    assert!(
        index
            .published_models()
            .iter()
            .any(|row| row.alias == "step-5-preview")
    );
    assert!(!index.published_models().iter().any(|row| row.alias == FREE));
}

#[test]
fn full_free_name_never_substitutes_the_paid_model() {
    let go = ["step-5-preview".into(), FREE.into()];
    let zen = [FREE.into()];
    let index = RuntimeCatalogIndex::from_catalogs(RuntimeCatalogs {
        go: &go,
        zen_free: &zen,
        ..Default::default()
    });
    assert!(
        index
            .resolve(FREE)
            .unwrap()
            .routeable_mappings()
            .iter()
            .all(|mapping| mapping.upstream_model == FREE)
    );
    assert!(
        index
            .resolve("step-5-preview")
            .unwrap()
            .routeable_mappings()
            .iter()
            .any(|mapping| mapping.is_opencode_go() && mapping.upstream_model == "step-5-preview")
    );
}

#[test]
fn saved_go_rename_is_preserved_and_unrelated_raw_conflicts_stay_closed() {
    let go = [FREE.into()];
    let overrides = [ExtraProviderCatalog {
        provider_id: OPENCODE_PROVIDER_ID.into(),
        mappings: vec![("my-step-free".into(), FREE.into())],
    }];
    let catalogs = RuntimeCatalogs {
        go: &go,
        builtin_aliases: &overrides,
        ..Default::default()
    };
    let index = RuntimeCatalogIndex::from_catalogs(catalogs);
    assert!(index.resolve(FREE).unwrap().is_pinned());
    assert_eq!(
        catalog_aliases(OPENCODE_PROVIDER_ID, catalogs).get(FREE),
        Some(&"my-step-free".into())
    );
    assert!(!index.published_models().iter().any(|row| row.alias == FREE));
    let custom = [FREE.into()];
    // Custom's same public name deliberately joins an already authorized Alias.
    assert!(matches!(
        RuntimeCatalogIndex::from_catalogs(RuntimeCatalogs {
            go: &go,
            custom: &custom,
            ..Default::default()
        })
        .resolve(FREE),
        Ok(ResolvedModel::Alias { .. })
    ));
    // A raw-only foreign mapping has no shared public-name authorization.
    let extra = [ExtraProviderCatalog {
        provider_id: "foreign".into(),
        mappings: vec![("foreign-step".into(), FREE.into())],
    }];
    assert!(matches!(
        RuntimeCatalogIndex::from_catalogs(RuntimeCatalogs {
            go: &go,
            extra: &extra,
            ..Default::default()
        })
        .resolve(FREE),
        Err(ResolveError::Ambiguous { .. })
    ));
    let foreign = [FREE.into()];
    for catalogs in [
        RuntimeCatalogs {
            go: &go,
            command_code: &foreign,
            ..Default::default()
        },
        RuntimeCatalogs {
            go: &go,
            minimax: &foreign,
            ..Default::default()
        },
    ] {
        let index = RuntimeCatalogIndex::from_catalogs(catalogs);
        assert!(matches!(
            index.resolve(FREE),
            Err(ResolveError::Ambiguous { .. })
        ));
        assert!(!index.published_models().iter().any(|row| row.alias == FREE));
    }
}

#[test]
fn raw_shaped_and_mixed_case_go_free_ids_remain_exact() {
    for id in ["vendor/step-free", "Step-Preview-free"] {
        let go = [id.into()];
        let index = RuntimeCatalogIndex::from_catalogs(RuntimeCatalogs {
            go: &go,
            ..Default::default()
        });
        assert!(index.resolve(id).unwrap().is_pinned());
        assert!(index.resolve(&id.to_ascii_uppercase()).is_err());
    }
}
