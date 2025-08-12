import {
  init,
  loadChunks,
  setInputs,
  tick,
  getOutputs,
  setPolicy,
  type Metrics,
} from './mycos';

interface Header {
  inputs: number;
  outputs: number;
}

function parseHeader(buf: ArrayBuffer): Header {
  const view = new DataView(buf);
  const magic = new TextDecoder().decode(new Uint8Array(buf.slice(0, 8)));
  if (magic !== 'MYCOSCH0') {
    throw new Error('Invalid chunk file');
  }
  return {
    inputs: view.getUint32(0x0c, true),
    outputs: view.getUint32(0x10, true),
  };
}

const fixtureSelect = document.getElementById(
  'fixture-select'
) as HTMLSelectElement;
const loadBtn = document.getElementById('load-btn') as HTMLButtonElement;
const inputsDiv = document.getElementById('inputs') as HTMLDivElement;
const outputsDiv = document.getElementById('outputs') as HTMLDivElement;
const tickBtn = document.getElementById('tick-btn') as HTMLButtonElement;
const runBtn = document.getElementById('run-btn') as HTMLButtonElement;
const nTicksInput = document.getElementById('n-ticks') as HTMLInputElement;
const policySelect = document.getElementById(
  'policy-select'
) as HTMLSelectElement;
const metricRounds = document.getElementById(
  'metric-rounds'
) as HTMLSpanElement;
const metricEffects = document.getElementById(
  'metric-effects'
) as HTMLSpanElement;

let inputBits = 0;
let outputBits = 0;
let inputsWords = new Uint32Array(0);
let outputsWords = new Uint32Array(0);

async function loadFixture(name: string): Promise<void> {
  const url = `${import.meta.env.BASE_URL}fixtures/${name}.myc`;
  const buffer = await fetch(url).then((r) => r.arrayBuffer());
  const header = parseHeader(buffer);
  loadChunks([buffer]);
  inputBits = header.inputs;
  outputBits = header.outputs;
  inputsWords = new Uint32Array(Math.ceil(inputBits / 32));
  outputsWords = new Uint32Array(Math.ceil(outputBits / 32));
  renderInputs();
  renderOutputs();
}

function renderInputs(): void {
  inputsDiv.innerHTML = '';
  for (let i = 0; i < inputBits; i++) {
    const label = document.createElement('label');
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.addEventListener('change', () => {
      const word = Math.floor(i / 32);
      const bit = i % 32;
      if (cb.checked) {
        inputsWords[word] |= 1 << bit;
      } else {
        inputsWords[word] &= ~(1 << bit);
      }
    });
    label.append(cb, String(i));
    inputsDiv.append(label);
  }
}

function renderOutputs(): void {
  outputsDiv.innerHTML = '';
  for (let i = 0; i < outputBits; i++) {
    const span = document.createElement('span');
    const word = Math.floor(i / 32);
    const bit = i % 32;
    const on = (outputsWords[word] & (1 << bit)) !== 0;
    span.textContent = on ? '1' : '0';
    outputsDiv.append(span);
  }
}

function refreshOutputs(): void {
  getOutputs(0, outputsWords);
  renderOutputs();
}

function updateMetrics(m: Metrics): void {
  metricRounds.textContent = String(m.rounds);
  metricEffects.textContent = String(m.effects);
}

tickBtn.addEventListener('click', () => {
  setInputs(0, inputsWords);
  const m = tick();
  refreshOutputs();
  updateMetrics(m);
});

runBtn.addEventListener('click', () => {
  const n = parseInt(nTicksInput.value, 10) || 0;
  setInputs(0, inputsWords);
  const m = tick(n);
  refreshOutputs();
  updateMetrics(m);
});

loadBtn.addEventListener('click', () => {
  void loadFixture(fixtureSelect.value);
});

policySelect.addEventListener('change', () => {
  setPolicy(policySelect.value as 'freeze' | 'clamp' | 'parity');
});

void init();
