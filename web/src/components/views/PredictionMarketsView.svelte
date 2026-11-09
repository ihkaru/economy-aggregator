<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchPredictionMarkets } from '../../lib/api';
  import type { PredictionMarketContract } from '../../lib/types';
  import Icons from '../Icons.svelte';

  let contracts: PredictionMarketContract[] = [];
  let isLoading: boolean = true;
  let selectedVenue: string = 'ALL';
  let selectedStatus: string = 'ACTIVE';
  let searchQuery: string = '';
  let selectedContract: PredictionMarketContract | null = null;

  const venues = ['ALL', 'KALSHI', 'POLYMARKET'];
  const statuses = ['ALL', 'ACTIVE', 'CLOSED'];

  async function loadData() {
    isLoading = true;
    try {
      const vParam = selectedVenue === 'ALL' ? undefined : selectedVenue.toLowerCase();
      contracts = await fetchPredictionMarkets(vParam, 100);
    } catch (err) {
      console.error('Failed to load prediction markets', err);
    } finally {
      isLoading = false;
    }
  }

  $: filteredContracts = contracts.filter((c) => {
    if (selectedVenue !== 'ALL' && c.venue.toLowerCase() !== selectedVenue.toLowerCase()) return false;
    if (selectedStatus === 'ACTIVE' && !c.is_active) return false;
    if (selectedStatus === 'CLOSED' && c.is_active) return false;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      return (
        c.title.toLowerCase().includes(q) ||
        c.outcome_label.toLowerCase().includes(q) ||
        c.venue_market_id.toLowerCase().includes(q)
      );
    }
    return true;
  });

  $: activeCount = contracts.filter((c) => c.is_active).length;
  $: kalshiCount = contracts.filter((c) => c.venue.toLowerCase() === 'kalshi').length;
  $: polyCount = contracts.filter((c) => c.venue.toLowerCase() === 'polymarket').length;

  onMount(() => {
    loadData();
  });
</script>

<div class="predictions-view tabular-nums">
  <!-- Top Bar: Filters & Controls -->
  <div class="view-header">
    <div class="header-left">
      <div class="title-cluster">
        <Icons name="trending" size={16} classNames="accent-icon" />
        <span class="view-title">PREDICTION MARKETS</span>
        <span class="view-sub">EVENT PROBABILITIES & RESOLUTION ORACLES</span>
      </div>

      <div class="filter-group">
        <span class="filter-label">VENUE:</span>
        {#each venues as v}
          <button
            class="filter-btn"
            class:active={selectedVenue === v}
            on:click={() => { selectedVenue = v; loadData(); }}
          >
            {v}
          </button>
        {/each}
      </div>

      <div class="filter-group">
        <span class="filter-label">STATUS:</span>
        {#each statuses as s}
          <button
            class="filter-btn"
            class:active={selectedStatus === s}
            on:click={() => (selectedStatus = s)}
          >
            {s}
          </button>
        {/each}
      </div>
    </div>

    <div class="header-right">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Filter contract..."
        class="search-input"
      />
      <button class="reload-btn" on:click={loadData} title="Reload contracts">
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </div>

  <!-- Summary Metrics Cards -->
  <div class="metrics-strip">
    <div class="metric-card">
      <span class="card-label">TOTAL CONTRACTS</span>
      <span class="card-value">{contracts.length}</span>
      <span class="card-sub">Kalshi CFTC & Polymarket UMA</span>
    </div>
    <div class="metric-card">
      <span class="card-label">ACTIVE MARKETS</span>
      <span class="card-value purple">{activeCount}</span>
      <span class="card-sub">Live Probabilistic Price Discovery</span>
    </div>
    <div class="metric-card">
      <span class="card-label">VENUE BREAKDOWN</span>
      <span class="card-value green">{kalshiCount} Kalshi / {polyCount} Poly</span>
      <span class="card-sub">Regulated Orderbook & AMM</span>
    </div>
  </div>

  <!-- Main Grid of Contracts -->
  <div class="grid-container">
    {#if isLoading}
      <div class="loading-state">
        <Icons name="loader" size={24} classNames="spinning" />
        <span>FETCHING PROBABILITY CURVES & ORACLE SPECS...</span>
      </div>
    {:else if filteredContracts.length === 0}
      <div class="empty-state">
        <span>No prediction market contracts matching criteria.</span>
      </div>
    {:else}
      <div class="contracts-grid">
        {#each filteredContracts as c}
          <div
            class="contract-card"
            class:selected={selectedContract?.market_id === c.market_id}
            on:click={() => (selectedContract = c)}
          >
            <div class="card-top">
              <div class="badge-row">
                <span class="venue-tag {c.venue.toLowerCase()}">{c.venue.toUpperCase()}</span>
                <span class="status-tag" class:live={c.is_active}>{c.is_active ? 'ACTIVE' : 'RESOLVED'}</span>
              </div>
              <span class="market-type-tag">{c.market_type}</span>
            </div>

            <div class="contract-title">{c.title}</div>
            <div class="outcome-title">Outcome: <span class="highlight">{c.outcome_label}</span></div>

            <!-- Implied Probability Bar -->
            <div class="prob-section">
              <div class="prob-header">
                <span class="prob-label">IMPLIED PROBABILITY</span>
                <span class="prob-pct">68.0%</span>
              </div>
              <div class="prob-bar-track">
                <div class="prob-bar-fill" style="width: 68%;"></div>
              </div>
            </div>

            <!-- Specs Grid -->
            <div class="specs-grid">
              <div class="spec-item"><span class="sk">TICK SIZE:</span><span class="sv">${c.tick_size}</span></div>
              <div class="spec-item"><span class="sk">FACE VALUE:</span><span class="sv">${c.face_value}</span></div>
              <div class="spec-item"><span class="sk">CLOSES:</span><span class="sv">{c.close_time_utc.split('T')[0]}</span></div>
              <div class="spec-item"><span class="sk">ORACLE:</span><span class="sv">{c.oracle.oracle_type.toUpperCase()}</span></div>
            </div>

            <div class="card-footer">
              <span class="oracle-source">Source: {c.oracle.source_name}</span>
              {#if c.venue_url}
                <a
                  href={c.venue_url}
                  target="_blank"
                  rel="noreferrer"
                  class="contract-link"
                  on:click|stopPropagation
                >
                  <Icons name="external-link" size={11} />
                  <span>VIEW CONTRACT</span>
                </a>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Detail Inspector Drawer -->
  {#if selectedContract}
    <div class="detail-drawer">
      <div class="drawer-header">
        <div class="drawer-title">
          <Icons name="trending" size={14} />
          <span>{selectedContract.venue.toUpperCase()} :: {selectedContract.title}</span>
        </div>
        <button class="close-btn" on:click={() => (selectedContract = null)}>
          <Icons name="close" size={13} />
        </button>
      </div>
      <div class="drawer-content">
        <div class="oracle-box">
          <span class="oracle-title">RESOLUTION CRITERIA ({selectedContract.oracle.oracle_type.toUpperCase()})</span>
          <p class="oracle-desc">{selectedContract.oracle.resolution_criteria}</p>
          <div class="oracle-meta">
            <span>Verified Source: <a href={selectedContract.oracle.source_url} target="_blank" rel="noreferrer" class="source-link">{selectedContract.oracle.source_name}</a></span>
            <span>Market Identifier: {selectedContract.venue_market_id}</span>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .predictions-view { display: flex; flex-direction: column; height: 100%; width: 100%; background: var(--bg-surface); color: var(--text-main); font-size: 11px; overflow: hidden; position: relative; }
  .view-header { height: 42px; background: var(--bg-panel); border-bottom: 1px solid var(--border-subtle); display: flex; align-items: center; justify-content: space-between; padding: 0 12px; gap: 12px; flex-shrink: 0; overflow-x: auto; white-space: nowrap; }
  .header-left, .header-right { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .title-cluster { display: flex; align-items: center; gap: 6px; margin-right: 8px; }
  .view-title { font-size: 12px; font-weight: 700; color: var(--text-bright); letter-spacing: 0.5px; }
  .view-sub { font-size: 9px; color: var(--text-muted); border-left: 1px solid var(--border-subtle); padding-left: 6px; }
  .filter-group { display: flex; align-items: center; gap: 2px; }
  .filter-label { font-size: 9px; color: var(--text-muted); font-weight: 700; margin-right: 2px; }
  .filter-btn { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-muted); height: 24px; padding: 0 6px; font-size: 9.5px; font-weight: 600; cursor: pointer; border-radius: 0px; outline: none; }
  .filter-btn.active { background: var(--bg-active); border-color: var(--border-contrast); color: var(--text-bright); font-weight: 700; }
  .search-input { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-bright); height: 24px; padding: 0 8px; font-size: 10.5px; outline: none; width: 140px; border-radius: 0px; }
  .reload-btn { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-muted); width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; cursor: pointer; border-radius: 0px; }
  .metrics-strip { display: flex; gap: 10px; padding: 10px 12px; background: #131722; border-bottom: 1px solid var(--border-subtle); flex-shrink: 0; overflow-x: auto; }
  .metric-card { flex: 1; min-width: 180px; background: #181c27; border: 1px solid var(--border-subtle); padding: 8px 12px; display: flex; flex-direction: column; }
  .card-label { font-size: 9px; font-weight: 700; color: var(--text-muted); letter-spacing: 0.5px; margin-bottom: 4px; }
  .card-value { font-size: 18px; font-weight: 700; color: var(--text-bright); }
  .card-value.purple { color: #a78bfa; }
  .card-value.green { color: var(--bull-color); }
  .card-sub { font-size: 9px; color: #787b86; margin-top: 2px; }
  .grid-container { flex: 1; overflow-y: auto; padding: 12px; }
  .contracts-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 12px; }
  .contract-card { background: #181c27; border: 1px solid var(--border-subtle); padding: 12px; display: flex; flex-direction: column; cursor: pointer; transition: border-color 0.12s, background 0.12s; }
  .contract-card:hover { border-color: #3b82f6; background: #1e2433; }
  .contract-card.selected { border-color: #8b5cf6; background: #1e2235; box-shadow: inset 0 0 0 1px #8b5cf6; }
  .card-top { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  .badge-row { display: flex; gap: 4px; }
  .venue-tag { font-size: 8.5px; font-weight: 700; padding: 1px 4px; }
  .venue-tag.kalshi { background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.3); }
  .venue-tag.polymarket { background: rgba(139, 92, 246, 0.15); color: #c4b5fd; border: 1px solid rgba(139, 92, 246, 0.3); }
  .status-tag { font-size: 8px; font-weight: 700; padding: 1px 4px; background: rgba(255, 255, 255, 0.05); color: var(--text-muted); border: 1px solid var(--border-subtle); }
  .status-tag.live { color: var(--bull-color); border-color: rgba(8, 153, 129, 0.3); }
  .market-type-tag { font-size: 8.5px; color: var(--text-muted); }
  .contract-title { font-size: 12px; font-weight: 700; color: var(--text-bright); line-height: 1.3; margin-bottom: 4px; }
  .outcome-title { font-size: 10px; color: var(--text-muted); margin-bottom: 10px; }
  .outcome-title .highlight { color: #f59e0b; font-weight: 600; }
  .prob-section { margin-bottom: 12px; background: rgba(0, 0, 0, 0.2); padding: 6px; border: 1px solid var(--border-subtle); }
  .prob-header { display: flex; justify-content: space-between; margin-bottom: 4px; font-size: 9px; font-weight: 700; }
  .prob-label { color: var(--text-muted); }
  .prob-pct { color: #60a5fa; }
  .prob-bar-track { height: 6px; background: #2a2e39; overflow: hidden; }
  .prob-bar-fill { height: 100%; background: linear-gradient(90deg, #3b82f6, #8b5cf6); }
  .specs-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; margin-bottom: 10px; font-size: 9px; }
  .spec-item { display: flex; justify-content: space-between; }
  .sk { color: var(--text-muted); }
  .sv { color: var(--text-bright); font-weight: 600; }
  .card-footer { display: flex; justify-content: space-between; align-items: center; border-top: 1px solid rgba(255, 255, 255, 0.04); padding-top: 8px; font-size: 9px; }
  .oracle-source { color: #787b86; }
  .contract-link { display: inline-flex; align-items: center; gap: 3px; color: #60a5fa; text-decoration: none; font-weight: 600; }
  .contract-link:hover { text-decoration: underline; }
  .detail-drawer { position: absolute; bottom: 0; left: 0; right: 0; background: #181c27; border-top: 1px solid var(--border-contrast); padding: 12px; box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.5); z-index: 20; }
  .drawer-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; border-bottom: 1px solid var(--border-subtle); padding-bottom: 6px; }
  .drawer-title { display: flex; align-items: center; gap: 6px; font-size: 11px; font-weight: 700; color: var(--text-bright); }
  .close-btn { background: transparent; border: none; color: var(--text-muted); cursor: pointer; }
  .oracle-box { font-size: 10.5px; }
  .oracle-title { font-size: 9.5px; font-weight: 700; color: #a78bfa; letter-spacing: 0.5px; display: block; margin-bottom: 4px; }
  .oracle-desc { color: var(--text-bright); line-height: 1.4; margin-bottom: 6px; }
  .oracle-meta { display: flex; gap: 16px; font-size: 9px; color: var(--text-muted); }
  .source-link { color: #60a5fa; text-decoration: none; }
  .loading-state, .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 200px; gap: 10px; color: var(--text-muted); }
  .spinning :global(svg) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>