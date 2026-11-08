<script lang="ts">
  import { onMount } from 'svelte';
  import type { InstrumentSummary, AssetClass } from '../lib/types';
  import Icons from './Icons.svelte';

  export let instruments: InstrumentSummary[] = [];
  export let isOpen: boolean = false;
  export let onSelect: (instrument: InstrumentSummary) => void;
  export let onClose: () => void;

  let searchQuery: string = '';
  let selectedFilter: string = 'ALL';
  let inputEl: HTMLInputElement;

  const categories = ['ALL', 'EQUITIES', 'COMMODITIES', 'FOREX', 'CRYPTO'];

  $: filteredInstruments = instruments.filter((item) => {
    const matchesFilter =
      selectedFilter === 'ALL' || item.asset_class.toUpperCase() === selectedFilter;
    const q = searchQuery.toLowerCase().trim();
    const matchesQuery =
      !q ||
      item.symbol.toLowerCase().includes(q) ||
      (item.canonical_symbol && item.canonical_symbol.toLowerCase().includes(q)) ||
      item.name.toLowerCase().includes(q);
    return matchesFilter && matchesQuery;
  });

  onMount(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '/' || ((e.ctrlKey || e.metaKey) && e.key === 'k')) {
        e.preventDefault();
        isOpen = true;
      }
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  });

  $: if (isOpen && inputEl) {
    setTimeout(() => inputEl.focus(), 50);
  }
</script>

{#if isOpen}
  <div
    class="modal-backdrop"
    on:click={(e) => { if (e.target === e.currentTarget) onClose(); }}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="presentation"
  >
    <div class="modal-window" role="dialog" aria-modal="true" tabindex="-1">
      <!-- Search Input Header -->
      <div class="modal-header">
        <span class="search-icon"><Icons name="search" size={16} /></span>
        <input
          bind:this={inputEl}
          type="text"
          placeholder="Search symbols or names (e.g. BBCA, GSPC, XAU, BTC)..."
          bind:value={searchQuery}
        />
        <button class="close-btn" on:click={onClose} title="Close (Esc)">
          <Icons name="close" size={15} />
        </button>
      </div>

      <!-- Category Filter Tabs -->
      <div class="filter-bar">
        {#each categories as cat}
          <button
            class="filter-tab"
            class:active={selectedFilter === cat}
            on:click={() => (selectedFilter = cat)}
          >
            {cat}
          </button>
        {/each}
      </div>

      <!-- Instrument List Table -->
      <div class="symbol-table-wrapper">
        <table class="symbol-table">
          <thead>
            <tr>
              <th style="width: 130px;">SYMBOL</th>
              <th>NAME</th>
              <th style="width: 100px;">ASSET</th>
              <th style="width: 80px;">SOURCE</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredInstruments as inst}
              <tr
                class="symbol-row"
                on:click={() => {
                  onSelect(inst);
                  onClose();
                }}
              >
                <td class="symbol-col">
                  <span class="sym-ticker">{inst.canonical_symbol || inst.symbol}</span>
                </td>
                <td class="name-col">{inst.name}</td>
                <td class="asset-col">
                  <span class="asset-tag">[{inst.asset_class.toUpperCase()}]</span>
                </td>
                <td class="source-col">{inst.source}</td>
              </tr>
            {/each}
            {#if filteredInstruments.length === 0}
              <tr>
                <td colspan="4" class="empty-cell">No matching instruments found in lakehouse.</td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>

      <!-- Footer Info -->
      <div class="modal-footer">
        <span>Use arrow keys or click to select | Press ESC to dismiss</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }

  .modal-window {
    width: 580px;
    max-width: 90vw;
    max-height: 80vh;
    background: var(--bg-panel);
    border: 1px solid var(--border-contrast);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.8);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
    padding: 0 12px;
    height: 42px;
  }

  .search-icon {
    color: var(--text-muted);
    margin-right: 8px;
  }

  input {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-bright);
    font-size: 13px;
    font-weight: 500;
    outline: none;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
  }

  .close-btn:hover {
    color: var(--text-bright);
  }

  .filter-bar {
    display: flex;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border-subtle);
    padding: 4px 8px;
    gap: 4px;
  }

  .filter-tab {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
    padding: 3px 8px;
    cursor: pointer;
    outline: none;
  }

  .filter-tab:hover {
    color: var(--text-main);
    background: var(--bg-hover);
  }

  .filter-tab.active {
    background: var(--bg-surface);
    border-color: var(--border-subtle);
    color: var(--text-bright);
  }

  .symbol-table-wrapper {
    flex: 1;
    overflow-y: auto;
    max-height: 380px;
  }

  .symbol-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
    text-align: left;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--bg-surface);
    color: var(--text-muted);
    font-weight: 600;
    padding: 6px 12px;
    font-size: 10px;
    border-bottom: 1px solid var(--border-subtle);
    letter-spacing: 0.5px;
  }

  td {
    padding: 7px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .symbol-row {
    cursor: pointer;
    transition: background 0.08s;
  }

  .symbol-row:hover {
    background: var(--bg-hover);
  }

  .sym-ticker {
    font-weight: 700;
    color: var(--text-bright);
    letter-spacing: 0.5px;
  }

  .name-col {
    color: var(--text-main);
  }

  .asset-tag {
    color: var(--accent-amber);
    font-size: 9.5px;
    font-weight: 600;
  }

  .source-col {
    color: var(--text-muted);
    font-size: 10px;
  }

  .empty-cell {
    text-align: center;
    color: var(--text-muted);
    padding: 24px;
  }

  .modal-footer {
    padding: 6px 12px;
    background: var(--bg-surface);
    border-top: 1px solid var(--border-subtle);
    color: var(--text-muted);
    font-size: 10px;
    display: flex;
    justify-content: space-between;
  }
</style>
