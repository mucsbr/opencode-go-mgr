<template>
  <div class="aliases-page">
    <header class="aliases-header">
      <div>
        <h1>{{ t("别名") }}</h1>
        <p>{{ t("汇总当前供应商合同、Custom 账号映射与人工确认的跨供应商绑定。") }}</p>
      </div>
      <n-button type="primary" :disabled="!contracts" @click="openNewAlias">
        {{ t("配置 Alias") }}
      </n-button>
    </header>

    <div
      v-if="initialLoading"
      class="aliases-state"
      role="status"
      aria-live="polite"
      :aria-label="t('加载中…')"
    >
      <n-spin size="small" />
    </div>

    <n-alert
      v-else-if="loadError && !contracts"
      type="error"
      :title="t('加载供应商失败: {error}', { error: loadError })"
    >
      <n-button size="small" secondary :loading="loading" @click="loadAliases()">
        {{ t("重试") }}
      </n-button>
    </n-alert>

    <section v-else class="aliases-section" aria-labelledby="alias-table-title">
      <h2 id="alias-table-title" class="sr-only">{{ t("别名") }}</h2>
      <n-alert
        v-if="loadError && contracts"
        type="warning"
        :title="t('加载供应商失败: {error}', { error: loadError })"
      >
        <n-button size="small" secondary :loading="loading" @click="loadAliases({ retain: true })">
          {{ t("重试") }}
        </n-button>
      </n-alert>
      <n-alert
        v-if="accountsLoadError"
        type="warning"
        :title="t('加载 Custom Alias 账号失败: {error}', { error: accountsLoadError })"
      >
        <n-button size="small" secondary :loading="loading" @click="loadAliases({ retain: true })">
          {{ t("重试") }}
        </n-button>
      </n-alert>
      <n-alert
        v-if="dynamicLoadError"
        type="warning"
        :title="t('加载供应商失败: {error}', { error: dynamicLoadError })"
      >
        <n-button size="small" secondary :loading="loading" @click="loadAliases({ retain: true })">
          {{ t("重试") }}
        </n-button>
      </n-alert>

      <n-empty v-if="aliasGroups.length === 0" :description="t('暂无 Alias')" />
      <div v-else class="aliases-table-wrap">
        <table class="aliases-table">
          <thead>
            <tr>
              <th>{{ t("对外模型名") }}</th>
              <th>{{ t("供应商 / 方案") }}</th>
              <th>{{ t("上游模型 ID") }}</th>
              <th>{{ t("可路由") }}</th>
              <th>{{ t("操作") }}</th>
            </tr>
          </thead>
          <tbody v-for="group in aliasGroups" :key="group.public_model">
            <tr v-for="(row, index) in group.rows" :key="row.key">
              <td v-if="index === 0" :rowspan="group.rows.length" class="aliases-name">
                <code>{{ group.public_model }}</code>
              </td>
              <td>{{ row.provider_plan }}</td>
              <td><code>{{ row.upstream_model }}</code></td>
              <td>{{ row.routable ? t("可用") : t("不可用") }}</td>
              <td v-if="index === 0" :rowspan="group.rows.length" class="aliases-actions">
                <n-button
                  v-if="configuredAliases.has(group.public_model.toLocaleLowerCase())"
                  size="small"
                  secondary
                  @click="openEditAlias(group.public_model)"
                >
                  {{ t("编辑") }}
                </n-button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <n-modal
      v-model:show="editorOpen"
      preset="card"
      :title="editingAlias ? t('编辑 Alias') : t('配置 Alias')"
      :style="{ width: 'min(720px, calc(100vw - 32px))' }"
      :mask-closable="!saving"
    >
      <n-alert v-if="editorError" type="error" :title="editorError" class="aliases-editor-error" />
      <n-form-item :label="t('对外模型名')" required>
        <n-input
          v-model:value="aliasDraft"
          :placeholder="t('例如：deepseek-flash')"
          :disabled="saving"
        />
      </n-form-item>
      <div class="aliases-editor-bindings">
        <div class="aliases-editor-label">{{ t("供应商映射") }}</div>
        <div v-for="(binding, index) in bindingDrafts" :key="index" class="aliases-editor-row">
          <n-select
            v-model:value="binding.provider_id"
            :options="providerOptions"
            :placeholder="t('选择供应商')"
            :disabled="saving"
            @update:value="binding.upstream_model = ''"
          />
          <n-select
            v-model:value="binding.upstream_model"
            :options="modelOptions(binding.provider_id)"
            :placeholder="t('选择上游模型')"
            filterable
            :disabled="saving || !binding.provider_id"
          />
          <n-button
            quaternary
            type="error"
            :disabled="saving || bindingDrafts.length <= 1"
            @click="removeBinding(index)"
          >
            {{ t("移除") }}
          </n-button>
        </div>
        <n-button size="small" secondary :disabled="saving" @click="addBinding">
          {{ t("添加供应商映射") }}
        </n-button>
      </div>
      <template #footer>
        <div class="aliases-editor-footer">
          <n-popconfirm
            v-if="editingAlias"
            :positive-text="t('删除')"
            :negative-text="t('取消')"
            @positive-click="deleteEditingAlias"
          >
            <template #trigger>
              <n-button type="error" secondary :disabled="saving">{{ t("删除 Alias") }}</n-button>
            </template>
            {{ t("确定删除这个人工 Alias 绑定吗？内置 Alias 不受影响。") }}
          </n-popconfirm>
          <span class="aliases-editor-spacer" />
          <n-button :disabled="saving" @click="editorOpen = false">{{ t("取消") }}</n-button>
          <n-button type="primary" :loading="saving" :disabled="!canSaveAlias" @click="saveAlias">
            {{ t("保存") }}
          </n-button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, onActivated, onMounted, ref } from "vue";
import {
  NAlert,
  NButton,
  NEmpty,
  NFormItem,
  NInput,
  NModal,
  NPopconfirm,
  NSelect,
  NSpin,
} from "naive-ui";
import type { Account } from "../api/dashboard.ts";
import type {
  DynamicProviderView,
  ProviderCatalogEntry,
  ProviderContractsResponse,
} from "../api/providers.ts";
import { providerApi } from "../api/providers.ts";
import { isDynamicCatalogEntry } from "../domain/dynamic-provider.ts";
import { flattenProviderScopes, normalizeProviderContractsResponse } from "../domain/provider-contracts.ts";
import { mergeProviderAliasRows } from "../domain/provider-aliases.ts";
import { t } from "../i18n/index.ts";
import { useAccountsStore } from "../stores/accounts.ts";
import { useProvidersStore } from "../stores/providers.ts";
import { dashboardErrorDetail } from "../utils/errors.ts";

type AliasBindingDraft = { provider_id: string; upstream_model: string };

const accountsStore = useAccountsStore();
const providersStore = useProvidersStore();
const contracts = ref<ProviderContractsResponse | null>(null);
const catalog = ref<ProviderCatalogEntry[] | null>(null);
const accounts = ref<Account[]>([]);
const dynamicProviders = ref<DynamicProviderView[]>([]);
const loading = ref(false);
const loadError = ref("");
const accountsLoadError = ref("");
const dynamicLoadError = ref("");
const editorOpen = ref(false);
const editingAlias = ref("");
const aliasDraft = ref("");
const bindingDrafts = ref<AliasBindingDraft[]>([]);
const editorError = ref("");
const saving = ref(false);
let activatedOnce = false;

const initialLoading = computed(() => loading.value && !contracts.value);
const scopes = computed(() => (
  contracts.value ? flattenProviderScopes(contracts.value, catalog.value) : []
));
const aliasRows = computed(() => (
  contracts.value
    ? mergeProviderAliasRows(
      scopes.value,
      accounts.value,
      dynamicProviders.value,
      contracts.value.alias_bindings,
    )
    : []
));
const aliasGroups = computed(() => {
  const groups = new Map<string, typeof aliasRows.value>();
  for (const row of aliasRows.value) {
    const key = row.public_model.toLocaleLowerCase();
    const existing = groups.get(key);
    if (existing) existing.push(row);
    else groups.set(key, [row]);
  }
  return [...groups.values()]
    .map((rows) => ({ public_model: rows[0]?.public_model ?? "", rows }))
    .sort((left, right) => left.public_model.localeCompare(right.public_model));
});
const configuredAliases = computed(() => new Set(
  (contracts.value?.alias_bindings ?? []).map((binding) => binding.alias.toLocaleLowerCase()),
));
const providerScopes = computed(() => scopes.value.filter((scope) => scope.scope_kind === "provider"));
const providerOptions = computed(() => providerScopes.value
  .filter((scope) => scope.models.some((model) => model.routable))
  .map((scope) => ({ label: scope.label, value: scope.provider_id })));
const aliasPattern = /^[a-z0-9](?:[a-z0-9.-]{0,126}[a-z0-9])?$/;
const canSaveAlias = computed(() => {
  const alias = aliasDraft.value.trim();
  if (!aliasPattern.test(alias) || bindingDrafts.value.length === 0) return false;
  const providers = new Set<string>();
  for (const binding of bindingDrafts.value) {
    if (!binding.provider_id || !binding.upstream_model || providers.has(binding.provider_id)) {
      return false;
    }
    providers.add(binding.provider_id);
  }
  return true;
});

async function loadAliases(options: { retain?: boolean } = {}): Promise<void> {
  if (loading.value) return;
  loading.value = true;
  if (!options.retain) {
    loadError.value = "";
    dynamicLoadError.value = "";
  }
  try {
    const [contractsResult, catalogResult, accountsResult] = await Promise.allSettled([
      providersStore.loadContracts(),
      providersStore.loadCatalog(),
      accountsStore.loadPresented(),
    ]);
    if (catalogResult.status === "fulfilled") {
      catalog.value = catalogResult.value;
      const entries = catalogResult.value.filter(isDynamicCatalogEntry);
      if (entries.length === 0) {
        dynamicProviders.value = [];
        dynamicLoadError.value = "";
      } else {
        const details = await Promise.allSettled(
          entries.map((entry) => providerApi.getDynamicProvider(entry.provider_id)),
        );
        const previous = new Map(dynamicProviders.value.map((provider) => [provider.id, provider]));
        const next: DynamicProviderView[] = [];
        const failures: string[] = [];
        details.forEach((result, index) => {
          if (result.status === "fulfilled") {
            next.push(result.value);
            return;
          }
          failures.push(dashboardErrorDetail(result.reason));
          if (options.retain) {
            const kept = previous.get(entries[index]?.provider_id ?? "");
            if (kept) next.push(kept);
          }
        });
        dynamicProviders.value = next;
        dynamicLoadError.value = failures[0] ?? "";
      }
    }
    if (accountsResult.status === "fulfilled") {
      accounts.value = accountsResult.value;
      accountsLoadError.value = "";
    } else {
      accountsLoadError.value = dashboardErrorDetail(accountsResult.reason);
    }
    if (contractsResult.status === "fulfilled") {
      contracts.value = normalizeProviderContractsResponse(contractsResult.value);
      loadError.value = "";
    } else {
      loadError.value = dashboardErrorDetail(contractsResult.reason);
    }
  } finally {
    loading.value = false;
  }
}

function modelOptions(providerId: string): Array<{ label: string; value: string }> {
  const scope = providerScopes.value.find((candidate) => candidate.provider_id === providerId);
  return (scope?.models ?? [])
    .filter((model) => model.routable)
    .map((model) => ({ label: model.model_id, value: model.model_id }));
}

function openNewAlias(): void {
  editingAlias.value = "";
  aliasDraft.value = "";
  bindingDrafts.value = [{ provider_id: "", upstream_model: "" }];
  editorError.value = "";
  editorOpen.value = true;
}

function openEditAlias(alias: string): void {
  editingAlias.value = alias;
  aliasDraft.value = alias;
  bindingDrafts.value = (contracts.value?.alias_bindings ?? [])
    .filter((binding) => binding.alias.toLocaleLowerCase() === alias.toLocaleLowerCase())
    .map((binding) => ({
      provider_id: binding.provider_id,
      upstream_model: binding.upstream_model,
    }));
  editorError.value = "";
  editorOpen.value = true;
}

function addBinding(): void {
  bindingDrafts.value.push({ provider_id: "", upstream_model: "" });
}

function removeBinding(index: number): void {
  bindingDrafts.value.splice(index, 1);
}

async function saveAlias(): Promise<void> {
  if (!contracts.value || !canSaveAlias.value || saving.value) return;
  saving.value = true;
  editorError.value = "";
  try {
    const previous = editingAlias.value.toLocaleLowerCase();
    const retained = contracts.value.alias_bindings.filter((binding) => (
      !previous || binding.alias.toLocaleLowerCase() !== previous
    ));
    const alias = aliasDraft.value.trim();
    contracts.value = await providerApi.updateModelAliasBindings([
      ...retained,
      ...bindingDrafts.value.map((binding) => ({ alias, ...binding })),
    ]);
    editorOpen.value = false;
  } catch (cause) {
    editorError.value = dashboardErrorDetail(cause);
  } finally {
    saving.value = false;
  }
}

async function deleteEditingAlias(): Promise<void> {
  if (!contracts.value || !editingAlias.value || saving.value) return;
  saving.value = true;
  editorError.value = "";
  try {
    const removed = editingAlias.value.toLocaleLowerCase();
    contracts.value = await providerApi.updateModelAliasBindings(
      contracts.value.alias_bindings.filter((binding) => (
        binding.alias.toLocaleLowerCase() !== removed
      )),
    );
    editorOpen.value = false;
  } catch (cause) {
    editorError.value = dashboardErrorDetail(cause);
  } finally {
    saving.value = false;
  }
}

onMounted(() => void loadAliases());
onActivated(() => {
  if (activatedOnce) void loadAliases({ retain: true });
  else activatedOnce = true;
});
</script>

<style scoped>
.aliases-page {
  min-width: 0;
  max-width: 1440px;
  margin: 0 auto;
  overflow-x: hidden;
}
.aliases-header {
  margin-bottom: 16px;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.aliases-header h1 {
  margin: 0;
  color: var(--ocg-ink);
  font: 700 var(--ocg-font-xl)/1.3 "Bahnschrift", "Segoe UI Variable Display", sans-serif;
}
.aliases-header p {
  margin: 4px 0 0;
  color: var(--ocg-muted);
  font-size: var(--ocg-font-sm);
}
.aliases-state {
  min-height: 160px;
  display: grid;
  place-items: center;
}
.aliases-section {
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--ocg-border);
  border-radius: 14px;
  background: var(--ocg-surface);
  box-shadow: var(--ocg-shadow-sm);
}
.aliases-section > .n-alert {
  margin-bottom: 12px;
}
.aliases-table-wrap {
  overflow-x: auto;
}
.aliases-table {
  width: 100%;
  min-width: 840px;
  border-collapse: collapse;
  font-size: var(--ocg-font-sm);
}
.aliases-table th,
.aliases-table td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--ocg-border);
  text-align: left;
  vertical-align: middle;
}
.aliases-table th {
  color: var(--ocg-muted);
  font-size: var(--ocg-font-xs);
  font-weight: 600;
}
.aliases-table .aliases-name,
.aliases-actions {
  vertical-align: top;
}
.aliases-actions {
  width: 88px;
}
.aliases-editor-error {
  margin-bottom: 12px;
}
.aliases-editor-bindings {
  display: grid;
  gap: 10px;
}
.aliases-editor-label {
  color: var(--ocg-muted);
  font-size: var(--ocg-font-sm);
  font-weight: 600;
}
.aliases-editor-row {
  display: grid;
  grid-template-columns: minmax(150px, 0.8fr) minmax(240px, 1.4fr) auto;
  gap: 8px;
  align-items: center;
}
.aliases-editor-footer {
  display: flex;
  align-items: center;
  gap: 8px;
}
.aliases-editor-spacer {
  flex: 1;
}
@media (max-width: 700px) {
  .aliases-header {
    align-items: stretch;
    flex-direction: column;
  }
  .aliases-editor-row {
    grid-template-columns: 1fr;
  }
}
</style>
