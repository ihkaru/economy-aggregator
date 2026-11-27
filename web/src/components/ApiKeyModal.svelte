<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { getApiKey, setApiKey, clearApiKey } from '../lib/api';

  export let isOpen: boolean = false;
  const dispatch = createEventDispatcher();
  let keyInput: string = getApiKey();

  function handleSave() {
    setApiKey(keyInput);
    dispatch('saved');
    dispatch('close');
  }

  function handleClear() {
    clearApiKey();
    keyInput = '';
    dispatch('saved');
  }
</script>

{#if isOpen}
  <div class="auth-backdrop" role="dialog" aria-modal="true" aria-label="API Key Authentication">
    <div class="auth-panel">
      <div class="auth-header">
        <span class="auth-title">API KEY AUTHENTICATION</span>
        <button class="auth-close" on:click={() => dispatch('close')}>ESC</button>
      </div>
      <div class="auth-body">
        <p class="auth-desc">
          Server requires an API key (<code>X-API-Key</code>). Stored locally in this browser only.
        </p>
        <input
          type="password"
          class="auth-input"
          placeholder="ea_prod_..."
          bind:value={keyInput}
          on:keydown={(e) => e.key === 'Enter' && handleSave()}
        />
        <div class="auth-actions">
          <button class="btn-primary" on:click={handleSave}>APPLY KEY</button>
          <button class="btn-secondary" on:click={handleClear}>CLEAR</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .auth-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 9, 13, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }
  .auth-panel {
    width: 420px;
    max-width: 92vw;
    background: #0d1017;
    border: 1px solid #2d3446;
    border-radius: 0;
    color: #e6edf3;
    font-family: 'Google Sans', sans-serif;
  }
  .auth-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    border-bottom: 1px solid #1f2430;
    background: #08090d;
  }
  .auth-title {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: #93a3b8;
  }
  .auth-close {
    background: transparent;
    border: 1px solid #2d3446;
    border-radius: 0;
    color: #93a3b8;
    font-size: 11px;
    padding: 2px 6px;
    cursor: pointer;
  }
  .auth-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .auth-desc {
    margin: 0;
    font-size: 12px;
    color: #8b949e;
    line-height: 1.4;
  }
  .auth-input {
    width: 100%;
    box-sizing: border-box;
    background: #08090d;
    border: 1px solid #2d3446;
    border-radius: 0;
    color: #e6edf3;
    padding: 8px 10px;
    font-size: 13px;
  }
  .auth-actions {
    display: flex;
    gap: 8px;
  }
  .btn-primary, .btn-secondary {
    border-radius: 0;
    padding: 7px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid #2d3446;
  }
  .btn-primary {
    background: #1f6feb;
    border-color: #1f6feb;
    color: #ffffff;
  }
  .btn-secondary {
    background: #161b22;
    color: #c9d1d9;
  }
</style>
