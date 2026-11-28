<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Icons from '../Icons.svelte';
  import AtlasTankCard from '../atlas/AtlasTankCard.svelte';
  import AtlasDiagnosticStrip from '../atlas/AtlasDiagnosticStrip.svelte';
  import {
    fetchLiquidityAtlas,
    fetchLiquidityTransitions,
    fetchProspectiveScorecard,
  } from '../../lib/atlas_api';
  import type {
    LiquidityAtlasResponse,
    ProspectiveScorecardReport,
    RegimeTransitionEvent,
  } from '../../lib/atlas_types';

  let atlas: LiquidityAtlasResponse | null = null;
  let transitions: RegimeTransitionEvent[] = [];
  let scorecard: ProspectiveScorecardReport | null = null;

  let asOfInput: string = '';
  let rangePreset: string = '1y';
  let windowWeeks: number = 4;
  let syncedTime: string | null = null;
  let diagTab: 'tree' | 'transitions' | 'prospective' = 'tree';

  let isLoading: boolean = true;
  let errorMsg: string | null = null;

  const ranges = ['12w', '6m', '1y', '3y', '5y', 'all'];
  const windows = [1, 4, 12];

  function shiftDateStr(dateStr: string, deltaDays: number, minBound?: string, maxBound?: string): string {
    const base = dateStr ? new Date(`${dateStr}T00:00:00Z`) : new Date();
    base.setUTCDate(base.getUTCDate() + deltaDays);
    let next = base.toISOString().slice(0, 10);
    if (minBound && next < minBound) next = minBound;
    if (maxBound && next > maxBound) next = maxBound;
    return next;
  }

  async function loadAtlas() {
    isLoading = true;
    errorMsg = null;
    syncedTime = null;
    try {
      const res = await fetchLiquidityAtlas({
        asOf: asOfInput || undefined,
        range: rangePreset,
        windowWeeks,
        includeSpillover: false,
      });
      atlas = res;
      if (!asOfInput) {
        asOfInput = res.as_of_date;
      }
      // Refresh secondary diagnostic tabs without blocking the 6 charts
      Promise.all([
        fetchLiquidityTransitions(15).catch(() => []),
        fetchProspectiveScorecard(res.as_of_date).catch(() => null),
      ]).then(([tr, sc]) => {
        transitions = tr;
        if (sc) scorecard = sc;
      });
    } catch (err: any) {
      errorMsg = err?.message || 'Failed to load Liquidity & Capital Flow Atlas';
    } finally {
      isLoading = false;
    }
  }

  function handleShift(days: number) {
    if (isLoading) return;
    const base = asOfInput || atlas?.as_of_date || new Date().toISOString().slice(0, 10);
    const next = shiftDateStr(
      base,
      days,
      atlas?.earliest_available_date,
      atlas?.latest_available_date
    );
    if (next === asOfInput) return;
    asOfInput = next;
    loadAtlas();
  }

  function handleJumpLatest() {
    if (isLoading || !atlas) return;
    if (asOfInput === atlas.latest_available_date) return;
    asOfInput = atlas.latest_available_date;
    loadAtlas();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement ||
      e.target instanceof HTMLSelectElement
    ) {
      return;
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      handleShift(e.shiftKey ? -28 : -7);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      handleShift(e.shiftKey ? 28 : 7);
    }
  }

  $: earliest = atlas?.earliest_available_date || '2016-01-04';
  $: latest = atlas?.latest_available_date || '';
  $: atEarliest = Boolean(earliest && asOfInput && asOfInput <= earliest);
  $: atLatest = Boolean(latest && asOfInput && asOfInput >= latest);

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    loadAtlas();
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="atlas-view tabular-nums">
  <!-- Sticky Top Time-Travel & Range Toolbar -->
  <header class="atlas-toolbar">
    <div class="title-area">
      <Icons name="layers" size={16} />
      <div class="title-text">
        <span class="view-title">CAPITAL FLOW &amp; LIQUIDITY ATLAS (6 TANKS)</span>
        <span class="view-sub">ZERO-LOOKAHEAD HISTORICAL REPLAY | RAW AS-IS VS DERIVED TRANSPARENCY</span>
      </div>
    </div>

    <!-- Left / Right Date Shifting Controls -->
    <div class="date-shifter" role="group" aria-label="Historical date navigation controls">
      <button
        type="button"
        class="shift-btn"
        disabled={isLoading || atEarliest}
        on:click={() => handleShift(-28)}
        title="Shift backward 4 weeks (Shift + ArrowLeft)"
      >
        <Icons name="chevrons-left" size={13} />
        <span>-4W</span>
      </button>
      <button
        type="button"
        class="shift-btn primary-shift"
        disabled={isLoading || atEarliest}
        on:click={() => handleShift(-7)}
        title="Shift backward 1 week (ArrowLeft)"
      >
        <Icons name="chevron-left" size={14} />
        <span>-1W</span>
      </button>

      <label class="date-input-wrap" title="Jump directly to any As-Of date (zero lookahead)">
        <span class="date-tag">AS-OF</span>
        <input
          type="date"
          class="date-input"
          bind:value={asOfInput}
          min={earliest}
          max={latest || undefined}
          on:change={loadAtlas}
        />
      </label>

      <button
        type="button"
        class="shift-btn primary-shift"
        disabled={isLoading || atLatest}
        on:click={() => handleShift(7)}
        title="Shift forward 1 week (ArrowRight)"
      >
        <span>+1W</span>
        <Icons name="chevron-right" size={14} />
      </button>
      <button
        type="button"
        class="shift-btn"
        disabled={isLoading || atLatest}
        on:click={() => handleShift(28)}
        title="Shift forward 4 weeks (Shift + ArrowRight)"
      >
        <span>+4W</span>
        <Icons name="chevrons-right" size={13} />
      </button>

      <button
        type="button"
        class="shift-btn latest-btn"
        disabled={isLoading || atLatest || !latest}
        on:click={handleJumpLatest}
        title="Jump to latest settled date ({latest || 'loading'})"
      >
        LATEST
      </button>
    </div>

    <!-- Range & Window Switchers -->
    <div class="filter-group">
      <div class="pill-group" role="group" aria-label="Chart horizon range">
        {#each ranges as r}
          <button
            type="button"
            class="seg-btn"
            class:active={rangePreset === r}
            disabled={isLoading}
            on:click={() => {
              rangePreset = r;
              loadAtlas();
            }}
          >
            {r.toUpperCase()}
          </button>
        {/each}
      </div>

      <div class="pill-group" role="group" aria-label="Flow summation window">
        {#each windows as w}
          <button
            type="button"
            class="seg-btn"
            class:active={windowWeeks === w}
            disabled={isLoading}
            on:click={() => {
              windowWeeks = w;
              loadAtlas();
            }}
          >
            {w}W
          </button>
        {/each}
      </div>

      <button
        type="button"
        class="reload-btn"
        disabled={isLoading}
        on:click={loadAtlas}
        title="Refresh Atlas snapshot"
      >
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </header>

  <!-- Synchronized Cursor & Bounds Status Ribbon -->
  <div class="status-ribbon">
    {#if syncedTime}
      <span class="cursor-active">SYNCED CURSOR ACROSS ALL 6 TANKS: <strong>{syncedTime}</strong></span>
    {:else if atlas}
      <span>
        SNAPSHOT: <strong>{atlas.as_of_date}</strong> (Window {atlas.range_start_date} to {atlas.as_of_date}) | Lake Bounds: {atlas.earliest_available_date} .. {atlas.latest_available_date}
      </span>
    {:else}
      <span>Initializing 6-Tank Liquidity &amp; Capital Flow Atlas...</span>
    {/if}
    <span class="kbd-hint">Keys: ArrowLeft / ArrowRight (+Shift = 4W)</span>
  </div>

  <!-- Main Scrollable Viewport -->
  <div class="atlas-scroll">
    {#if errorMsg}
      <div class="error-box" role="alert">
        <span>ERROR: {errorMsg}</span>
        <button type="button" class="retry-btn" on:click={loadAtlas}>RETRY</button>
      </div>
    {/if}

    {#if isLoading && !atlas}
      <div class="loading-box">
        <Icons name="loader" size={18} classNames="spinning" />
        <span>Computing zero-lookahead 6-Tank flow snapshot...</span>
      </div>
    {:else if atlas}
      <div class="tanks-grid" class:dimmed={isLoading}>
        {#each atlas.tanks as tank (tank.tank_id)}
          <AtlasTankCard
            {tank}
            windowWeeks={atlas.window_weeks}
            {syncedTime}
            onHoverTime={(t) => (syncedTime = t)}
          />
        {/each}
      </div>

      <AtlasDiagnosticStrip
        tree={atlas.diagnostic_tree}
        {transitions}
        {scorecard}
        activeTab={diagTab}
        onSelectTab={(tab) => (diagTab = tab)}
      />
    {/if}
  </div>
</div>

<style>
  .atlas-view {
    display: flex; flex-direction: column; height: 100%; width: 100%;
    background: var(--bg-main); color: var(--text-main); overflow: hidden;
  }
  .atlas-toolbar {
    background: var(--bg-panel); border-bottom: 1px solid var(--border-subtle);
    padding: 7px 12px; display: flex; flex-wrap: wrap; align-items: center;
    justify-content: space-between; gap: 8px 12px; flex-shrink: 0;
  }
  .title-area { display: flex; align-items: center; gap: 8px; min-width: 200px; }
  .title-text { display: flex; flex-direction: column; min-width: 0; }
  .view-title { font-size: 12px; font-weight: 700; color: var(--text-bright); letter-spacing: 0.4px; }
  .view-sub { font-size: 9px; color: #94a3b8; overflow-wrap: anywhere; }
  .date-shifter { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
  .shift-btn, .seg-btn, .reload-btn {
    background: var(--bg-surface); border: 1px solid var(--border-contrast); color: var(--text-main);
    height: 28px; padding: 0 8px; display: inline-flex; align-items: center; gap: 4px;
    font-size: 10.5px; font-weight: 700; cursor: pointer;
  }
  .shift-btn.primary-shift { border-color: #3b82f6; color: #93c5fd; }
  .shift-btn.latest-btn { border-color: rgba(8, 153, 129, 0.6); color: #34d399; }
  .shift-btn:hover:not(:disabled), .seg-btn:hover:not(:disabled), .reload-btn:hover:not(:disabled) {
    background: var(--bg-hover); color: var(--text-bright); border-color: #60a5fa;
  }
  .shift-btn:disabled, .seg-btn:disabled, .reload-btn:disabled { opacity: 0.35; cursor: not-allowed; }
  .date-input-wrap {
    display: inline-flex; align-items: center; gap: 5px; background: #090b10;
    border: 1px solid var(--border-contrast); height: 28px; padding: 0 7px;
  }
  .date-tag { font-size: 9px; font-weight: 700; color: #94a3b8; }
  .date-input {
    background: transparent; border: none; color: var(--text-bright);
    font-size: 11px; font-weight: 700; outline: none; color-scheme: dark;
  }
  .filter-group { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .pill-group { display: inline-flex; }
  .seg-btn { border-right-width: 0; padding: 0 7px; color: #94a3b8; }
  .pill-group .seg-btn:last-child { border-right-width: 1px; }
  .seg-btn.active { background: var(--bg-active); color: var(--text-bright); border-color: #3b82f6; }
  .reload-btn { width: 28px; justify-content: center; padding: 0; }
  .shift-btn:focus-visible, .seg-btn:focus-visible, .reload-btn:focus-visible, .date-input:focus-visible {
    outline: 2px solid #3b82f6; outline-offset: 1px;
  }
  .status-ribbon {
    background: #0a0c12; border-bottom: 1px solid var(--border-subtle); padding: 4px 12px;
    display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 6px;
    font-size: 10px; color: #94a3b8; flex-shrink: 0;
  }
  .cursor-active { color: #60a5fa; }
  .kbd-hint { color: #94a3b8; font-size: 9.5px; }
  .atlas-scroll {
    flex: 1; overflow-y: auto; overflow-x: hidden; padding: 10px 12px 16px;
    display: flex; flex-direction: column; gap: 12px;
  }
  .tanks-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
  .tanks-grid.dimmed { opacity: 0.6; transition: opacity 0.15s ease-out; }
  .error-box {
    background: rgba(242, 54, 69, 0.15); border: 1px solid #f23645; color: #fecaca;
    padding: 8px 12px; display: flex; justify-content: space-between; align-items: center; gap: 12px;
  }
  .retry-btn { background: #141722; border: 1px solid #f87171; color: #fff; padding: 4px 10px; font-weight: 700; cursor: pointer; }
  .loading-box { display: flex; align-items: center; justify-content: center; gap: 10px; height: 220px; color: #94a3b8; font-size: 11.5px; }
  :global(.spinning) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
  @media (max-width: 1439px) {
    .tanks-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 899px) {
    .tanks-grid { grid-template-columns: minmax(0, 1fr); }
    .shift-btn, .seg-btn, .reload-btn, .date-input-wrap { min-height: 44px; }
  }
</style>
