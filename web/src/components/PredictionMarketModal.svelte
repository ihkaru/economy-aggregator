<script lang="ts">
  import { onMount } from 'svelte';
  import Icons from './Icons.svelte';
  import { fetchPredictionMarkets } from '../lib/api';
  import type { PredictionMarketContract } from '../lib/types';

  export let isOpen: boolean = false;
  export let onClose: () => void;

  let contracts: PredictionMarketContract[] = [];
  let isLoading: boolean = false;
  let selectedVenue: string = 'all';
  let searchQuery: string = '';
  let errorMsg: string | null = null;

  onMount(async () => {
    await loadContracts();
  });

  async function loadContracts() {
    isLoading = true;
    errorMsg = null;
    try {
      contracts = await fetchPredictionMarkets(
        selectedVenue === 'all' ? undefined : selectedVenue,
        50
      );
    } catch (e: any) {
      errorMsg = e.message || 'Failed to load prediction market contracts';
    } finally {
      isLoading = false;
    }
  }

  function handleVenueChange(venue: string) {
    selectedVenue = venue;
    loadContracts();
  }

  $: filteredContracts = contracts.filter((c) => {
    if (!searchQuery) return true;
    const q = searchQuery.toLowerCase();
    return c.title.toLowerCase().includes(q) || c.venue_market_id.toLowerCase().includes(q);
  });
</script>

{#if isOpen}
  <div class="modal-backdrop" on:click={onClose} role="presentation">
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <div
      class="modal-dialog"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      aria-label="Prediction Markets Catalog"
    >
      <div class="modal-header">
        <div class="header-left">
          <Icons name="activity" size={14} classNames="icon-gold" />
          <span class="modal-title">PREDICTION MARKETS (PROBABILITY [0.0, 1.0])</span>
          <span class="venue-count">16 VENUES UNIFIED</span>
        </div>
        <button class="close-btn" on:click={onClose} title="Close Modal">
          <Icons name="x" size={14} />
        </button>
      </div>

      <!-- Controls & Search -->
      <div class="modal-controls">
        <div class="search-box">
          <Icons name="search" size={13} classNames="icon-dim" />
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search contracts (e.g. CPI, Fed Rate, Election)..."
            class="search-input"
          />
        </div>

        <div class="venue-tabs">
          <button
            class="tab-btn"
            class:active={selectedVenue === 'all'}
            on:click={() => handleVenueChange('all')}
          >
            ALL
          </button>
          <button
            class="tab-btn"
            class:active={selectedVenue === 'kalshi'}
            on:click={() => handleVenueChange('kalshi')}
          >
            KALSHI (CFTC)
          </button>
          <button
            class="tab-btn"
            class:active={selectedVenue === 'polymarket'}
            on:click={() => handleVenueChange('polymarket')}
          >
            POLYMARKET (CLOB)
          </button>
        </div>
      </div>

      <!-- Contract List -->
      <div class="contract-list tabular-nums">
        {#if isLoading}
          <div class="state-msg">
            <span class="sync-dot"></span>
            <span>FETCHING LIVE CONTRACT SPECS & ORACLES...</span>
          </div>
        {:else if errorMsg}
          <div class="state-msg error">{errorMsg}</div>
        {:else if filteredContracts.length === 0}
          <div class="state-msg">NO MATCHING CONTRACTS FOUND</div>
        {:else}
          {#each filteredContracts as contract}
            <div class="contract-row">
              <div class="row-header">
                <span
                  class="venue-badge"
                  class:kalshi-badge={contract.venue === 'kalshi'}
                  class:poly-badge={contract.venue === 'polymarket'}
                >
                  {contract.venue.toUpperCase()}
                </span>
                <span class="ticker-text">{contract.venue_market_id}</span>
                <span class="status-tag" class:active-tag={contract.is_active}>
                  {contract.status.toUpperCase()}
                </span>
              </div>

              <div class="row-title">{contract.title}</div>

              <div class="row-meta">
                <div class="oracle-info">
                  <span class="meta-label">ORACLE:</span>
                  <span class="meta-val">{contract.oracle.source_name}</span>
                </div>
                <div class="outcome-chips">
                  {#each contract.outcomes as outcome}
                    <span class="chip">{outcome}</span>
                  {/each}
                </div>
              </div>

              {#if contract.venue_url}
                <div class="row-footer">
                  <a
                    href={contract.venue_url}
                    target="_blank"
                    rel="noreferrer"
                    class="proof-link"
                  >
                    VIEW OFFICIAL VENUE LISTING →
                  </a>
                </div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal-dialog {
    width: 680px;
    max-height: 80vh;
    background: var(--bg-panel);
    border: 1px solid var(--border-contrast);
    display: flex;
    flex-direction: column;
    font-size: 11px;
    border-radius: 0px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
  }

  .modal-header {
    height: 38px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    color: var(--text-bright);
  }
  .venue-count {
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
  }
  .close-btn:hover {
    color: var(--text-bright);
  }

  .modal-controls {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    gap: 8px;
    background: var(--bg-panel);
  }

  .search-box {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border-contrast);
    padding: 0 8px;
    height: 26px;
  }

  .search-input {
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-bright);
    font-size: 11px;
    width: 100%;
  }

  .venue-tabs {
    display: flex;
    gap: 4px;
  }
  .tab-btn {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    padding: 0 8px;
    font-size: 10px;
    cursor: pointer;
    border-radius: 0px;
  }
  .tab-btn.active {
    background: var(--text-bright);
    color: var(--bg-surface);
    border-color: var(--text-bright);
    font-weight: 600;
  }

  .contract-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .state-msg {
    padding: 32px;
    text-align: center;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .state-msg.error {
    color: #ef4444;
  }

  .contract-row {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-radius: 0px;
  }
  .row-header { display: flex; align-items: center; gap: 6px; }
  .venue-badge { font-size: 9px; padding: 1px 4px; font-weight: 600; }
  .kalshi-badge { background: rgba(37, 99, 235, 0.15); color: #3b82f6; border: 1px solid #3b82f6; }
  .poly-badge { background: rgba(168, 85, 247, 0.15); color: #a855f7; border: 1px solid #a855f7; }
  .ticker-text { font-weight: 600; color: var(--text-muted); font-size: 10px; }
  .status-tag { margin-left: auto; font-size: 9px; color: var(--text-muted); padding: 1px 4px; border: 1px solid var(--border-subtle); }
  .status-tag.active-tag { color: #10b981; border-color: #10b981; }
  .row-title { font-size: 12px; font-weight: 600; color: var(--text-bright); }
  .row-meta { display: flex; justify-content: space-between; align-items: center; font-size: 10px; margin-top: 2px; }
  .oracle-info { color: var(--text-muted); }
  .meta-label { font-size: 9px; }
  .meta-val { color: var(--text-bright); }
  .outcome-chips { display: flex; gap: 4px; }
  .chip { font-size: 9px; background: var(--bg-panel); border: 1px solid var(--border-subtle); padding: 1px 5px; color: var(--text-bright); }
  .row-footer { margin-top: 4px; font-size: 9px; }
  .proof-link { color: var(--accent); text-decoration: underline; }
  .proof-link:hover { color: var(--text-bright); }
  .sync-dot { width: 6px; height: 6px; background: #3b82f6; animation: pulse 1s infinite alternate; }
  @keyframes pulse { from { opacity: 0.3; } to { opacity: 1; } }
</style>
