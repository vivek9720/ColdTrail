# ColdTrail

ColdTrail is an offline Rust toolkit for reviewing cold-chain shipment
evidence. It helps quality, logistics, and compliance teams reconcile route
metadata, temperature data-logger exports, ANSI X12 EDI 214 shipment-status
messages, and refrigerated-trailer telemetry captures before a shipment is
released or escalated for investigation.

Cold-chain records often arrive as a mix of files from different systems:
carrier EDI updates, CSV exports from USB/Bluetooth loggers, route manifests
from a TMS, and gateway captures from refrigerated equipment. ColdTrail gives
those artifacts one local validation path with no network calls and no external
service dependencies.

## What It Does

- Parses data-logger CSV exports with common headers such as `timestamp`,
  `sensor_code`, `temperature`, `value_c`, `unit`, `quality`, and `status`.
- Parses the shipment-status subset of ANSI X12 EDI 214, including `B10`,
  `MS2`, `AT7`, `MS1`, `L11`, and loop markers.
- Parses plain-text evidence bundles that combine route facts, CSV logs, EDI
  status history, notes, and optional gateway captures.
- Decodes staged refrigerated-trailer gateway captures for teams that archive
  binary fleet telemetry alongside business records.
- Validates temperature excursions, manifest consistency, logger quality
  flags, delivery-status coverage, and binary-capture warnings.
- Provides a small CLI for local audit triage.

## CLI

```text
coldtrail <evidence-bundle>
```

Example:

```text
coldtrail examples/validated_shipment.cte
```

The command prints a text report and exits non-zero when critical findings are
present.

## Library API

```rust
let bundle = coldtrail::parse_evidence_bundle(bytes)?;
let report = coldtrail::validate_evidence(&bundle);

let samples = coldtrail::parse_logger_csv(csv_bytes)?;
let statuses = coldtrail::parse_edi214(edi_bytes)?;
```

Gateway captures can also be decoded directly:

```rust
let decoded = coldtrail::parse(capture_bytes)?;
let replay = coldtrail::replay(replay_bytes)?;
```

## Evidence Bundle Format

ColdTrail evidence bundles are plain text files with named sections:

```text
[manifest]
route_id=4660
product_class=vaccine
temp_range=-20.0,-8.0
stop1=1,0x5001,35,37.7740,-122.4190

[logger_csv]
timestamp,sensor_code,value_c,unit,quality
2026-07-05T08:00:00Z,0x4100,-18.2,C,ok

[edi214]
ST*214*0001~
B10*BOL7781*SHIP7781*CTRX~
AT7*D1***20260705*1500*UT~
MS1*LOS ANGELES*CA*US~
SE*5*0001~
```

## Development

The crate has no registry dependencies and is designed to build in offline
quality-control environments. Parser regression inputs are kept with the
repository so validation behavior can be exercised from a clean checkout.
