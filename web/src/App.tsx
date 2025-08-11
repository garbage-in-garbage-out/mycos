import { useState } from 'preact/hooks';
import {
  init,
  loadChunks,
  setInputs,
  tick,
  getOutputs,
  setPolicy,
  Metrics,
} from './mycos';

interface ChunkInfo {
  buffer: ArrayBuffer;
  inputBits: number;
  outputBits: number;
}

function parseChunk(buffer: ArrayBuffer): ChunkInfo {
  const view = new DataView(buffer);
  const inputBits = view.getUint32(0x0c, true);
  const outputBits = view.getUint32(0x10, true);
  return { buffer, inputBits, outputBits };
}

function bitsToWords(bits: number): number {
  return Math.ceil(bits / 32);
}

export function App() {
  const [ready, setReady] = useState(false);
  const [chunk, setChunk] = useState<ChunkInfo | null>(null);
  const [inputs, setInputState] = useState<Uint32Array>(new Uint32Array(0));
  const [outputs, setOutputs] = useState<Uint32Array>(new Uint32Array(0));
  const [metrics, setMetrics] = useState<Metrics>({ rounds: 0, effects: 0 });
  const [tickCount, setTickCount] = useState(1);
  const [policy, setPolicyState] = useState<'freeze' | 'clamp' | 'parity'>(
    'freeze'
  );

  async function ensureInit() {
    if (!ready) {
      await init();
      setReady(true);
    }
  }

  async function onLoadChunk(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    await ensureInit();
    const buf = await file.arrayBuffer();
    const info = parseChunk(buf);
    setChunk(info);
    const inWords = new Uint32Array(bitsToWords(info.inputBits));
    const outWords = new Uint32Array(bitsToWords(info.outputBits));
    setInputState(inWords);
    setOutputs(outWords);
    loadChunks([buf]);
  }

  function toggleBit(idx: number) {
    const word = Math.floor(idx / 32);
    const bit = idx % 32;
    const next = new Uint32Array(inputs);
    next[word] ^= 1 << bit;
    setInputState(next);
    setInputs(0, next);
  }

  function runTick(times: number) {
    let last: Metrics = { rounds: 0, effects: 0 };
    for (let i = 0; i < times; i++) {
      last = tick();
    }
    setMetrics(last);
    if (chunk) {
      const out = new Uint32Array(outputs.length);
      getOutputs(0, out);
      setOutputs(out);
    }
  }

  function onRun() {
    runTick(1);
  }

  function onRunMany() {
    runTick(tickCount);
  }

  function onPolicyChange(e: Event) {
    const mode = (e.target as HTMLSelectElement).value as
      | 'freeze'
      | 'clamp'
      | 'parity';
    setPolicy(mode);
    setPolicyState(mode);
  }

  return (
    <div>
      <h1>Mycos Dev UI</h1>
      <section>
        <h2>Load Fixtures</h2>
        <input type="file" accept=".myc" onChange={onLoadChunk} />
      </section>
      {chunk && (
        <section>
          <h2>Inputs</h2>
          <div>
            {Array.from({ length: chunk.inputBits }).map((_, i) => (
              <label key={i} style={{ marginRight: '0.5rem' }}>
                <input
                  type="checkbox"
                  checked={!!(inputs[Math.floor(i / 32)] & (1 << i % 32))}
                  onChange={() => toggleBit(i)}
                />
                {i}
              </label>
            ))}
          </div>
        </section>
      )}
      <section>
        <h2>Controls</h2>
        <button onClick={onRun}>Tick</button>
        <input
          type="number"
          min="1"
          value={tickCount}
          onInput={(e) =>
            setTickCount(
              parseInt((e.target as HTMLInputElement).value, 10) || 1
            )
          }
          style={{ width: '4rem', margin: '0 0.5rem' }}
        />
        <button onClick={onRunMany}>Run N Ticks</button>
        <label style={{ marginLeft: '1rem' }}>
          Policy:
          <select
            value={policy}
            onChange={onPolicyChange}
            style={{ marginLeft: '0.5rem' }}
          >
            <option value="freeze">freeze</option>
            <option value="clamp">clamp</option>
            <option value="parity">parity</option>
          </select>
        </label>
      </section>
      {chunk && (
        <section>
          <h2>Outputs</h2>
          <div>
            {Array.from(outputs).map((word, i) => (
              <div key={i}>
                Word {i}: 0x{word.toString(16).padStart(8, '0')}
              </div>
            ))}
          </div>
        </section>
      )}
      <section>
        <h2>Metrics</h2>
        <ul>
          <li>Rounds: {metrics.rounds}</li>
          <li>Effects: {metrics.effects}</li>
          <li>Proposals: 0</li>
          <li>Winners: 0</li>
          <li>Oscillator: false</li>
          <li>Policy: {policy}</li>
        </ul>
      </section>
    </div>
  );
}
