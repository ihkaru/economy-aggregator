<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchMacroCalendar } from '../../lib/api';
  import type { RichEconomicEvent } from '../../lib/types';
  import Icons from '../Icons.svelte';

  let events: RichEconomicEvent[] = [];
  let isLoading: boolean = true;
  let selectedCountry: string = 'US';
  let selectedImportance: string = 'all';
  let searchQuery: string = '';
  let selectedEvent: RichEconomicEvent | null = null;

  const countries = ['ALL', 'US', 'EU', 'JP', 'GB', 'ID'];
  const importances = ['ALL', 'HIGH', 'MEDIUM', 'LOW'];

  async function loadData() {
    isLoading = true;
    try {
      const countryParam = selectedCountry === 'ALL' ? 'US' : selectedCountry;
      const impParam = selectedImportance === 'ALL' ? undefined : selectedImportance.toLowerCase();
      events = await fetchMacroCalendar(countryParam, impParam, 100);
    } catch (err) {
      console.error('Failed to load macro events', err);
    } finally {
      isLoading = false;
    }
  }

  $: filteredEvents = events.filter((ev) => {
    if (selectedCountry !== 'ALL' && ev.country !== selectedCountry) return false;
    if (selectedImportance !== 'ALL' && ev.importance.toLowerCase() !== selectedImportance.toLowerCase()) return false;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      return ev.title.toLowerCase().includes(q) || ev.indicator_code.toLowerCase().includes(q);
    }
    return true;
  });

  $: highImpactCount = events.filter((e) => e.importance === 'high').length;
  $: surpriseCount = events.filter((e) => e.actual_value !== null && e.forecast_value !== null && e.actual_value !== e.forecast_value).length;

  onMount(() => {
    loadData();
  });
</script>

<div class="macro-view tabular-nums">
  <!-- Top Bar: Filters & Metrics -->
  <div class="view-header">
    <div class="header-left">
      <div class="title-cluster">
        <Icons name="calendar" size={16} classNames="accent-icon" />
        <span class="view-title">MACRO HUB</span>
        <span class="view-sub">POINT-IN-TIME ECONOMIC CALENDAR</span>
      </div>

      <div class="filter-group">
        <span class="filter-label">COUNTRY:</span>
        {#each countries as c}
          <button
            class="filter-btn"
            class:active={selectedCountry === c}
            on:click={() => { selectedCountry = c; loadData(); }}
          >
            {c}
          </button>
        {/each}
      </div>

      <div class="filter-group">
        <span class="filter-label">IMPACT:</span>
        {#each importances as imp}
          <button
            class="filter-btn"
            class:active={selectedImportance === imp}
            on:click={() => { selectedImportance = imp; loadData(); }}
          >
            {imp}
          </button>
        {/each}
      </div>
    </div>

    <div class="header-right">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Filter indicator..."
        class="search-input"
      />
      <button class="reload-btn" on:click={loadData} title="Reload events">
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </div>

  <!-- Summary Cards -->
  <div class="metrics-strip">
    <div class="metric-card">
      <span class="card-label">TOTAL TRACKED RELEASES</span>
      <span class="card-value">{events.length}</span>
      <span class="card-sub">SDMX & Point-in-Time Lakehouse</span>
    </div>
    <div class="metric-card">
      <span class="card-label">HIGH IMPACT RELEASES</span>
      <span class="card-value amber">{highImpactCount}</span>
      <span class="card-sub">Central Bank & CPI Catalysts</span>
    </div>
    <div class="metric-card">
      <span class="card-label">CONSENSUS SURPRISES</span>
      <span class="card-value green">{surpriseCount}</span>
      <span class="card-sub">Actual ≠ Forecast Divergences</span>
    </div>
  </div>

  <!-- Main Table -->
  <div class="table-container">
    {#if isLoading}
      <div class="loading-state">
        <Icons name="loader" size={24} classNames="spinning" />
        <span>FETCHING POINT-IN-TIME RELEASES...</span>
      </div>
    {:else if filteredEvents.length === 0}
      <div class="empty-state">
        <span>No macroeconomic events found matching criteria.</span>
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>SCHEDULED (UTC)</th>
            <th>COUNTRY</th>
            <th>INDICATOR</th>
            <th>IMPACT</th>
            <th>ACTUAL</th>
            <th>FORECAST</th>
            <th>PREVIOUS</th>
            <th>SURPRISE</th>
            <th>REVISIONS</th>
            <th>SOURCE PROVENANCE</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredEvents as ev}
            {@const isSurprise = ev.actual_value !== null && ev.forecast_value !== null && ev.actual_value !== ev.forecast_value}
            {@const isBullish = ev.better_than_expected === true}
            <tr on:click={() => (selectedEvent = ev)} class:selected={selectedEvent?.event_id === ev.event_id}>
              <td class="date-cell">{ev.scheduled_utc.replace('T', ' ').slice(0, 16)}</td>
              <td><span class="country-tag">{ev.country}</span></td>
              <td class="title-cell">
                <span class="indicator-code">[{ev.indicator_code}]</span>
                <span class="indicator-name">{ev.title}</span>
              </td>
              <td>
                <span class="impact-tag {ev.importance}">{ev.importance.toUpperCase()}</span>
              </td>
              <td class="val-cell bold">
                {ev.actual_value !== null ? `${ev.actual_value} ${ev.unit_symbol}` : '-'}
              </td>
              <td class="val-cell">{ev.forecast_value !== null ? `${ev.forecast_value} ${ev.unit_symbol}` : '-'}</td>
              <td class="val-cell">{ev.previous_value !== null ? `${ev.previous_value} ${ev.unit_symbol}` : '-'}</td>
              <td>
                {#if isSurprise}
                  <span class="surprise-badge" class:bull={isBullish} class:bear={!isBullish}>
                    {isBullish ? '▲ ABOVE' : '▼ BELOW'}
                  </span>
                {:else if ev.actual_value !== null}
                  <span class="in-line-tag">IN LINE</span>
                {:else}
                  <span class="pending-tag">PENDING</span>
                {/if}
              </td>
              <td>
                <span class="revision-pill">{ev.revision_count} REVS</span>
              </td>
              <td>
                <a
                  href={ev.attribution.source_url}
                  target="_blank"
                  rel="noreferrer"
                  class="source-link"
                  on:click|stopPropagation
                >
                  <Icons name="external-link" size={11} />
                  <span>{ev.attribution.source_name.toUpperCase()}</span>
                </a>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Detail Inspector Drawer -->
  {#if selectedEvent}
    <div class="detail-drawer">
      <div class="drawer-header">
        <div class="drawer-title">
          <Icons name="calendar" size={14} />
          <span>{selectedEvent.indicator_code}: {selectedEvent.title}</span>
        </div>
        <button class="close-btn" on:click={() => (selectedEvent = null)}>
          <Icons name="close" size={13} />
        </button>
      </div>
      <div class="drawer-content">
        <div class="drawer-grid">
          <div class="drawer-item"><span class="k">Event ID:</span><span class="v">{selectedEvent.event_id}</span></div>
          <div class="drawer-item"><span class="k">Country:</span><span class="v">{selectedEvent.country}</span></div>
          <div class="drawer-item"><span class="k">Observation Period:</span><span class="v">{selectedEvent.period}</span></div>
          <div class="drawer-item"><span class="k">Attribution License:</span><span class="v">{selectedEvent.attribution.license_type}</span></div>
          <div class="drawer-item"><span class="k">Source Ingestion URL:</span><span class="v break">{selectedEvent.attribution.source_url}</span></div>
          <div class="drawer-item"><span class="k">Revision History:</span><span class="v">{selectedEvent.revision_count} point-in-time updates recorded</span></div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .macro-view { display: flex; flex-direction: column; height: 100%; width: 100%; background: var(--bg-surface); color: var(--text-main); font-size: 11px; overflow: hidden; position: relative; }
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
  .card-value.amber { color: var(--accent-amber); }
  .card-value.green { color: var(--bull-color); }
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
  .impact-tag { font-size: 8.5px; font-weight: 700; padding: 1px 4px; border: 1px solid transparent; }
  .impact-tag.high { color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.3); background: rgba(245, 158, 11, 0.08); }
  .impact-tag.medium { color: #60a5fa; border-color: rgba(96, 165, 250, 0.3); background: rgba(96, 165, 250, 0.08); }
  .impact-tag.low { color: var(--text-muted); border-color: var(--border-subtle); }
  .val-cell { color: var(--text-main); }
  .val-cell.bold { color: var(--text-bright); font-weight: 700; }
  .surprise-badge { font-size: 9px; font-weight: 700; padding: 1px 4px; }
  .surprise-badge.bull { color: var(--bull-color); background: rgba(8, 153, 129, 0.1); }
  .surprise-badge.bear { color: var(--bear-color); background: rgba(242, 54, 69, 0.1); }
  .in-line-tag { font-size: 9px; color: var(--text-muted); }
  .pending-tag { font-size: 9px; color: #555e75; }
  .revision-pill { background: rgba(255, 255, 255, 0.03); border: 1px solid var(--border-subtle); padding: 1px 4px; font-size: 8.5px; font-weight: 600; color: #93c5fd; }
  .source-link { display: inline-flex; align-items: center; gap: 4px; color: #60a5fa; text-decoration: none; font-size: 9px; font-weight: 600; }
  .source-link:hover { text-decoration: underline; }
  .loading-state, .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 200px; gap: 10px; color: var(--text-muted); }
  .detail-drawer { position: absolute; bottom: 0; left: 0; right: 0; background: #181c27; border-top: 1px solid var(--border-contrast); padding: 12px; box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.5); z-index: 20; }
  .drawer-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; border-bottom: 1px solid var(--border-subtle); padding-bottom: 6px; }
  .drawer-title { display: flex; align-items: center; gap: 6px; font-size: 11px; font-weight: 700; color: var(--text-bright); }
  .close-btn { background: transparent; border: none; color: var(--text-muted); cursor: pointer; }
  .drawer-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 8px; }
  .drawer-item { display: flex; flex-direction: column; font-size: 10px; }
  .drawer-item .k { color: var(--text-muted); font-weight: 600; margin-bottom: 2px; }
  .drawer-item .v { color: var(--text-bright); }
  .drawer-item .break { word-break: break-all; color: #60a5fa; }
  .spinning :global(svg) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>