<script lang="ts">
  import { onMount } from 'svelte';
  import Icons from './Icons.svelte';
  import { fetchMacroCalendar } from '../lib/api';
  import type { RichEconomicEvent } from '../lib/types';

  export let isOpen: boolean = false;
  export let onClose: () => void;

  let events: RichEconomicEvent[] = [];
  let isLoading: boolean = false;
  let selectedCountry: string = 'US';
  let selectedImportance: string = 'all';
  let errorMsg: string | null = null;

  const countries = ['US', 'EU', 'GB', 'ID', 'JP', 'CN', 'AU'];

  onMount(async () => {
    await loadEvents();
  });

  async function loadEvents() {
    isLoading = true;
    errorMsg = null;
    try {
      events = await fetchMacroCalendar(
        selectedCountry,
        selectedImportance === 'all' ? undefined : selectedImportance,
        50
      );
    } catch (e: any) {
      errorMsg = e.message || 'Failed to load economic calendar';
    } finally {
      isLoading = false;
    }
  }

  function handleCountryChange(c: string) {
    selectedCountry = c;
    loadEvents();
  }

  function handleImportanceChange(imp: string) {
    selectedImportance = imp;
    loadEvents();
  }
</script>

{#if isOpen}
  <aside class="macro-drawer" aria-label="Macroeconomic Calendar">
    <div class="drawer-header">
      <div class="header-title">
        <Icons name="calendar" size={14} classNames="icon-gold" />
        <span class="title-text">ECONOMIC CALENDAR</span>
        <span class="source-badge">SDMX / POINT-IN-TIME</span>
      </div>
      <button class="close-btn" on:click={onClose} title="Close Calendar">
        <Icons name="x" size={14} />
      </button>
    </div>

    <!-- Filters Bar -->
    <div class="filters-bar">
      <div class="country-pills">
        {#each countries as c}
          <button
            class="pill-btn"
            class:active={selectedCountry === c}
            on:click={() => handleCountryChange(c)}
          >
            {c}
          </button>
        {/each}
      </div>

      <div class="importance-pills">
        <button
          class="imp-btn"
          class:active={selectedImportance === 'all'}
          on:click={() => handleImportanceChange('all')}
        >
          ALL
        </button>
        <button
          class="imp-btn imp-high"
          class:active={selectedImportance === 'high'}
          on:click={() => handleImportanceChange('high')}
        >
          HIGH
        </button>
      </div>
    </div>

    <!-- Event List -->
    <div class="events-scroll tabular-nums">
      {#if isLoading}
        <div class="loading-state">
          <span class="sync-dot"></span>
          <span>QUERYING PARQUET LAKE / SDMX...</span>
        </div>
      {:else if errorMsg}
        <div class="error-state">
          <span>{errorMsg}</span>
        </div>
      {:else if events.length === 0}
        <div class="empty-state">
          <span>NO SCHEDULED RELEASES IN THIS WINDOW</span>
        </div>
      {:else}
        {#each events as event}
          <div class="event-card">
            <div class="event-top">
              <span class="event-time">
                {event.scheduled_utc.replace('T', ' ').replace('Z', ' UTC').slice(0, 19)}
              </span>
              <span
                class="tier-badge"
                class:tier-high={event.impact_tier === 3}
                class:tier-med={event.impact_tier === 2}
                class:tier-low={event.impact_tier <= 1}
              >
                {event.impact_tier === 3 ? 'HIGH' : event.impact_tier === 2 ? 'MED' : 'LOW'}
              </span>
            </div>

            <div class="event-main">
              <span class="event-country">[{event.country}]</span>
              <span class="event-title">{event.title}</span>
            </div>

            <div class="event-metrics">
              <div class="metric-item">
                <span class="metric-label">ACTUAL</span>
                <span
                  class="metric-val"
                  class:bull={event.better_than_expected === true}
                  class:bear={event.better_than_expected === false}
                >
                  {event.raw_actual || '-'}
                </span>
              </div>
              <div class="metric-item">
                <span class="metric-label">FORECAST</span>
                <span class="metric-val">{event.raw_forecast || '-'}</span>
              </div>
              <div class="metric-item">
                <span class="metric-label">PREVIOUS</span>
                <span class="metric-val">{event.raw_previous || '-'}</span>
              </div>
            </div>

            <div class="event-footer">
              <span class="agency-tag">
                {event.attribution.agency_name || event.attribution.provider.toUpperCase()}
              </span>
              {#if event.attribution.source_url}
                <a
                  href={event.attribution.source_url}
                  target="_blank"
                  rel="noreferrer"
                  class="source-link"
                >
                  SOURCE PROOF
                </a>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </aside>
{/if}

<style>
  .macro-drawer {
    position: absolute;
    top: 38px;
    right: 0;
    bottom: 24px;
    width: 380px;
    background: var(--bg-panel);
    border-left: 1px solid var(--border-contrast);
    display: flex;
    flex-direction: column;
    z-index: 50;
    font-size: 11px;
    box-shadow: -4px 0 16px rgba(0, 0, 0, 0.4);
  }

  .drawer-header {
    height: 38px;
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px;
    background: var(--bg-surface);
  }

  .header-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: var(--text-bright);
  }

  .source-badge {
    font-size: 9px;
    background: var(--bg-hover);
    color: var(--text-muted);
    padding: 1px 4px;
    border: 1px solid var(--border-subtle);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .close-btn:hover {
    color: var(--text-bright);
  }

  .filters-bar {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    justify-content: space-between;
    gap: 6px;
    background: var(--bg-panel);
  }

  .country-pills, .importance-pills {
    display: flex;
    gap: 4px;
  }

  .pill-btn, .imp-btn {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    padding: 2px 6px;
    font-size: 10px;
    cursor: pointer;
    border-radius: 0px;
  }
  .pill-btn.active, .imp-btn.active {
    background: var(--text-bright);
    color: var(--bg-surface);
    border-color: var(--text-bright);
    font-weight: 600;
  }

  .imp-high.active {
    background: #ef4444;
    color: #ffffff;
    border-color: #ef4444;
  }

  .events-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .loading-state, .error-state, .empty-state {
    padding: 24px;
    text-align: center;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .event-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-radius: 0px;
  }

  .event-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .event-time {
    color: var(--text-muted);
    font-size: 10px;
  }

  .tier-badge {
    font-size: 9px;
    padding: 1px 4px;
    font-weight: 600;
  }
  .tier-high {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid #ef4444;
  }
  .tier-med {
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
    border: 1px solid #f59e0b;
  }
  .tier-low {
    background: var(--bg-hover);
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .event-main {
    display: flex;
    gap: 4px;
    font-weight: 600;
    color: var(--text-bright);
  }
  .event-country {
    color: var(--accent);
  }

  .event-metrics {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 4px;
    background: var(--bg-panel);
    padding: 4px;
    margin-top: 2px;
  }
  .metric-item {
    display: flex;
    flex-direction: column;
  }
  .metric-label {
    font-size: 8px;
    color: var(--text-muted);
  }
  .metric-val {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-bright);
  }

  .event-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 4px;
    font-size: 9px;
  }
  .agency-tag {
    color: var(--text-muted);
  }
  .source-link {
    color: var(--accent);
    text-decoration: underline;
  }
  .source-link:hover {
    color: var(--text-bright);
  }

  .bull { color: #10b981 !important; }
  .bear { color: #ef4444 !important; }
  .sync-dot {
    width: 6px;
    height: 6px;
    background: #3b82f6;
    animation: pulse 1s infinite alternate;
  }
  @keyframes pulse {
    from { opacity: 0.3; }
    to { opacity: 1; }
  }
</style>
