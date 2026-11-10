<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fetchSubscriptions,
    deleteSubscription,
    addSubscription,
    fetchIngestionStatus,
    triggerManualCatchUp,
    searchSymbols,
  } from '../../lib/api';
  import type { SubscriptionItem, OrchestratorStatus, DiscoverySearchResult } from '../../lib/types';
  import Icons from '../Icons.svelte';

  let subscriptions: SubscriptionItem[] = [];
  let status: OrchestratorStatus | null = null;
  let isLoading: boolean = true;
  let isCatchingUp: boolean = false;
  let actionMessage: string | null = null;

  // Search / Discovery State
  let searchQuery: string = '';
  let selectedSearchSource: string = 'all';
  let isSearching: boolean = false;
  let searchResults: DiscoverySearchResult[] = [];

  const searchSources = ['all', 'yahoo', 'binance', 'polymarket', 'kalshi'];

  async function loadData() {
    isLoading = true;
    try {
      const [subs, st] = await Promise.all([
        fetchSubscriptions(),
        fetchIngestionStatus().catch(() => null),
      ]);
      subscriptions = subs;
      status = st;
    } catch (err: any) {
      console.error('Failed loading subscriptions', err);
      actionMessage = `Error: ${err.message || 'Failed loading data'}`;
    } finally {
      isLoading = false;
    }
  }

  async function handleUnsubscribe(symbol: string, source: string) {
    try {
      await deleteSubscription(symbol, source);
      actionMessage = `Unsubscribed from ${symbol} (${source})`;
      await loadData();
    } catch (err: any) {
      actionMessage = `Error unsubscribing: ${err.message}`;
    }
  }

  async function handleCatchUp() {
    isCatchingUp = true;
    actionMessage = null;
    try {
      const res = await triggerManualCatchUp();
      actionMessage = `Catch-up initiated! Ingested: ${res.total_candles_ingested || 0} records`;
      await loadData();
    } catch (err: any) {
      actionMessage = `Catch-up error: ${err.message}`;
    } finally {
      isCatchingUp = false;
    }
  }

  async function handleSearch() {
    if (!searchQuery.trim()) return;
    isSearching = true;
    try {
      searchResults = await searchSymbols(searchQuery.trim(), selectedSearchSource);
    } catch (err: any) {
      console.error('Search error', err);
    } finally {
      isSearching = false;
    }
  }

  async function handleQuickSubscribe(result: DiscoverySearchResult) {
    try {
      await addSubscription({
        source: result.source,
        symbol: result.raw_symbol,
        asset_class: result.asset_class,
        timeframes: ['1d'],
        enabled: true,
        poll_interval_secs: result.asset_class === 'forex' ? 60 : 300,
        created_at_utc: new Date().toISOString(),
      });
      actionMessage = `Subscribed to ${result.raw_symbol} (${result.source})`;
      searchResults = searchResults.filter((r) => r.raw_symbol !== result.raw_symbol);
      await loadData();
    } catch (err: any) {
      actionMessage = `Subscribe error: ${err.message}`;
    }
  }

  onMount(() => {
    loadData();
  });
</script>

<div class="subscriptions-view tabular-nums">
  <!-- Top Bar -->
  <div class="view-header">
    <div class="header-left">
      <div class="title-cluster">
        <Icons name="refresh" size={16} classNames="accent-icon" />
        <span class="view-title">INGESTION & SUBSCRIPTIONS</span>
        <span class="view-sub">NATURAL (SOURCE, SYMBOL) IDENTITY · AUTONOMOUS CATCH-UP</span>
      </div>
    </div>

    <div class="header-right">
      {#if actionMessage}
        <span class="action-badge">{actionMessage}</span>
      {/if}
      <button class="catchup-btn" on:click={handleCatchUp} disabled={isCatchingUp}>
        <Icons name="activity" size={13} classNames={isCatchingUp ? 'spinning' : ''} />
        <span>{isCatchingUp ? 'CATCHING UP...' : 'TRIGGER CATCH-UP'}</span>
      </button>
      <button class="reload-btn" on:click={loadData} title="Reload Subscriptions">
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </div>

  <!-- Metrics Strip -->
  <div class="metrics-strip">
    <div class="metric-card">
      <span class="card-label">ACTIVE SUBSCRIPTIONS</span>
      <span class="card-value cyan">{subscriptions.length}</span>
      <span class="card-sub">Dynamic Lakehouse Feeds</span>
    </div>
    <div class="metric-card">
      <span class="card-label">DAEMON ORCHESTRATOR</span>
      <span class="card-value" class:bull={status?.is_running} class:bear={!status?.is_running}>
        {status?.is_running ? 'RUNNING' : 'OFFLINE'}
      </span>
      <span class="card-sub">{status?.active_streams?.length || 0} WS · {status?.active_pollers?.length || 0} Pollers</span>
    </div>
    <div class="metric-card">
      <span class="card-label">INGESTION BOOT</span>
      <span class="card-value small">{status?.boot_timestamp_utc ? status.boot_timestamp_utc.split('T')[0] : 'N/A'}</span>
      <span class="card-sub">{status?.boot_timestamp_utc ? status.boot_timestamp_utc.split('T')[1].slice(0, 8) + ' UTC' : '-'}</span>
    </div>
  </div>

  <!-- Discovery Search Card -->
  <div class="discovery-card">
    <div class="discovery-header">
      <span class="section-title">MULTI-SOURCE DISCOVERY & 1-CLICK SUBSCRIBE</span>
      <div class="source-pills">
        {#each searchSources as src}
          <button
            class="source-pill"
            class:active={selectedSearchSource === src}
            on:click={() => { selectedSearchSource = src; if (searchQuery) handleSearch(); }}
          >
            {src.toUpperCase()}
          </button>
        {/each}
      </div>
    </div>

    <form class="search-bar" on:submit|preventDefault={handleSearch}>
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Search Yahoo, Binance, Polymarket, Kalshi (e.g. BBRI, SOLUSDT, Fed, CPI)..."
        class="search-input"
      />
      <button type="submit" class="search-btn" disabled={isSearching}>
        <Icons name="search" size={13} />
        <span>{isSearching ? 'SEARCHING...' : 'DISCOVER'}</span>
      </button>
    </form>

    {#if searchResults.length > 0}
      <div class="search-results-table">
        <table>
          <thead>
            <tr>
              <th>SOURCE</th>
              <th>SYMBOL</th>
              <th>NAME</th>
              <th>ASSET CLASS</th>
              <th>ACTION</th>
            </tr>
          </thead>
          <tbody>
            {#each searchResults as res}
              <tr>
                <td><span class="badge {res.source}">{res.source.toUpperCase()}</span></td>
                <td class="bold cyan">{res.raw_symbol}</td>
                <td class="name-cell">{res.name}</td>
                <td><span class="badge-sub">{res.asset_class}</span></td>
                <td>
                  <button class="sub-btn" on:click={() => handleQuickSubscribe(res)}>
                    + SUBSCRIBE
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- Subscriptions Table -->
  <div class="table-container">
    <div class="table-header-strip">
      <span class="section-title">PERSISTENT SUBSCRIPTIONS (data/config/subscriptions.json)</span>
      <span class="count-tag">{subscriptions.length} Instruments</span>
    </div>
    <div class="table-wrapper">
      <table class="data-table">
        <thead>
          <tr>
            <th style="width: 110px;">SOURCE</th>
            <th style="width: 140px;">SYMBOL</th>
            <th style="width: 120px;">ASSET CLASS</th>
            <th style="width: 120px;">TIMEFRAMES</th>
            <th style="width: 100px;">INTERVAL</th>
            <th style="width: 100px;">STATUS</th>
            <th style="width: 100px;">ACTIONS</th>
          </tr>
        </thead>
        <tbody>
          {#each subscriptions as sub}
            <tr>
              <td><span class="badge {sub.source}">{sub.source.toUpperCase()}</span></td>
              <td class="bold text-white">{sub.symbol}</td>
              <td><span class="badge-sub">{sub.asset_class}</span></td>
              <td class="mono">{sub.timeframes.join(', ')}</td>
              <td class="mono text-muted">{sub.poll_interval_secs ? sub.poll_interval_secs + 's' : 'WebSocket'}</td>
              <td>
                <span class="status-pill" class:active={sub.enabled}>
                  {sub.enabled ? 'ACTIVE' : 'DISABLED'}
                </span>
              </td>
              <td>
                <button
                  class="unsub-btn"
                  on:click={() => handleUnsubscribe(sub.symbol, sub.source)}
                  title="Unsubscribe from {sub.symbol}"
                >
                  REMOVE
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
</div>

<style>
  .subscriptions-view { display: flex; flex-direction: column; height: 100%; background: #0c0e14; color: #d1d5db; overflow-y: auto; padding: 16px; gap: 16px; box-sizing: border-box; }
  .view-header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #1f2430; padding-bottom: 12px; }
  .title-cluster { display: flex; align-items: center; gap: 8px; }
  .view-title { font-size: 14px; font-weight: 700; letter-spacing: 0.05em; color: #f3f4f6; }
  .view-sub { font-size: 11px; color: #9ca3af; margin-left: 6px; }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .action-badge { font-size: 11px; color: #10b981; background: #064e3b33; padding: 4px 8px; border-radius: 4px; border: 1px solid #05966944; }
  .catchup-btn { display: flex; align-items: center; gap: 6px; background: #2563eb; color: #fff; border: none; padding: 6px 12px; font-size: 11px; font-weight: 600; border-radius: 4px; cursor: pointer; transition: background 0.15s; }
  .catchup-btn:hover:not(:disabled) { background: #1d4ed8; }
  .catchup-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .reload-btn { background: #1f2430; border: 1px solid #2d3345; color: #9ca3af; border-radius: 4px; padding: 6px; cursor: pointer; }
  .reload-btn:hover { color: #f3f4f6; background: #2d3345; }
  .metrics-strip { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
  .metric-card { background: #12151f; border: 1px solid #1f2430; border-radius: 6px; padding: 12px; display: flex; flex-direction: column; gap: 4px; }
  .card-label { font-size: 10px; color: #9ca3af; font-weight: 600; letter-spacing: 0.05em; }
  .card-value { font-size: 20px; font-weight: 700; color: #f3f4f6; }
  .card-value.cyan { color: #38bdf8; }
  .card-value.small { font-size: 14px; }
  .card-sub { font-size: 10px; color: #6b7280; }
  .discovery-card { background: #12151f; border: 1px solid #1f2430; border-radius: 6px; padding: 14px; display: flex; flex-direction: column; gap: 10px; }
  .discovery-header { display: flex; justify-content: space-between; align-items: center; }
  .section-title { font-size: 11px; font-weight: 700; color: #9ca3af; letter-spacing: 0.05em; }
  .source-pills { display: flex; gap: 6px; }
  .source-pill { background: #1f2430; border: 1px solid #2d3345; color: #9ca3af; font-size: 10px; padding: 3px 8px; border-radius: 3px; cursor: pointer; }
  .source-pill.active { background: #3b82f6; border-color: #3b82f6; color: #fff; font-weight: 600; }
  .search-bar { display: flex; gap: 8px; }
  .search-input { flex: 1; background: #0c0e14; border: 1px solid #2d3345; border-radius: 4px; padding: 8px 12px; color: #f3f4f6; font-size: 12px; outline: none; }
  .search-input:focus { border-color: #3b82f6; }
  .search-btn { display: flex; align-items: center; gap: 6px; background: #1f2430; border: 1px solid #3b82f6; color: #60a5fa; padding: 8px 14px; border-radius: 4px; font-size: 11px; font-weight: 600; cursor: pointer; }
  .search-btn:hover { background: #3b82f6; color: #fff; }
  .search-results-table { max-height: 180px; overflow-y: auto; border: 1px solid #1f2430; border-radius: 4px; }
  .search-results-table table { width: 100%; border-collapse: collapse; font-size: 11px; }
  .search-results-table th { background: #181c28; color: #9ca3af; padding: 6px 10px; text-align: left; font-size: 10px; border-bottom: 1px solid #1f2430; }
  .search-results-table td { padding: 6px 10px; border-bottom: 1px solid #181c28; }
  .name-cell { color: #9ca3af; max-width: 280px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub-btn { background: #065f46; color: #34d399; border: 1px solid #059669; border-radius: 3px; font-size: 10px; font-weight: 600; padding: 3px 8px; cursor: pointer; }
  .sub-btn:hover { background: #059669; color: #fff; }
  .table-container { background: #12151f; border: 1px solid #1f2430; border-radius: 6px; padding: 14px; display: flex; flex-direction: column; gap: 10px; flex: 1; }
  .table-header-strip { display: flex; justify-content: space-between; align-items: center; }
  .count-tag { font-size: 10px; color: #6b7280; font-weight: 600; }
  .table-wrapper { overflow-x: auto; flex: 1; }
  .data-table { width: 100%; border-collapse: collapse; font-size: 12px; }
  .data-table th { background: #181c28; color: #9ca3af; padding: 8px 12px; text-align: left; font-size: 10px; letter-spacing: 0.05em; border-bottom: 1px solid #1f2430; }
  .data-table td { padding: 8px 12px; border-bottom: 1px solid #181c28; }
  .data-table tr:hover { background: #181c2844; }
  .badge { display: inline-block; padding: 2px 6px; border-radius: 3px; font-size: 9px; font-weight: 700; letter-spacing: 0.05em; background: #374151; color: #e5e7eb; }
  .badge.yahoo { background: #581c87; color: #e9d5ff; }
  .badge.binance { background: #854d0e; color: #fef08a; }
  .badge.biquote { background: #1e3a8a; color: #bfdbfe; }
  .badge.polymarket { background: #065f46; color: #a7f3d0; }
  .badge.kalshi { background: #831843; color: #fbcfe8; }
  .badge-sub { font-size: 10px; color: #9ca3af; background: #1f2430; padding: 2px 6px; border-radius: 3px; }
  .status-pill { font-size: 9px; font-weight: 700; padding: 2px 6px; border-radius: 3px; background: #374151; color: #9ca3af; }
  .status-pill.active { background: #064e3b; color: #34d399; }
  .unsub-btn { background: #27272a; color: #ef4444; border: 1px solid #ef444433; border-radius: 3px; font-size: 10px; font-weight: 600; padding: 4px 8px; cursor: pointer; }
  .unsub-btn:hover { background: #7f1d1d; color: #fca5a5; }
  .bold { font-weight: 600; }
  .mono { font-family: monospace; font-size: 11px; }
  .text-white { color: #f3f4f6; }
  .text-muted { color: #6b7280; }
  .bull { color: #10b981; }
  .bear { color: #ef4444; }
  :global(.spinning) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
