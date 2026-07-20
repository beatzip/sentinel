# Sentinel AI - Dataset Collection Plan

## Sources

### Legit Demos (Target: 200)

| Source | Count | Label | How to Get |
|--------|-------|-------|------------|
| HLTV Pro Matches | 100 | Legit | HLTV API / manual download |
| Faceit L10 | 50 | Legit | Faceit API |
| Premier Mode | 50 | Legit | CS2 built-in |

### Cheater Demos (Target: 50)

| Source | Count | Label | How to Get |
|--------|-------|-------|------------|
| Overwatch Reviews | 30 | Cheater | Valve (limited) |
| Community Reports | 20 | Cheater | Reddit, forums |

### Unknown (Target: 50)

| Source | Count | Label |
|--------|-------|-------|
| Mixed MM | 50 | Unknown |

## Dataset Structure

```
dataset/
├── metadata.csv
│   ├── demo_path
│   ├── map
│   ├── duration
│   ├── label (legit/cheater/unknown)
│   └── source
├── legit/
│   ├── hltv/
│   ├── faceit/
│   └── premier/
├── cheater/
│   ├── overwatch/
│   └── community/
└── unknown/
```

## Validation Metrics

With 200 labeled demos:
- Precision @ 10% threshold
- Recall @ 90% TPR
- F1 Score
- AUC-ROC
- Per-map analysis
- Per-skill-level analysis
