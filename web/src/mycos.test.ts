import { describe, expect, it } from 'vitest';
import { init, tick } from './mycos';
import * as engineMod from '../engine/pkg/engine.js';

describe('mycos wasm wrapper', () => {
  it('initializes and ticks with real WebGPU', async () => {
    const mod = engineMod as { default?: unknown };
    expect(typeof mod.default).toBe('function');
    const canvas = document.createElement('canvas');
    await init(canvas);
    const metrics = tick();
    expect(metrics.rounds).toBe(0);
    expect(metrics.effects).toBe(0);
  });
});
