import { describe, expect, it } from 'bun:test';
import { fetchInstruments } from '../src/lib/api';

describe('API Catalog & Fallback Tests', () => {
  it('fetchInstruments returns non-empty list of core instruments on network failure', async () => {
    const list = await fetchInstruments();
    expect(list.length).toBeGreaterThan(0);
    
    // Ensure core instruments exist
    const symbols = list.map((i) => i.symbol);
    expect(symbols).toContain('BBCA');
    expect(symbols).toContain('GSPC');
    expect(symbols).toContain('BTC_USDT');

    // Ensure all items have valid structures
    for (const inst of list) {
      expect(inst.symbol).toBeDefined();
      expect(inst.canonical_symbol).toBeDefined();
      expect(inst.asset_class).toBeDefined();
      expect(inst.timeframes.length).toBeGreaterThan(0);
    }
  });
});
