<script lang="ts">
  import { onMount } from 'svelte';
  import type { AppViewId } from '../lib/types';
  import Icons from './Icons.svelte';

  export let isOpen: boolean = false;
  export let activeView: AppViewId = 'terminal';
  export let onSelectView: (view: AppViewId) => void;
  export let onClose: () => void;

  interface AppItem {
    id: AppViewId;
    title: string;
    category: string;
    description: string;
    badge: string;
    icon: 'activity' | 'calendar' | 'trending' | 'layers' | 'folder' | 'shield' | 'refresh';
    accentColor: string;
    shortcut: string;
  }

  const apps: AppItem[] = [
    {
      id: 'terminal',
      title: 'Market Terminal',
      category: 'CHARTING & EXECUTION',
      description: 'Zero-latency Parquet L1 candlestick terminal with TradingView charts.',
      badge: 'LIVE',
      icon: 'activity',
      accentColor: '#3b82f6',
      shortcut: '1',
    },
    {
      id: 'macro',
      title: 'Macro Hub',
      category: 'MACROECONOMICS',
      description: 'Point-in-Time calendar, variance timeline, revisions, and SDMX attribution.',
      badge: 'PiT REVISIONS',
      icon: 'calendar',
      accentColor: '#f59e0b',
      shortcut: '2',
    },
    {
      id: 'predictions',
      title: 'Prediction Markets',
      category: 'PROBABILISTIC PRICING',
      description: 'Kalshi & Polymarket contracts, probability curves, and oracle specs.',
      badge: 'ORACLES',
      icon: 'trending',
      accentColor: '#8b5cf6',
      shortcut: '3',
    },
    {
      id: 'consensus',
      title: 'Consensus Matrix',
      category: 'DIVERGENCE ANALYTICS',
      description: 'Economist consensus estimates vs prediction market crowd probabilities.',
      badge: 'VARIANCE',
      icon: 'layers',
      accentColor: '#089981',
      shortcut: '4',
    },
    {
      id: 'lakehouse',
      title: 'Lakehouse Explorer',
      category: 'DATA INFRASTRUCTURE',
      description: 'Parquet partition hierarchy, zero-decompression row counts, and disk metrics.',
      badge: 'STORAGE',
      icon: 'folder',
      accentColor: '#06b6d4',
      shortcut: '5',
    },
    {
      id: 'integrity',
      title: 'Integrity Gauntlet',
      category: 'QUALITY & SANITY',
      description: 'Audit 6 core sanity invariants, zero-inverted-bars, and Hampel outlier flags.',
      badge: 'GOVERNANCE',
      icon: 'shield',
      accentColor: '#ef4444',
      shortcut: '6',
    },
    {
      id: 'subscriptions',
      title: 'Autonomous Ingestion',
      category: 'DATA PIPELINE',
      description: 'Manage active feeds, multi-source discovery search, and background gap catch-up.',
      badge: 'DAEMON',
      icon: 'refresh',
      accentColor: '#10b981',
      shortcut: '7',
    },
  ];

  let searchQuery = '';

  $: filteredApps = apps.filter((app) => {
    const q = searchQuery.toLowerCase();
    return (
      app.title.toLowerCase().includes(q) ||
      app.category.toLowerCase().includes(q) ||
      app.description.toLowerCase().includes(q)
    );
  });

  function handleSelect(id: AppViewId) {
    onSelectView(id);
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!isOpen) return;
    if (e.key === 'Escape') {
      onClose();
    } else if (['1', '2', '3', '4', '5', '6'].includes(e.key) && !searchQuery) {
      const idx = parseInt(e.key, 10) - 1;
      if (apps[idx]) {
        handleSelect(apps[idx].id);
      }
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if isOpen}
  <!-- Backdrop -->
  <div class="launcher-backdrop" on:click={onClose} role="presentation">
    <!-- Modal Window (Homescreen App Drawer) -->
    <div class="launcher-window" on:click|stopPropagation role="dialog" aria-modal="true">
      <!-- Top Bar: Header & Search -->
      <div class="launcher-header">
        <div class="header-title-box">
          <Icons name="grid" size={16} classNames="header-icon" />
          <span class="header-title">APPLICATIONS & DATA EXPLORERS</span>
          <span class="header-sub">ANDROID HOMESCREEN NAVIGATION</span>
        </div>
        <button class="close-btn" on:click={onClose} title="Close launcher (Esc)">
          <Icons name="close" size={14} />
        </button>
      </div>

      <!-- Quick Search Bar -->
      <div class="search-bar-wrap">
        <Icons name="search" size={14} classNames="search-icon" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Search datasets, views, or press 1-6..."
          class="search-input"
          autofocus
        />
        {#if searchQuery}
          <button class="clear-btn" on:click={() => (searchQuery = '')}>
            <Icons name="close" size={12} />
          </button>
        {/if}
      </div>

      <!-- Apps Grid (Homescreen Tiles) -->
      <div class="apps-grid">
        {#each filteredApps as app}
          <button
            class="app-tile"
            class:active={activeView === app.id}
            on:click={() => handleSelect(app.id)}
            style="--tile-accent: {app.accentColor}"
          >
            <div class="tile-top">
              <div class="app-icon-box">
                <Icons name={app.icon} size={20} />
              </div>
              <div class="badge-cluster">
                <span class="shortcut-pill">[{app.shortcut}]</span>
                <span class="badge-pill">{app.badge}</span>
              </div>
            </div>

            <div class="tile-body">
              <div class="app-title">{app.title}</div>
              <div class="app-category">{app.category}</div>
              <div class="app-desc">{app.description}</div>
            </div>

            <div class="tile-footer">
              <span class="active-indicator">
                {#if activeView === app.id}
                  ACTIVE VIEW
                {:else}
                  LAUNCH APP
                {/if}
              </span>
            </div>
          </button>
        {/each}
      </div>

      <!-- Footer System Status -->
      <div class="launcher-footer tabular-nums">
        <div class="footer-left">
          <span class="system-tag">ENGINE: DUAL-REMOTE VSA</span>
          <span class="system-tag">STORAGE: PARQUET L1</span>
          <span class="system-tag">SANITY: 6 INVARIANTS</span>
        </div>
        <div class="footer-right">
          <span>PRESS [ESC] TO CLOSE</span>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .launcher-backdrop { position: fixed; inset: 0; background: rgba(10, 14, 23, 0.75); backdrop-filter: blur(4px); z-index: 999; display: flex; align-items: center; justify-content: center; padding: 16px; }
  .launcher-window { background: #131722; border: 1px solid var(--border-contrast); width: 100%; max-width: 820px; max-height: 90vh; display: flex; flex-direction: column; box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6); }
  .launcher-header { height: 44px; padding: 0 14px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--border-subtle); background: #1e222d; }
  .header-title-box { display: flex; align-items: center; gap: 8px; }
  .header-title { font-size: 11.5px; font-weight: 700; letter-spacing: 0.5px; color: var(--text-bright); }
  .header-sub { font-size: 9px; color: var(--text-muted); letter-spacing: 0.5px; padding-left: 6px; border-left: 1px solid var(--border-subtle); }
  .close-btn { background: transparent; border: none; color: var(--text-muted); cursor: pointer; padding: 4px; display: flex; align-items: center; }
  .close-btn:hover { color: var(--text-bright); }
  .search-bar-wrap { padding: 10px 14px; border-bottom: 1px solid var(--border-subtle); background: #181c27; display: flex; align-items: center; gap: 8px; }
  .search-input { flex: 1; background: transparent; border: none; color: var(--text-bright); font-family: inherit; font-size: 12px; outline: none; }
  .clear-btn { background: transparent; border: none; color: var(--text-muted); cursor: pointer; padding: 2px; }
  .apps-grid { padding: 16px; display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr)); gap: 12px; overflow-y: auto; max-height: 520px; }
  .app-tile { background: #181c27; border: 1px solid var(--border-subtle); padding: 12px; display: flex; flex-direction: column; text-align: left; cursor: pointer; transition: background 0.12s, border-color 0.12s; outline: none; position: relative; }
  .app-tile:hover { background: #1e2433; border-color: var(--tile-accent); }
  .app-tile.active { background: #1e2638; border-color: var(--tile-accent); box-shadow: inset 0 0 0 1px var(--tile-accent); }
  .tile-top { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .app-icon-box { width: 36px; height: 36px; background: rgba(255, 255, 255, 0.03); border: 1px solid var(--border-subtle); color: var(--tile-accent); display: flex; align-items: center; justify-content: center; }
  .badge-cluster { display: flex; align-items: center; gap: 4px; }
  .shortcut-pill { background: rgba(255, 255, 255, 0.05); border: 1px solid var(--border-subtle); color: var(--text-muted); font-size: 9px; font-weight: 700; padding: 1px 4px; }
  .badge-pill { background: rgba(255, 255, 255, 0.04); border: 1px solid var(--border-subtle); color: var(--tile-accent); font-size: 8.5px; font-weight: 700; padding: 1px 5px; letter-spacing: 0.5px; }
  .tile-body { flex: 1; margin-bottom: 8px; }
  .app-title { font-size: 13px; font-weight: 700; color: var(--text-bright); margin-bottom: 2px; letter-spacing: 0.2px; }
  .app-category { font-size: 8.5px; font-weight: 700; color: var(--text-muted); letter-spacing: 0.5px; margin-bottom: 6px; }
  .app-desc { font-size: 10.5px; color: #8b95af; line-height: 1.4; }
  .tile-footer { border-top: 1px solid rgba(255, 255, 255, 0.04); padding-top: 8px; display: flex; justify-content: flex-end; }
  .active-indicator { font-size: 9px; font-weight: 700; color: var(--tile-accent); letter-spacing: 0.5px; }
  .launcher-footer { height: 32px; background: #11141c; border-top: 1px solid var(--border-subtle); padding: 0 14px; display: flex; align-items: center; justify-content: space-between; font-size: 9.5px; color: var(--text-muted); }
  .footer-left { display: flex; align-items: center; gap: 8px; }
  .system-tag { background: rgba(255, 255, 255, 0.03); padding: 1px 4px; border: 1px solid var(--border-subtle); }
</style>