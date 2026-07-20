# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in Sentinel AI, please report it responsibly.

### How to Report

1. **DO NOT** open a public issue
2. Email security reports to: [security@sentinel-ai.dev]
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### What to Expect

- Acknowledgment within 48 hours
- Assessment within 1 week
- Fix timeline based on severity
- Credit in release notes (unless you prefer anonymity)

## Security Considerations

### Data Privacy

- Sentinel AI analyzes demo files locally
- No data is sent to external servers
- Player data is processed in-memory only
- Reports are generated locally

### Demo File Safety

- Demo files are parsed in a sandboxed environment
- Malformed files are handled gracefully
- No code execution from demo content
- Resource limits prevent DoS via large files

### Network Security

- No network access required for analysis
- Optional: telemetry can be disabled
- No automatic updates without user consent

### Dependency Security

- Dependencies are audited regularly
- Minimal dependency footprint
- No unsafe code in core analysis

## Security Best Practices

When deploying Sentinel AI:

1. Run with minimal privileges
2. Restrict file system access to demo directories
3. Monitor resource usage
4. Keep dependencies updated
5. Review custom configurations

## Contact

For security inquiries: [security@sentinel-ai.dev]
