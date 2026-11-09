<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchStoragePartitions } from '../../lib/api';
  import type { PartitionItem } from '../../lib/types';
  import Icons from '../Icons.svelte';

  let partitions: PartitionItem[] = [];
  let isLoading: boolean = true;
  let selectedDomain: string = 'ALL';
  let searchQuery: string = '';

  const domains = ['ALL', 'market', 'macro', 'prediction_markets'];

  async function loadData() {
    isLoading = true;
    try {
      partitions = await fetchStoragePartitions();
    } catch (err) {
      console.error('Failed to load lakehouse partitions', err);
    } finally {
      isLoading = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return bytes + ' B';
  }

  $: filtered = partitions.filter((p) => {
    if (selectedDomain !== 'ALL' && p.domain !== selectedDomain) return false;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      return (
        p.identifier.toLowerCase().includes(q) ||
        p.domain.toLowerCase().includes(q) ||
        p.file_name.toLowerCase().includes(q)
      );
    }
    return true;
  });

  $: totalRows = partitions.reduce((sum, p) => sum + p.row_count, 0);
  $: totalBytes = partitions.reduce((sum, p) => sum + p.size_bytes, 0);

  onMount(() => {
    loadData();
  });
</script>

<div class="lakehouse-view tabular-nums">
  <!-- Top Bar -->
  <div class="view-header">
    <div class="header-left">
      <div class="title-cluster">
        <Icons name="folder" size={16} classNames="accent-icon" />
        <span class="view-title">LAKEHOUSE EXPLORER</span>
        <span class="view-sub">PARQUET L1 PARTITION HIERARCHY & ROW-COUNT METRICS</span>
      </div>

      <div class="filter-group">
        <span class="filter-label">DOMAIN:</span>
        {#each domains as d}
          <button
            class="filter-btn"
            class:active={selectedDomain === d}
            on:click={() => (selectedDomain = d)}
          >
            {d.toUpperCase()}
          </button>
        {/each}
      </div>
    </div>

    <div class="header-right">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Filter symbol/file..."
        class="search-input"
      />
      <button class="reload-btn" on:click={loadData} title="Rescan Parquet metadata">
        <Icons name="refresh" size={13} classNames={isLoading ? 'spinning' : ''} />
      </button>
    </div>
  </div>

  <!-- Summary Cards -->
  <div class="metrics-strip">
    <div class="metric-card">
      <span class="card-label">TOTAL PARQUET FILES</span>
      <span class="card-value">{partitions.length}</span>
      <span class="card-sub">Snappy Compressed L1 Storage Chunks</span>
    </div>
    <div class="metric-card">
      <span class="card-label">CONFIRMED LAKE ROWS</span>
      <span class="card-value cyan">{totalRows.toLocaleString()}</span>
      <span class="card-sub">Zero-Decompression File Metadata Count</span>
    </div>
    <div class="metric-card">
      <span class="card-label">TOTAL STORAGE FOOTPRINT</span>
      <span class="card-value green">{formatBytes(totalBytes)}</span>
      <span class="card-sub">Compressed On-Disk Footprint</span>
    </div>
  </div>

  <!-- Table Container -->
  <div class="table-container">
    {#if isLoading}
      <div class="loading-state">
        <Icons name="loader" size={24} classNames="spinning" />
        <span>READING PARQUET FILE METADATA HEADERS...</span>
      </div>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <span>No lakehouse partition files matching criteria.</span>
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>DOMAIN</th>
            <th>IDENTIFIER</th>
            <th>TIMEFRAME</th>
            <th>PARTITION (YYYY-MM)</th>
            <th>PARQUET FILE</th>
            <th>ROW COUNT</th>
            <th>COMPRESSED SIZE</th>
            <th>COMPRESSION</th>
            <th>STATUS</th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as p}
            <tr>
              <td>
                <span class="domain-tag {p.domain}">{p.domain.toUpperCase()}</span>
              </td>
              <td class="identifier-cell">
                <span class="id-text">{p.identifier}</span>
              </td>
              <td>
                <span class="tf-tag">{p.timeframe.toUpperCase()}</span>
              </td>
              <td class="date-cell">{p.year}-{p.month.toString().padStart(2, '0')}</td>
              <td class="filename-cell">
                <code>{p.file_name}</code>
              </td>
              <td class="val-cell bold">{p.row_count.toLocaleString()}</td>
              <td class="val-cell">{formatBytes(p.size_bytes)}</td>
              <td><span class="codec-tag">SNAPPY</span></td>
              <td>
                <div class="status-cell">
                  <span class="dot-green"></span>
                  <span>VERIFIED</span>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .lakehouse-view { display: flex; flex-direction: column; height: 100%; width: 100%; background: var(--bg-surface); color: var(--text-main); font-size: 11px; overflow: hidden; position: relative; }
  .view-header { height: 42px; background: var(--bg-panel); border-bottom: 1px solid var(--border-subtle); display: flex; align-items: center; justify-content: space-between; padding: 0 12px; gap: 12px; flex-shrink: 0; }
  .header-left, .header-right { display: flex; align-items: center; gap: 8px; }
  .title-cluster { display: flex; align-items: center; gap: 6px; }
  .view-title { font-size: 12px; font-weight: 700; color: var(--text-bright); letter-spacing: 0.5px; }
  .view-sub { font-size: 9px; color: var(--text-muted); border-left: 1px solid var(--border-subtle); padding-left: 6px; }
  .filter-group { display: flex; align-items: center; gap: 2px; }
  .filter-label { font-size: 9px; color: var(--text-muted); font-weight: 700; margin-right: 2px; }
  .filter-btn { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-muted); height: 24px; padding: 0 6px; font-size: 9.5px; font-weight: 600; cursor: pointer; border-radius: 0px; outline: none; }
  .filter-btn.active { background: var(--bg-active); border-color: var(--border-contrast); color: var(--text-bright); font-weight: 700; }
  .search-input { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-bright); height: 24px; padding: 0 8px; font-size: 10.5px; outline: none; width: 140px; border-radius: 0px; }
  .reload-btn { background: var(--bg-surface); border: 1px solid var(--border-subtle); color: var(--text-muted); width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; cursor: pointer; border-radius: 0px; }
  .metrics-strip { display: flex; gap: 10px; padding: 10px 12px; background: #131722; border-bottom: 1px solid var(--border-subtle); flex-shrink: 0; }
  .metric-card { flex: 1; background: #181c27; border: 1px solid var(--border-subtle); padding: 8px 12px; display: flex; flex-direction: column; }
  .card-label { font-size: 9px; font-weight: 700; color: var(--text-muted); letter-spacing: 0.5px; margin-bottom: 4px; }
  .card-value { font-size: 18px; font-weight: 700; color: var(--text-bright); }
  .card-value.cyan { color: #06b6d4; }
  .card-value.green { color: var(--bull-color); }
  .card-sub { font-size: 9px; color: #787b86; margin-top: 2px; }
  .table-container { flex: 1; overflow: auto; background: var(--bg-surface); }
  .data-table { width: 100%; border-collapse: collapse; text-align: left; }
  .data-table th { background: #181c27; color: var(--text-muted); font-size: 9px; font-weight: 700; letter-spacing: 0.5px; padding: 6px 10px; border-bottom: 1px solid var(--border-subtle); position: sticky; top: 0; z-index: 10; white-space: nowrap; }
  .data-table td { padding: 6px 10px; border-bottom: 1px solid rgba(255, 255, 255, 0.03); white-space: nowrap; }
  .data-table tr:hover { background: var(--bg-hover); }
  .domain-tag { font-size: 8.5px; font-weight: 700; padding: 1px 4px; }
  .domain-tag.market { background: rgba(59, 130, 246, 0.15); color: #60a5fa; border: 1px solid rgba(59, 130, 246, 0.3); }
  .domain-tag.macro { background: rgba(245, 158, 11, 0.15); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.3); }
  .domain-tag.prediction_markets { background: rgba(139, 92, 246, 0.15); color: #c4b5fd; border: 1px solid rgba(139, 92, 246, 0.3); }
  .identifier-cell { font-weight: 600; color: var(--text-bright); }
  .tf-tag { font-size: 8.5px; font-weight: 700; padding: 1px 3px; background: rgba(255, 255, 255, 0.04); border: 1px solid var(--border-subtle); color: var(--accent-amber); }
  .filename-cell code { font-family: monospace; font-size: 10px; color: #93c5fd; }
  .val-cell.bold { color: var(--text-bright); font-weight: 700; }
  .codec-tag { font-size: 8px; font-weight: 700; color: #a78bfa; background: rgba(167, 139, 250, 0.1); border: 1px solid rgba(167, 139, 250, 0.3); padding: 1px 4px; }
  .status-cell { display: flex; align-items: center; gap: 5px; font-size: 9px; font-weight: 700; color: var(--bull-color); }
  .dot-green { width: 5px; height: 5px; background: var(--bull-color); }
  .loading-state, .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 200px; gap: 10px; color: var(--text-muted); }
  .spinning :global(svg) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>