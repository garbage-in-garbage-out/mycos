# Mycos Binary Fixtures

This directory contains golden binary fixtures for testing the Mycos chunk format and execution engine.

## Binary Files (.myc)

All binary files follow the Mycos v1 binary specification:

### tiny_toggle.myc
- **Ni=1, No=1, Nn=1**
- Simple Input→Internal→Output chain
- Tests basic propagation through a single internal bit
- Expected behavior: Input[0] high → Internal[0] high → Output[0] high

### oscillator_2cycle.myc  
- **Ni=0, No=1, Nn=2**
- Two internal bits that toggle each other
- Tests oscillation detection and SCC handling
- Initial state: Internal[0]=1, Internal[1]=0
- Expected behavior: 2-cycle oscillation that should be detected and quenched

### fanout_1_to_1024.myc
- **Ni=1, No=0, Nn=1024**
- Single input fans out to 1024 internal bits
- Tests large-scale parallel processing
- Expected behavior: Input[0] high → all 1024 Internal bits high

### noop.myc
- **Ni=2, No=2, Nn=0**
- Valid header with no connections
- Tests parser with empty connection table
- Expected behavior: No state changes regardless of input

### gated_child.myc
- **Ni=1, No=1, Nn=1**
- Simple child chunk for gated nesting tests
- Tests basic chunk functionality in isolation
- Expected behavior: Input→Internal→Output propagation

### parent_with_gate.myc
- **Ni=1, No=1, Nn=2**
- Parent chunk with gate bit for nesting tests
- Internal[0] acts as gate, Internal[1] as logic
- Expected behavior: Input enables gate, gate enables logic, logic enables output

## JSON Expectation Files

Each .myc file has a corresponding .json file with expected execution traces:

- **initial_state**: Starting bit values
- **test_cases**: Array of test scenarios
  - **input_changes**: Input modifications to apply
  - **expected_ticks**: Tick-by-tick state evolution
    - **state**: Complete bit state after tick
    - **changes**: Only the bits that changed this tick

## Binary Format Validation

All binary files can be validated using:

```bash
hexdump -C fixtures/tiny_toggle.myc | head -5
```

Expected header format:
```
00000000  4d 59 43 4f 53 43 48 30  01 00 00 00 01 00 00 00  |MYCOSCH0........|
00000010  01 00 00 00 01 00 00 00  02 00 00 00 00 00 00 00  |................|
```

- Bytes 0-7: Magic "MYCOSCH0"
- Bytes 8-9: Version 0x0001
- Bytes 10-11: Flags 0x0000
- Bytes 12-15: InputBits (little-endian u32)
- Bytes 16-19: OutputBits (little-endian u32)
- Bytes 20-23: InternalBits (little-endian u32)
- Bytes 24-27: ConnectionCount (little-endian u32)
- Bytes 28-31: Reserved (0x00000000)

## Usage in Tests

These fixtures are designed for:

1. **Parser validation**: Ensure binary format is read correctly
2. **Execution testing**: Verify CPU reference implementation
3. **GPU validation**: Compare GPU results against CPU reference
4. **Performance benchmarking**: Measure execution speed on known workloads
5. **Regression testing**: Detect changes in behavior across versions

## Generation

Fixtures were generated using `generate_fixtures.py` which creates binary files according to the exact specification in AGENTS.md section 3.