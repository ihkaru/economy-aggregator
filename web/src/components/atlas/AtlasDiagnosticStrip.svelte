<script lang="ts">
  import Icons from '../Icons.svelte';
  import type {
    DiagnosticTreeReport,
    ProspectiveScorecardReport,
    RegimeTransitionEvent,
  } from '../../lib/atlas_types';

  export let tree: DiagnosticTreeReport;
  export let transitions: RegimeTransitionEvent[] = [];
  export let scorecard: ProspectiveScorecardReport | null = null;
  export let activeTab: 'tree' | 'transitions' | 'prospective' = 'tree';
  export let onSelectTab: (tab: 'tree' | 'transitions' | 'prospective') => void = () => {};

  function tierLabel(tier: string): { text: string; cls: string } {
    switch (tier) {
      case 'tier1_accounting': return { text: 'TIER 1 ACCOUNTING', cls: 'bull' };
      case 'tier2_replicated': return { text: 'TIER 2 REPLICATED', cls: 'blue' };
      case 'tier3_regime_descriptive': return { text: 'TIER 3 DESCRIPTIVE', cls: 'amber' };
      case 'tier4_hypothesis_gauge': return { text: 'TIER 4 HYPOTHESIS', cls: 'slate' };
      default: return { text: 'BLIND SPOT', cls: 'bear' };
    }
  }

  $: badge = tierLabel(tree.terminal_epistemic_tier);
</script>

<section class="diag-strip tabular-nums" aria-label="Diagnostic Tree and Epistemic Governance">
  <header class="diag-header">
    <div class="diag-summary">
      <div class="verdict-row">
        <span class="verdict-chip">{tree.protocol_version}</span>
        <span class="tier-chip {badge.cls}">{badge.text}</span>
        <span class="leaf-title">{tree.terminal_leaf_id}</span>
        <span class="regime-sub">Regime: {tree.root_regime} | Window: {tree.window_weeks}W | As-Of: {tree.as_of_date}</span>
      </div>
      <p class="verdict-text">{tree.terminal_verdict}</p>
    </div>

    <div class="tab-bar" role="tablist">
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'tree'}
        class="tab-btn"
        class:active={activeTab === 'tree'}
        on:click={() => onSelectTab('tree')}
      >
        NODES ({tree.nodes.length}) &amp; BLIND SPOTS ({tree.blind_spots.length})
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'transitions'}
        class="tab-btn"
        class:active={activeTab === 'transitions'}
        on:click={() => onSelectTab('transitions')}
      >
        TRANSITIONS ({transitions.length})
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'prospective'}
        class="tab-btn"
        class:active={activeTab === 'prospective'}
        on:click={() => onSelectTab('prospective')}
      >
        PROSPECTIVE OOS
      </button>
    </div>
  </header>

  {#if activeTab === 'tree'}
    <div class="diag-body">
      <div class="nodes-grid">
        {#each tree.nodes as node}
          {@const nt = tierLabel(node.epistemic_tier)}
          <div class="node-card" class:on-path={tree.active_path.includes(node.node_id)}>
            <div class="node-top">
              <span class="node-id">{node.node_id}</span>
              <span class="node-status status-{node.status}">{node.status.toUpperCase()}</span>
            </div>
            <div class="node-q">{node.question}</div>
            <div class="node-meta">
              <span class="tier-mini {nt.cls}">{nt.text}</span>
              <span class="branch-pill">branch: {node.branch_taken}</span>
            </div>
            <p class="node-sum">{node.summary}</p>
          </div>
        {/each}
      </div>

      {#if tree.blind_spots.length > 0}
        <div class="blind-box">
          <div class="blind-title">
            <Icons name="alert-triangle" size={13} />
            <span>UNRESOLVED DATA BLIND SPOTS ({tree.blind_spots.length})</span>
          </div>
          <div class="blind-list">
            {#each tree.blind_spots as bs}
              <div class="blind-item">
                <div class="bs-q">[{bs.triggered_by_node}] {bs.question}</div>
                <div class="bs-gap">Gap: {bs.missing_data_gap} | Rule: {bs.governance_rule}</div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {:else if activeTab === 'transitions'}
    <div class="diag-body">
      {#if transitions.length === 0}
        <div class="empty-tab">No regime transition events recorded in Parquet history yet.</div>
      {:else}
        <div class="trans-table-wrap">
          <table class="trans-table">
            <thead>
              <tr>
                <th>As-Of</th>
                <th>Kind</th>
                <th>Transition</th>
                <th>Summary</th>
              </tr>
            </thead>
            <tbody>
              {#each transitions as ev}
                <tr>
                  <td class="mono">{ev.as_of_date} ({ev.window_weeks}W)</td>
                  <td><span class="kind-pill">{ev.transition_kind}</span></td>
                  <td class="mono">{ev.from_leaf_id || 'INITIAL'} -&gt; {ev.to_leaf_id}</td>
                  <td class="wrap-cell">{ev.summary}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else}
    <div class="diag-body">
      {#if !scorecard}
        <div class="empty-tab">Loading prospective OOS scorecard...</div>
      {:else}
        <div class="oos-summary-bar">
          <span>Protocol: <strong>{scorecard.protocol_version}</strong> ({scorecard.protocol_anchor_tag})</span>
          <span>OOS Start: <strong>{scorecard.oos_start_week}</strong></span>
          <span>Latest Panel Week: <strong>{scorecard.latest_panel_week || 'N/A'}</strong></span>
          <span>Weeks Collected: <strong>{scorecard.oos_weeks_collected}</strong> ({scorecard.primary_weeks_remaining} remaining)</span>
          <span class="oos-status">{scorecard.overall_readout_status}</span>
        </div>
        <div class="oos-rules">
          {#each [...scorecard.primary_rules_m2, ...scorecard.secondary_rules_m5] as r}
            <div class="oos-rule-card">
              <div class="oos-rule-head">
                <span class="node-id">{r.rule_id}</span>
                <span class="kind-pill">{r.rule_class}</span>
                <span class="branch-pill">{r.readout_status}</span>
              </div>
              <div class="oos-rule-meta">
                <span>Family: {r.family}</span>
                <span>Triggers: {r.trigger_count}</span>
                <span>OOS Obs: {r.oos_observations_collected}/{r.target_readout_weeks}</span>
                <span>{r.running_metric_label}: {r.running_metric_value ?? 'N/A'}</span>
              </div>
              <p class="node-sum">{r.governance_note}</p>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .diag-strip {
    background: var(--bg-panel);
    border: 1px solid var(--border-subtle);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .diag-header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: flex-start; gap: 10px; border-bottom: 1px solid var(--border-subtle); padding-bottom: 8px; }
  .diag-summary { flex: 1; min-width: 260px; }
  .verdict-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-bottom: 4px; }
  .verdict-chip { background: #1e293b; border: 1px solid #334155; color: #93c5fd; padding: 2px 6px; font-size: 9.5px; font-weight: 700; font-family: var(--font-mono); }
  .tier-chip, .tier-mini { padding: 2px 6px; font-size: 9px; font-weight: 700; border: 1px solid var(--border-contrast); }
  .bull { color: #34d399; border-color: rgba(8, 153, 129, 0.5); background: rgba(8, 153, 129, 0.12); }
  .blue { color: #93c5fd; border-color: rgba(59, 130, 246, 0.5); background: rgba(59, 130, 246, 0.12); }
  .amber { color: #fbbf24; border-color: rgba(245, 158, 11, 0.5); background: rgba(245, 158, 11, 0.12); }
  .bear { color: #f87171; border-color: rgba(242, 54, 69, 0.5); background: rgba(242, 54, 69, 0.12); }
  .slate { color: #cbd5e1; border-color: #334155; background: #1e293b; }
  .leaf-title { font-size: 12px; font-weight: 700; color: var(--text-bright); font-family: var(--font-mono); overflow-wrap: anywhere; }
  .regime-sub { font-size: 10px; color: #94a3b8; }
  .verdict-text { font-size: 11px; color: var(--text-main); line-height: 1.45; overflow-wrap: anywhere; }
  .tab-bar { display: flex; flex-wrap: wrap; gap: 4px; }
  .tab-btn {
    background: var(--bg-surface); border: 1px solid var(--border-subtle); color: #94a3b8;
    padding: 5px 9px; font-size: 10px; font-weight: 700; cursor: pointer;
  }
  .tab-btn:hover { background: var(--bg-hover); color: var(--text-bright); }
  .tab-btn.active { background: var(--bg-active); border-color: #3b82f6; color: var(--text-bright); }
  .tab-btn:focus-visible { outline: 2px solid #3b82f6; outline-offset: 1px; }
  .nodes-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 8px; }
  .node-card { background: #0b0d13; border: 1px solid var(--border-subtle); padding: 8px; display: flex; flex-direction: column; gap: 4px; }
  .node-card.on-path { border-color: #2962ff; background: rgba(41, 98, 255, 0.05); }
  .node-top { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
  .node-id { font-family: var(--font-mono); font-size: 10.5px; font-weight: 700; color: #93c5fd; overflow-wrap: anywhere; }
  .node-status { font-size: 9px; font-weight: 700; padding: 1px 5px; border: 1px solid var(--border-contrast); }
  .status-true { color: #34d399; border-color: rgba(8, 153, 129, 0.5); }
  .status-false { color: #94a3b8; }
  .status-stale_lag, .status-insufficient_data { color: #fbbf24; border-color: rgba(245, 158, 11, 0.5); }
  .node-q { font-size: 10.5px; font-weight: 600; color: var(--text-bright); overflow-wrap: anywhere; }
  .node-meta { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
  .branch-pill, .kind-pill { font-size: 9px; padding: 1px 5px; background: #141722; border: 1px solid var(--border-subtle); color: #cbd5e1; }
  .node-sum { font-size: 10px; color: #cbd5e1; line-height: 1.4; overflow-wrap: anywhere; }
  .blind-box { margin-top: 8px; padding: 8px 10px; border: 1px solid rgba(245, 158, 11, 0.4); background: rgba(245, 158, 11, 0.06); }
  .blind-title { display: flex; align-items: center; gap: 6px; font-size: 10.5px; font-weight: 700; color: #fbbf24; margin-bottom: 5px; }
  .blind-list { display: flex; flex-direction: column; gap: 5px; }
  .blind-item { border-top: 1px solid rgba(245, 158, 11, 0.2); padding-top: 4px; font-size: 10px; color: var(--text-main); }
  .bs-q { font-weight: 600; overflow-wrap: anywhere; }
  .bs-gap { color: #94a3b8; margin-top: 2px; overflow-wrap: anywhere; }
  .trans-table-wrap { overflow-x: auto; }
  .trans-table { width: 100%; border-collapse: collapse; font-size: 10.5px; }
  .trans-table th, .trans-table td { padding: 5px 8px; border-bottom: 1px solid var(--border-subtle); text-align: left; vertical-align: top; }
  .trans-table th { color: #94a3b8; font-weight: 700; }
  .mono { font-family: var(--font-mono); white-space: nowrap; }
  .wrap-cell { overflow-wrap: anywhere; }
  .oos-summary-bar { display: flex; flex-wrap: wrap; gap: 10px 16px; font-size: 10.5px; padding: 6px 8px; background: #0b0d13; border: 1px solid var(--border-subtle); margin-bottom: 8px; }
  .oos-status { color: #fbbf24; font-weight: 700; }
  .oos-rules { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 8px; }
  .oos-rule-card { background: #0b0d13; border: 1px solid var(--border-subtle); padding: 8px; display: flex; flex-direction: column; gap: 4px; }
  .oos-rule-head { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
  .oos-rule-meta { display: flex; flex-wrap: wrap; gap: 8px; font-size: 9.5px; color: #94a3b8; }
  .empty-tab { font-size: 11px; color: #94a3b8; padding: 8px 0; }
  @media (max-width: 768px) {
    .tab-btn { min-height: 44px; padding: 6px 10px; }
  }
</style>
