<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { getOidcLoginUrl, loginWithCredential } from '../lib/api';

  export let isOpen: boolean = false;
  export let oidcEnabled: boolean = false;
  export let oidcProvider: string = 'rIDM SSO';

  const dispatch = createEventDispatcher();
  let credential: string = '';
  let errorText: string | null = null;
  let submitting: boolean = false;

  async function handleSubmit() {
    if (!credential.trim() || submitting) return;
    submitting = true;
    errorText = null;
    try {
      const session = await loginWithCredential(credential);
      credential = '';
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
  <div class="auth-backdrop" role="dialog" aria-modal="true" aria-label="Terminal Session Sign In">
    <div class="auth-panel">
      <div class="auth-header">
        <span class="auth-title">TERMINAL SESSION AUTHENTICATION</span>
        <span class="auth-badge">HTTPONLY AEAD</span>
      </div>
      <div class="auth-body">
        <p class="auth-desc">
          Sign in to start an encrypted <code>HttpOnly</code> browser session. No secret is ever stored in <code>localStorage</code>.
        </p>

        {#if errorText}
          <div class="auth-error">{errorText}</div>
        {/if}

        <form class="auth-form" on:submit|preventDefault={handleSubmit}>
          <input
            type="password"
            class="auth-input"
            autocomplete="current-password"
            placeholder="Operator passphrase or API key"
            bind:value={credential}
            disabled={submitting}
          />
          <button type="submit" class="btn-primary" disabled={submitting || !credential.trim()}>
            {submitting ? 'VERIFYING...' : 'SIGN IN'}
          </button>
        </form>

        {#if oidcEnabled}
          <div class="auth-divider"><span>OR ENTERPRISE SSO</span></div>
          <a class="btn-sso" href={getOidcLoginUrl()}>
            SIGN IN WITH {oidcProvider.toUpperCase()}
          </a>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .auth-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 9, 13, 0.9);
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
  .btn-primary, .btn-sso {
    display: block;
    text-align: center;
    text-decoration: none;
    border-radius: 0;
    padding: 8px 14px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    cursor: pointer;
  }
  .btn-primary {
    background: #1f6feb;
    border: 1px solid #1f6feb;
    color: #ffffff;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .auth-divider {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #6e7681;
    font-size: 10px;
    letter-spacing: 0.08em;
  }
  .auth-divider::before, .auth-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: #1f2430;
  }
  .btn-sso {
    background: #161b22;
    border: 1px solid #2d3446;
    color: #60a5fa;
  }
</style>
