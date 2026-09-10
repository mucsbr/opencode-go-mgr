import type { Account } from "../api/dashboard.ts";
import type { DynamicProviderView, ModelAliasBindingView } from "../api/providers.ts";
import type { ProviderScopeView } from "./provider-contracts.ts";

export interface ProviderAliasRow {
  key: string;
  public_model: string;
  provider_plan: string;
  custom_account: string | null;
  upstream_model: string;
  routable: boolean;
  custom_account_id: string | null;
  user_defined: boolean;
}

function providerPlanLabel(scope: ProviderScopeView): string {
  return scope.label;
}

/** Built-in and Custom rows remain read-only cross-references. */
export function providerAliasRows(
  scopes: readonly ProviderScopeView[],
  accounts: readonly Account[],
): ProviderAliasRow[] {
  const rows: ProviderAliasRow[] = [];
  const providerRawModels = new Set(
    scopes
      .filter((scope) => scope.scope_kind === "provider")
      .flatMap((scope) => scope.models.map((model) => model.model_id)),
  );
  for (const scope of scopes) {
    if (scope.scope_kind !== "provider") continue;
    for (const model of scope.models) {
      if (!model.alias) continue;
      rows.push({
        key: `${scope.key}:${model.alias}:${model.model_id}`,
        public_model: model.alias,
        provider_plan: providerPlanLabel(scope),
        custom_account: null,
        upstream_model: model.model_id,
        routable: model.routable,
        custom_account_id: null,
        user_defined: false,
      });
    }
  }

  for (const account of accounts) {
    if (account.provider_id !== "custom") continue;
    const scope = scopes.find((candidate) => (
      candidate.scope_kind === "custom_endpoint" && candidate.scope_id === account.id
    ));
    for (const capability of account.model_capabilities) {
      const contract = scope?.models.find((model) => (
        (model.alias || model.model_id).toLocaleLowerCase()
          === capability.public_model.toLocaleLowerCase()
      ));
      const conflictsWithProviderRaw = providerRawModels.has(capability.public_model);
      rows.push({
        key: `custom:${account.id}:${capability.public_model}:${capability.upstream_model}`,
        public_model: capability.public_model,
        provider_plan: scope?.label || "Custom API",
        custom_account: account.name,
        upstream_model: capability.upstream_model,
        routable: !conflictsWithProviderRaw
          && account.enabled
          && account.setup_step === "ready"
          && account.plan_routable
          && Boolean(contract?.routable),
        custom_account_id: account.id,
        user_defined: false,
      });
    }
  }
  return rows;
}

export function dynamicProviderAliasRows(
  providers: readonly DynamicProviderView[],
): ProviderAliasRow[] {
  return providers.flatMap((provider) => provider.models.map((model) => ({
    key: `dynamic:${provider.id}:${model.public_model}:${model.upstream_model}`,
    public_model: model.public_model,
    provider_plan: provider.name,
    custom_account: null,
    upstream_model: model.upstream_model,
    routable: true,
    custom_account_id: null,
    user_defined: false,
  })));
}

export function userProviderAliasRows(
  scopes: readonly ProviderScopeView[],
  bindings: readonly ModelAliasBindingView[],
): ProviderAliasRow[] {
  return bindings.map((binding) => {
    const scope = scopes.find((candidate) => (
      candidate.scope_kind === "provider" && candidate.provider_id === binding.provider_id
    ));
    const model = scope?.models.find((candidate) => candidate.model_id === binding.upstream_model);
    return {
      key: `user:${binding.alias}:${binding.provider_id}:${binding.upstream_model}`,
      public_model: binding.alias,
      provider_plan: scope?.label || binding.provider_id,
      custom_account: null,
      upstream_model: binding.upstream_model,
      routable: Boolean(model?.routable),
      custom_account_id: null,
      user_defined: true,
    };
  });
}

/** Production Alias table plus administrator-confirmed sealed-Provider bindings. */
export function mergeProviderAliasRows(
  scopes: readonly ProviderScopeView[],
  accounts: readonly Account[],
  providers: readonly DynamicProviderView[],
  userBindings: readonly ModelAliasBindingView[] = [],
): ProviderAliasRow[] {
  return [
    ...providerAliasRows(scopes, accounts),
    ...dynamicProviderAliasRows(providers),
    ...userProviderAliasRows(scopes, userBindings),
  ];
}
