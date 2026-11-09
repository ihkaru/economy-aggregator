<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchConsensusDivergence } from '../../lib/api';
  import type { MacroPredictionDivergence } from '../../lib/types';
  import Icons from '../Icons.svelte';

  let divergences: MacroPredictionDivergence[] = [];
  let isLoading: boolean = true;
  let searchQuery: string = '';
  let selectedDiv: MacroPredictionDivergence | null = null;

  async function loadData() {
    isLoading = true;
    try {
      divergences = await fetchConsensusDivergence();
    } catch (err) {
      console.error('Failed to load consensus divergence', err);
    } finally {
      isLoading = false;
    }
  }

  $: filtered = divergences.filter((d) => {
    if (!searchQuery) return true;
    const q = searchQuery.toLowerCase();
    return d.title.toLowerCase().includes(q) || d.indicator_code.toLowerCase().includes(q);
  });

  $: linkedCount = divergences.filter((d) => d.linked_markets_count > 0).length;
  $: resolvedCount = divergences.filter((d) => d.actual_released_value !== null).length;

  onMount(() => {
    loadData();
  });
</script>

<div class="consensus-view tabular-nums">
  <!-- Top Bar -->
  <div class="view-header">
    <div class="header-left">
      <div class="title-cluster">
        <Icons name="layers" size={16} classNames="accent-icon" />
        <span class="view-title">CONSENSUS MATRIX</span>
        <span class="view-sub">ECONOMIST FORECAST VS PREDICTION MARKET CROWD DIVERGENCE</span>
      </div>
    </div>

    <div class="header-right">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Filter indicator..."
        class="search-input"
      />
      <button class="reload-btn" on:click={loadData} title="Reload divergence data">
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </div>

  <!-- Metrics Strip -->
  <div class="metrics-strip">
    <div class="metric-card">
      <span class="card-label">CATALYSTS TRACKED</span>
      <span class="card-value">{divergences.length}</span>
      <span class="card-sub">High-Impact Scheduled Macro Releases</span>
    </div>
    <div class="metric-card">
      <span class="card-label">MARKET-LINKED INDICATORS</span>
      <span class="card-value green">{linkedCount}</span>
      <span class="card-sub">Coupled with Kalshi / Polymarket Oracles</span>
    </div>
    <div class="metric-card">
      <span class="card-label">POST-RELEASE RESOLUTIONS</span>
      <span class="card-value amber">{resolvedCount}</span>
      <span class="card-sub">Confirmed Point-in-Time Actuals</span>
    </div>
  </div>

  <!-- Table Container -->
  <div class="table-container">
    {#if isLoading}
      <div class="loading-state">
        <Icons name="loader" size={24} classNames="spinning" />
        <span>COMPUTING MACRO-PREDICTION CROSS DIVERGENCE...</span>
      </div>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <span>No divergence records found matching criteria.</span>
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>SCHEDULED (UTC)</th>
            <th>COUNTRY</th>
            <th>INDICATOR</th>
            <th>ECONOMIST FORECAST</th>
            <th>PREVIOUS VALUE</th>
            <th>PREDICTION CROWD YES %</th>
            <th>VENUES</th>
            <th>LINKED CONTRACTS</th>
            <th>ACTUAL RELEASE</th>
            <th>SURPRISE RESOLUTION</th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as d}
            {@const probPct = d.average_implied_prob_yes !== null && d.average_implied_prob_yes !== undefined ? Math.round(d.average_implied_prob_yes * 100) : null}
            <tr on:click={() => (selectedDiv = d)} class:selected={selectedDiv?.indicator_code === d.indicator_code}>
              <td class="date-cell">{d.scheduled_utc.replace('T', ' ').slice(0, 16)}</td>
              <td><span class="country-tag">{d.country}</span></td>
              <td class="title-cell">
                <span class="indicator-code">[{d.indicator_code}]</span>
                <span class="indicator-name">{d.title}</span>
              </td>
              <td class="val-cell bold">
                {d.economist_forecast !== null && d.economist_forecast !== undefined ? `${d.economist_forecast} ${d.unit_symbol}` : '-'}
              </td>
              <td class="val-cell">
                {d.previous_value !== null && d.previous_value !== undefined ? `${d.previous_value} ${d.unit_symbol}` : '-'}
              </td>
              <td class="prob-cell">
                {#if probPct !== null}
                  <div class="prob-wrapper">
                    <span class="prob-val">{probPct}%</span>
                    <div class="mini-bar-track">
                      <div class="mini-bar-fill" style="width: {probPct}%;"></div>
                    </div>
                  </div>
                {:else}
                  <span class="dim-text">NO CONTRACT</span>
                {/if}
              </td>
              <td>
                <div class="venue-tags">
                  {#if d.venues_represented.length > 0}
                    {#each d.venues_represented as v}
                      <span class="v-tag {v.toLowerCase()}">{v.toUpperCase()}</span>
                    {/each}
                  {:else}
                    <span class="dim-text">-</span>
                  {/if}
                </div>
              </td>
              <td class="center-cell">
                <span class="contracts-count">{d.linked_markets_count}</span>
              </td>
              <td class="val-cell bold">
                {d.actual_released_value !== null && d.actual_released_value !== undefined ? `${d.actual_released_value} ${d.unit_symbol}` : '-'}
              </td>
              <td>
                {#if d.surprise_direction}
                  <span class="surprise-tag {d.surprise_direction}">
                    {d.surprise_direction.toUpperCase().replace('_', ' ')}
                  </span>
                {:else}
                  <span class="pending-tag">PENDING RELEASE</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Detail Drawer -->
  {#if selectedDiv}
    <div class="detail-drawer">
      <div class="drawer-header">
        <div class="drawer-title">
          <Icons name="layers" size={14} />
          <span>CROSS-ANALYTICS: {selectedDiv.indicator_code} - {selectedDiv.title}</span>
        </div>
        <button class="close-btn" on:click={() => (selectedDiv = null)}>
          <Icons name="close" size={13} />
        </button>
      </div>
      <div class="drawer-content">
        <div class="drawer-grid">
          <div class="drawer-item"><span class="k">Country / Currency:</span><span class="v">{selectedDiv.country} ({selectedDiv.unit_symbol})</span></div>
          <div class="drawer-item"><span class="k">Economist Consensus:</span><span class="v">{selectedDiv.economist_forecast ?? 'N/A'}</span></div>
          <div class="drawer-item"><span class="k">Crowd Implied Probability:</span><span class="v">{selectedDiv.average_implied_prob_yes ? `${(selectedDiv.average_implied_prob_yes * 100).toFixed(1)}%` : 'No open orderbook'}</span></div>
          <div class="drawer-item"><span class="k">Active Market Venues:</span><span class="v">{selectedDiv.venues_represented.join(', ') || 'None'}</span></div>
          <div class="drawer-item"><span class="k">Resolution Status:</span><span class="v">{selectedDiv.surprise_direction ?? 'Pending scheduled release'}</span></div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .consensus-view { display: flex; flex-direction: column; height: 100%; width: 100%; background: var(--bg-surface); color: var(--text-main); font-size: 11px; overflow: hidden; position: relative; }
  .view-header { height: 42px; background: var(--bg-panel); border-bottom: 1px solid var(--border-subtle); display: flex; align-items: center; justify-content: space-between; padding: 0 12px; gap: 12px; flex-shrink: 0; }
  .header-left, .header-right { display: flex; align-items: center; gap: 8px; }
  .title-cluster { display: flex; align-items: center; gap: 6px; }
  .view-title { font-size: 12px; font-weight: 700; color: var(--text-bright); letter-spacing: 0.5px; }
  .view-sub { font-size: 9px; color: var(--text-muted); border-left: 1px solid var(--border-subtle); padding-left: 6px; }
  .search-input { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-bright); height: 24px; padding: 0 8px; font-size: 10.5px; outline: none; width: 140px; border-radius: 0px; }
  .reload-btn { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-muted); width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; cursor: pointer; border-radius: 0px; }
  .metrics-strip { display: flex; gap: 10px; padding: 10px 12px; background: #131722; border-bottom: 1px solid var(--border-subtle); flex-shrink: 0; }
  .metric-card { flex: 1; background: #181c27; border: 1px solid var(--border-subtle); padding: 8px 12px; display: flex; flex-direction: column; }
  .card-label { font-size: 9px; font-weight: 700; color: var(--text-muted); letter-spacing: 0.5px; margin-bottom: 4px; }
  .card-value { font-size: 18px; font-weight: 700; color: var(--text-bright); }
  .card-value.green { color: var(--bull-color); }
  .card-value.amber { color: var(--accent-amber); }
  .card-sub { font-size: 9px; color: #787b86; margin-top: 2px; }
  .table-container { flex: 1; overflow: auto; background: var(--bg-surface); }
  .data-table { width: 100%; border-collapse: collapse; text-align: left; }
  .data-table th { background: #181c27; color: var(--text-muted); font-size: 9px; font-weight: 700; letter-spacing: 0.5px; padding: 6px 10px; border-bottom: 1px solid var(--border-subtle); position: sticky; top: 0; z-index: 10; white-space: nowrap; }
  .data-table td { padding: 6px 10px; border-bottom: 1px solid rgba(255, 255, 255, 0.03); white-space: nowrap; }
  .data-table tr:hover { background: var(--bg-hover); cursor: pointer; }
  .data-table tr.selected { background: #1e2638; }
  .country-tag { background: rgba(255, 255, 255, 0.04); border: 1px solid var(--border-subtle); padding: 1px 4px; font-size: 9.5px; font-weight: 700; color: var(--text-bright); }
  .title-cell { display: flex; align-items: center; gap: 6px; }
  .indicator-code { color: #60a5fa; font-weight: 700; font-size: 10px; }
  .indicator-name { color: var(--text-bright); font-weight: 500; }
  .val-cell.bold { color: var(--text-bright); font-weight: 700; }
  .prob-wrapper { display: flex; align-items: center; gap: 6px; }
  .prob-val { color: #60a5fa; font-weight: 700; font-size: 10.5px; width: 32px; }
  .mini-bar-track { width: 50px; height: 5px; background: #2a2e39; overflow: hidden; }
  .mini-bar-fill { height: 100%; background: #3b82f6; }
  .dim-text { color: #555e75; font-size: 9px; }
  .venue-tags { display: flex; gap: 4px; }
  .v-tag { font-size: 8px; font-weight: 700; padding: 1px 3px; }
  .v-tag.kalshi { background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.3); }
  .v-tag.polymarket { background: rgba(139, 92, 246, 0.15); color: #c4b5fd; border: 1px solid rgba(139, 92, 246, 0.3); }
  .contracts-count { font-weight: 700; color: var(--text-bright); }
  .surprise-tag { font-size: 8.5px; font-weight: 700; padding: 1px 4px; }
  .surprise-tag.above_consensus { color: var(--bull-color); background: rgba(8, 153, 129, 0.1); }
  .surprise-tag.below_consensus { color: var(--bear-color); background: rgba(242, 54, 69, 0.1); }
  .surprise-tag.in_line { color: var(--accent-amber); background: rgba(245, 158, 11, 0.1); }
  .pending-tag { font-size: 9px; color: #555e75; }
  .detail-drawer { position: absolute; bottom: 0; left: 0; right: 0; background: #181c27; border-top: 1px solid var(--border-contrast); padding: 12px; box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.5); z-index: 20; }
  .drawer-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; border-bottom: 1px solid var(--border-subtle); padding-bottom: 6px; }
  .drawer-title { display: flex; align-items: center; gap: 6px; font-size: 11px; font-weight: 700; color: var(--text-bright); }
  .close-btn { background: transparent; border: none; color: var(--text-muted); cursor: pointer; }
  .drawer-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 8px; }
  .drawer-item { display: flex; flex-direction: column; font-size: 10px; }
  .drawer-item .k { color: var(--text-muted); font-weight: 600; margin-bottom: 2px; }
  .drawer-item .v { color: var(--text-bright); }
  .loading-state, .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 200px; gap: 10px; color: var(--text-muted); }
  .spinning :global(svg) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>