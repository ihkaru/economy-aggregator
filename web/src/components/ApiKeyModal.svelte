<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { loginWithAccount } from '../lib/api';

  export let isOpen: boolean = false;

  const dispatch = createEventDispatcher();
  let username: string = '';
  let password: string = '';
  let errorText: string | null = null;
  let submitting: boolean = false;

  async function handleSubmit() {
    if (!username.trim() || !password.trim() || submitting) return;
    submitting = true;
    errorText = null;
    try {
      const session = await loginWithAccount(username.trim(), password);
      password = '';
      dispatch('authenticated', session);
      dispatch('close');
    } catch (e: any) {
      errorText = e.message || 'Authentication failed';
    } finally {
      submitting = false;
    }
  }
</script>

{#if isOpen}
  <div class="auth-backdrop" role="dialog" aria-modal="true" aria-label="Terminal Admin Sign In">
    <div class="auth-panel">
      <div class="auth-header">
        <span class="auth-title">TERMINAL ADMIN AUTHENTICATION</span>
        <span class="auth-badge">HTTPONLY AEAD</span>
      </div>
      <div class="auth-body">
        <p class="auth-desc">
          Sign in with your standalone UI admin account. Sessions use encrypted <code>HttpOnly</code> cookies (no secrets stored in <code>localStorage</code>).
        </p>

        {#if errorText}
          <div class="auth-error">{errorText}</div>
        {/if}

        <form class="auth-form" on:submit|preventDefault={handleSubmit}>
          <label class="field-label">
            <span>USERNAME</span>
            <input
              type="text"
              class="auth-input"
              autocomplete="username"
              placeholder="ihzakarunia"
              bind:value={username}
              disabled={submitting}
            />
          </label>
          <label class="field-label">
            <span>PASSWORD</span>
            <input
              type="password"
              class="auth-input"
              autocomplete="current-password"
              placeholder="Admin password"
              bind:value={password}
              disabled={submitting}
            />
          </label>
          <button
            type="submit"
            class="btn-primary"
            disabled={submitting || !username.trim() || !password.trim()}
          >
            {submitting ? 'VERIFYING...' : 'SIGN IN'}
          </button>
        </form>
      </div>
    </div>
  </div>
{/if}

<style>
  .auth-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 9, 13, 0.92);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }
  .auth-panel {
    width: 400px;
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
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #93a3b8;
  }
  .auth-badge {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 2px 6px;
    border: 1px solid #089981;
    color: #089981;
  }
  .auth-body {
    padding: 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .auth-desc {
    margin: 0;
    font-size: 12px;
    color: #8b949e;
    line-height: 1.45;
  }
  .auth-error {
    background: rgba(242, 54, 69, 0.14);
    border: 1px solid #f23645;
    color: #fda4af;
    font-size: 11px;
    padding: 7px 10px;
  }
  .auth-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field-label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #93a3b8;
  }
  .auth-input {
    width: 100%;
    box-sizing: border-box;
    background: #08090d;
    border: 1px solid #2d3446;
    border-radius: 0;
    color: #e6edf3;
    padding: 9px 10px;
    font-size: 13px;
  }
  .btn-primary {
    margin-top: 4px;
    border-radius: 0;
    padding: 9px 14px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    cursor: pointer;
    background: #1f6feb;
    border: 1px solid #1f6feb;
    color: #ffffff;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
