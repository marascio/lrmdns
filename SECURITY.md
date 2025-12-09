# Security Policy

## Supported Versions

As lrmdns is currently in early development (v0.1.x), only the latest version receives security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**Do NOT create a public GitHub issue for security vulnerabilities.**

Instead, please report security issues via:
- **GitHub Security Advisories** (preferred): https://github.com/marascio/lrmdns/security/advisories/new
  - This allows for private disclosure and coordinated response
  - GitHub will notify the maintainers automatically

### What to Include

When reporting a vulnerability, please include:
- Description of the vulnerability
- Steps to reproduce the issue
- Potential impact
- Any suggested fixes (if you have them)

### Response Timeline

- **Acknowledgment**: Within 48 hours of report
- **Initial Assessment**: Within 1 week
- **Fix Timeline**: Depends on severity
  - Critical: Within 7 days
  - High: Within 30 days
  - Medium: Within 90 days
  - Low: Next minor release

## Security Considerations

### Authoritative-Only Server

lrmdns is designed as an **authoritative-only** DNS server. It does NOT:
- Perform recursive resolution
- Cache responses from other servers
- Act as a recursive resolver for clients

This design significantly reduces the attack surface compared to full-service DNS servers.

### Known Limitations

- **No DNSSEC signing**: The server only serves pre-signed zones. Online signing is not implemented.
- **Rate limiting**: Basic per-IP rate limiting is implemented but may not be sufficient for all DDoS scenarios.
- **Zone transfer security**: AXFR is supported but without TSIG authentication.
- **NSEC3 not supported**: Only NSEC is supported for authenticated denial of existence.

### Recommended Deployment Practices

1. **Run behind a firewall**: Restrict DNS port (53) access to legitimate networks
2. **Use rate limiting**: Configure appropriate rate limits in `lrmdns.yaml`
3. **Monitor logs**: Enable structured logging and monitor for anomalous traffic
4. **Regular updates**: Keep lrmdns and dependencies up to date
5. **Principle of least privilege**: Run lrmdns as a non-root user with minimal permissions
6. **Zone file validation**: Validate zone files before deploying to production

### Security Audits

This project has not yet undergone a professional security audit. Contributions for security review are welcome.

## Disclosure Policy

When a security vulnerability is confirmed:
1. A fix will be developed and tested
2. A security advisory will be published
3. A new version will be released
4. The vulnerability will be publicly disclosed after users have had time to update

## Contact

For non-security issues, please use the GitHub issue tracker.
For security concerns, please follow the reporting process above.
